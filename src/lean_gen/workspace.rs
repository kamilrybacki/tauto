use serde::{Deserialize, Serialize};

use crate::contract_ir::{Condition, ContractIR, ContractSet, Expression, ExpressionValue};
use crate::lean_gen::model::{self, Model};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LeanWorkspaceFile {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LeanWorkspace {
    pub files: Vec<LeanWorkspaceFile>,
}

fn lean_ident(s: &str) -> String {
    let cleaned: String = s
        .replace(' ', "")
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect();
    if cleaned.is_empty() {
        return "Contract".to_owned();
    }
    if cleaned.chars().next().unwrap().is_ascii_digit() {
        return format!("C{cleaned}");
    }
    cleaned
}

fn assign_module_names(contracts: &[ContractIR]) -> Vec<String> {
    use std::collections::HashMap;
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut names = Vec::with_capacity(contracts.len());
    for c in contracts {
        let base = lean_ident(&c.case);
        let count = seen.entry(base.clone()).or_insert(0);
        let name = if *count == 0 { base.clone() } else { format!("{}_{}", base, count) };
        *count += 1;
        names.push(name);
    }
    names
}

fn render_expr(expr: &Expression) -> String {
    match (expr.kind.as_str(), &expr.value) {
        // Lean 4 enum constructors use dot-notation when the type is inferrable
        ("enum", ExpressionValue::Str(s)) => format!(".{s}"),
        // Bool literals are lowercase in Lean 4 — Rust's Display already does this
        ("bool", ExpressionValue::Bool(b)) => b.to_string(),
        // Int / Nat literals
        ("int", ExpressionValue::Int(n)) => n.to_string(),
        // Field accessors and variables render as-is
        (_, ExpressionValue::Str(s)) => s.clone(),
        (_, ExpressionValue::Int(n)) => n.to_string(),
        (_, ExpressionValue::Bool(b)) => b.to_string(),
    }
}

/// A contract file: satisfiability theorems for each modelable condition
/// (`requires` and `ensures`). Each is a real, `sorry`-free proof that the
/// condition is satisfiable. Assumes/preserves and unmodelable conditions are
/// noted as comments. Original conditions are echoed as comments for context.
fn contract_file(contract: &ContractIR, module_name: &str, model: &Model) -> LeanWorkspaceFile {
    let mut body = vec![
        "import TautoContracts.Model".to_owned(),
        String::new(),
        format!("namespace Tauto.Contracts.{module_name}"),
        String::new(),
        format!("-- {}/{}/{}", contract.entity, contract.operation, contract.case),
        String::new(),
    ];

    if !contract.assumes.is_empty() {
        body.push(comment_block("Assumed preconditions", &contract.assumes));
    }
    if !contract.preserves.is_empty() {
        body.push(comment_block("Preserved invariants", &contract.preserves));
    }

    let mut n = 0usize;
    for (section, cond) in contract
        .requires
        .iter()
        .map(|c| ("requires", c))
        .chain(contract.ensures.iter().map(|c| ("ensures", c)))
    {
        let field = model::condition_field(cond).unwrap_or_else(|| "?".to_owned());
        let name = format!("{}_sat_{n}", lean_ident(&field).to_lowercase());
        match model::satisfiability(model, &contract.entity, &name, cond) {
            Some(t) => {
                body.push(format!("-- {section}: {}", cond_comment(cond)));
                body.push(t.text);
                body.push(String::new());
            }
            None => body.push(format!("-- {section} (not modelled): {}", cond_comment(cond))),
        }
        n += 1;
    }
    if n == 0 {
        body.push("-- (no conditions to model)".to_owned());
    }

    body.push(format!("end Tauto.Contracts.{module_name}"));
    body.push(String::new());

    LeanWorkspaceFile {
        path: format!("TautoContracts/contracts/{module_name}.lean"),
        content: body.join("\n"),
    }
}

fn cond_comment(cond: &Condition) -> String {
    format!("{} {} {}", render_expr(&cond.left), cond.operator, render_expr(&cond.right))
}

fn comment_block(header: &str, items: &[String]) -> String {
    let mut lines = vec![format!("-- {header}:")];
    for item in items {
        lines.push(format!("--   {item}"));
    }
    lines.push(String::new());
    lines.join("\n")
}

/// The domain-model file: inferred enum `inductive`s used by every other file.
fn model_file(model: &Model) -> LeanWorkspaceFile {
    LeanWorkspaceFile {
        path: "TautoContracts/Model.lean".to_owned(),
        content: model::render_model(model),
    }
}

/// The conflicts file: for each same-(entity, operation) contract pair, prove
/// the fields that are contradictorily constrained cannot both hold — turning
/// the conflict heuristic into machine-checked theorems. Returns `None` when no
/// pair yields a provable contradiction (so we don't emit an empty import).
fn conflicts_file(contract_set: &ContractSet, model: &Model) -> Option<LeanWorkspaceFile> {
    let mut body = vec![
        "import TautoContracts.Model".to_owned(),
        String::new(),
        "namespace Tauto.Conflicts".to_owned(),
        String::new(),
    ];
    let mut count = 0usize;

    let contracts = &contract_set.contracts;
    for i in 0..contracts.len() {
        for j in (i + 1)..contracts.len() {
            let (a, b) = (&contracts[i], &contracts[j]);
            if a.entity != b.entity || a.operation != b.operation {
                continue;
            }
            // Compare like positions only: guards (requires×requires) and
            // outcomes (ensures×ensures). Crossing a pre-state guard with a
            // post-state outcome on the same field name is meaningless.
            //   - disjoint guards  → the rules are distinct transitions (no conflict)
            //   - contradictory outcomes → a genuine conflict
            for (section, a_conds, b_conds) in [
                ("guards", &a.requires, &b.requires),
                ("outcome", &a.ensures, &b.ensures),
            ] {
                let mut done_fields: Vec<String> = Vec::new();
                for ca in a_conds {
                    let Some(field) = model::condition_field(ca) else { continue };
                    if done_fields.contains(&field) {
                        continue;
                    }
                    for cb in b_conds {
                        if model::condition_field(cb).as_deref() != Some(field.as_str()) {
                            continue;
                        }
                        let name = format!(
                            "{section}_{}_{}_{}",
                            lean_ident(&a.case).to_lowercase(),
                            lean_ident(&b.case).to_lowercase(),
                            lean_ident(&field).to_lowercase()
                        );
                        if let Some(t) = model::conflict_theorem(model, &a.entity, &name, &field, ca, cb) {
                            let note = if section == "guards" {
                                "disjoint guards → distinct transitions, not a conflict"
                            } else {
                                "contradictory outcomes → conflict"
                            };
                            body.push(format!("-- {} vs {} on `{field}`: {note}", a.case, b.case));
                            body.push(t.text);
                            body.push(String::new());
                            done_fields.push(field.clone());
                            count += 1;
                            break;
                        }
                    }
                }
            }
        }
    }

    if count == 0 {
        return None;
    }
    body.push("end Tauto.Conflicts".to_owned());
    body.push(String::new());
    Some(LeanWorkspaceFile {
        path: "TautoContracts/Conflicts.lean".to_owned(),
        content: body.join("\n"),
    })
}

fn main_module_file(module_names: &[String], has_conflicts: bool) -> LeanWorkspaceFile {
    let mut lines = vec![
        "-- Auto-generated import index".to_owned(),
        String::new(),
        "import TautoContracts.Model".to_owned(),
    ];
    for name in module_names {
        lines.push(format!("import TautoContracts.contracts.{name}"));
    }
    if has_conflicts {
        lines.push("import TautoContracts.Conflicts".to_owned());
    }
    lines.push(String::new());
    LeanWorkspaceFile {
        path: "TautoContracts.lean".to_owned(),
        content: lines.join("\n"),
    }
}

fn lakefile() -> LeanWorkspaceFile {
    // Lake's lakefile.toml takes the package keys (name, version, …) at the top
    // level — there is no `[package]` table. Recent Lake rejects the old form
    // with "missing required key: name".
    let content = r#"# Auto-generated
name = "TautoContracts"
version = "0.1.0"
defaultTargets = ["TautoContracts"]

[[lean_lib]]
name = "TautoContracts"
"#;
    LeanWorkspaceFile { path: "lakefile.toml".to_owned(), content: content.to_owned() }
}

pub fn generate_lean_workspace(contract_set: &ContractSet) -> LeanWorkspace {
    let model = Model::infer(contract_set);
    let module_names = assign_module_names(&contract_set.contracts);
    let contract_files: Vec<LeanWorkspaceFile> = contract_set
        .contracts
        .iter()
        .zip(module_names.iter())
        .map(|(c, name)| contract_file(c, name, &model))
        .collect();
    let conflicts = conflicts_file(contract_set, &model);
    let main = main_module_file(&module_names, conflicts.is_some());
    let lake = lakefile();
    let mut files = vec![lake, main, model_file(&model)];
    files.extend(contract_files);
    if let Some(c) = conflicts {
        files.push(c);
    }
    LeanWorkspace { files }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract_ir::{Condition, ContractIR, ContractSet, Expression, ExpressionValue};

    fn str_expr(kind: &str, value: &str) -> Expression {
        Expression { kind: kind.to_owned(), value: ExpressionValue::Str(value.to_owned()) }
    }

    fn cond(left: &str, op: &str, right: &str) -> Condition {
        Condition {
            left: str_expr("field", left),
            operator: op.to_owned(),
            right: str_expr("enum", right),
        }
    }

    #[test]
    fn enum_expr_renders_with_dot_prefix() {
        let expr = Expression { kind: "enum".to_owned(), value: ExpressionValue::Str("Paid".to_owned()) };
        assert_eq!(render_expr(&expr), ".Paid");
    }

    #[test]
    fn field_expr_renders_without_dot_prefix() {
        let expr = Expression { kind: "field".to_owned(), value: ExpressionValue::Str("order.status".to_owned()) };
        assert_eq!(render_expr(&expr), "order.status");
    }

    #[test]
    fn bool_expr_renders_lowercase() {
        let t = Expression { kind: "bool".to_owned(), value: ExpressionValue::Bool(true) };
        let f = Expression { kind: "bool".to_owned(), value: ExpressionValue::Bool(false) };
        assert_eq!(render_expr(&t), "true");
        assert_eq!(render_expr(&f), "false");
    }

    #[test]
    fn int_expr_renders_as_number() {
        let expr = Expression { kind: "int".to_owned(), value: ExpressionValue::Int(42) };
        assert_eq!(render_expr(&expr), "42");
    }

    #[test]
    fn theorem_comment_uses_dot_notation_for_enum_values() {
        let ws = generate_lean_workspace(&rich_set());
        let f = ws.files.iter().find(|f| f.path.starts_with("TautoContracts/contracts/")).unwrap();
        // enum rhs values must use Lean 4 dot-notation
        assert!(f.content.contains(".Paid") || f.content.contains(".Cancelled"),
            "enum values must render with dot prefix, got:\n{}", f.content);
    }

    fn minimal_set() -> ContractSet {
        ContractSet::new(vec![ContractIR::new("CancelPaidOrder", "Order", "cancelOrder")])
    }

    fn rich_set() -> ContractSet {
        ContractSet::new(vec![ContractIR {
            case: "CancelPaidOrder".to_owned(),
            entity: "Order".to_owned(),
            operation: "cancelOrder".to_owned(),
            requires: vec![cond("order.status", "==", "Paid")],
            ensures: vec![cond("result.status", "==", "Cancelled")],
            forbidden: vec![],
            preserves: vec![],
            assumes: vec![],
            intent: None, examples: Vec::new(), source: None,
        }])
    }

    #[test]
    fn workspace_contains_lakefile() {
        let ws = generate_lean_workspace(&minimal_set());
        assert!(ws.files.iter().any(|f| f.path == "lakefile.toml"));
    }

    #[test]
    fn lakefile_uses_toml_comment_syntax() {
        let ws = generate_lean_workspace(&minimal_set());
        let lake = ws.files.iter().find(|f| f.path == "lakefile.toml").unwrap();
        assert!(lake.content.starts_with('#'), "TOML comments must use #, not --");
    }

    #[test]
    fn lakefile_has_top_level_name_key() {
        // Lake requires the package `name` at the document top level; a
        // `[package]` table makes Lake fail with "missing required key: name".
        let ws = generate_lean_workspace(&minimal_set());
        let lake = ws.files.iter().find(|f| f.path == "lakefile.toml").unwrap();
        assert!(
            lake.content.lines().any(|l| l.trim_start().starts_with("name =")),
            "lakefile.toml must declare a top-level name key"
        );
        assert!(!lake.content.contains("[package]"), "lakefile.toml must not use a [package] table");
    }

    #[test]
    fn workspace_contains_main_module_file() {
        let ws = generate_lean_workspace(&minimal_set());
        assert!(ws.files.iter().any(|f| f.path == "TautoContracts.lean"));
    }

    #[test]
    fn workspace_contains_contract_file_per_contract() {
        let ws = generate_lean_workspace(&minimal_set());
        let contract_files: Vec<_> = ws
            .files
            .iter()
            .filter(|f| f.path.starts_with("TautoContracts/contracts/") && f.path.ends_with(".lean"))
            .collect();
        assert_eq!(contract_files.len(), 1);
    }

    #[test]
    fn contract_file_path_uses_contracts_prefix() {
        let ws = generate_lean_workspace(&minimal_set());
        let f = ws.files.iter().find(|f| f.path.starts_with("TautoContracts/contracts/")).unwrap();
        assert!(f.path.ends_with(".lean"));
        // The file path must correspond to the module the main index imports:
        // TautoContracts/contracts/<name>.lean  <->  import TautoContracts.contracts.<name>
        let module = f.path.trim_end_matches(".lean").replace('/', ".");
        let main = ws.files.iter().find(|f| f.path == "TautoContracts.lean").unwrap();
        assert!(
            main.content.contains(&format!("import {module}")),
            "main index must import the contract module; path={} module={module}\n{}",
            f.path, main.content
        );
    }

    #[test]
    fn contract_file_contains_namespace() {
        let ws = generate_lean_workspace(&minimal_set());
        let f = ws.files.iter().find(|f| f.path.starts_with("TautoContracts/contracts/")).unwrap();
        assert!(f.content.contains("namespace Tauto.Contracts."));
        assert!(f.content.contains("end Tauto.Contracts."));
    }

    #[test]
    fn rich_contract_produces_real_satisfiability_theorems() {
        let ws = generate_lean_workspace(&rich_set());
        let f = ws.files.iter().find(|f| f.path.starts_with("TautoContracts/contracts/")).unwrap();
        // One satisfiability theorem per modelable condition (1 requires + 1 ensures).
        assert_eq!(f.content.matches("theorem ").count(), 2);
        // Real proofs, not stubs.
        assert!(!f.content.contains("sorry"));
        assert!(f.content.contains("∃ x :"));
    }

    #[test]
    fn generated_workspace_is_sorry_free() {
        // The whole point of the rewrite: real, discharged proofs — no `sorry`
        // and no vacuous `True` obligations anywhere in the workspace.
        let ws = generate_lean_workspace(&rich_set());
        for f in &ws.files {
            assert!(!f.content.contains("sorry"), "unexpected sorry in {}", f.path);
            assert!(!f.content.contains(": True"), "unexpected vacuous True in {}", f.path);
        }
    }

    #[test]
    fn distinct_nested_fields_do_not_false_conflict() {
        // billing.status vs shipping.status share the leaf `status` but are
        // different fields — must NOT emit a conflict theorem.
        let mk = |case: &str, path: &str, val: &str| ContractIR {
            case: case.to_owned(),
            entity: "Order".to_owned(),
            operation: "ship".to_owned(),
            requires: vec![],
            ensures: vec![cond(path, "==", val)],
            forbidden: vec![],
            preserves: vec![],
            assumes: vec![],
            intent: None, examples: Vec::new(), source: None,
        };
        let ws = generate_lean_workspace(&ContractSet::new(vec![
            mk("A", "result.billing.status", "Approved"),
            mk("B", "result.shipping.status", "Rejected"),
        ]));
        // No Conflicts.lean because the fields differ.
        assert!(ws.files.iter().all(|f| !f.path.ends_with("Conflicts.lean")));
    }

    #[test]
    fn contradictory_pair_yields_conflict_theorem() {
        // Two same-op contracts with contradictory ensures on `status`.
        let mk = |case: &str, val: &str| ContractIR {
            case: case.to_owned(),
            entity: "Order".to_owned(),
            operation: "ship".to_owned(),
            requires: vec![],
            ensures: vec![cond("result.status", "==", val)],
            forbidden: vec![],
            preserves: vec![],
            assumes: vec![],
            intent: None, examples: Vec::new(), source: None,
        };
        let ws = generate_lean_workspace(&ContractSet::new(vec![mk("A", "Shipped"), mk("B", "Rejected")]));
        let conflicts = ws.files.iter().find(|f| f.path.ends_with("Conflicts.lean"));
        let c = conflicts.expect("expected a Conflicts.lean");
        assert!(c.content.contains("¬ (x = .Shipped ∧ x = .Rejected)"));
        assert!(!c.content.contains("sorry"));
    }

    #[test]
    fn digit_leading_case_gets_c_prefix() {
        let cs = ContractSet::new(vec![ContractIR::new("1InvalidStart", "E", "op")]);
        let ws = generate_lean_workspace(&cs);
        let f = ws.files.iter().find(|f| f.path.starts_with("TautoContracts/contracts/")).unwrap();
        assert!(f.path.starts_with("TautoContracts/contracts/C"), "digit-leading names must be prefixed with C");
    }

    #[test]
    fn colliding_cases_get_numeric_suffix() {
        let cs = ContractSet::new(vec![
            ContractIR::new("A-B", "E", "op"),
            ContractIR::new("AB", "E", "op"),
        ]);
        let ws = generate_lean_workspace(&cs);
        let paths: Vec<_> = ws
            .files
            .iter()
            .filter(|f| f.path.starts_with("TautoContracts/contracts/"))
            .map(|f| &f.path)
            .collect();
        assert_eq!(paths.len(), 2);
        let unique: std::collections::HashSet<_> = paths.iter().collect();
        assert_eq!(unique.len(), 2, "colliding identifiers must produce unique paths");
    }

    #[test]
    fn empty_case_gets_contract_fallback() {
        let cs = ContractSet::new(vec![ContractIR::new("---", "E", "op")]);
        let ws = generate_lean_workspace(&cs);
        let f = ws.files.iter().find(|f| f.path.starts_with("TautoContracts/contracts/")).unwrap();
        assert!(f.path.contains("Contract"), "empty-after-sanitize must fall back to 'Contract'");
    }

    // lean_ident strips underscores, so disambiguation suffixes (_1, _2, …) can never
    // be produced by any case string — the suffix namespace is disjoint from the base namespace.
    #[test]
    fn suffix_namespace_is_disjoint_from_base_namespace() {
        // "AB_1" sanitizes to "AB1", not "AB_1", so it cannot collide with the
        // disambiguation suffix produced for a second "AB".
        let cs = ContractSet::new(vec![
            ContractIR::new("AB", "E", "op"),
            ContractIR::new("AB", "E", "op"),
            ContractIR::new("AB_1", "E", "op"),  // lean_ident → "AB1", not "AB_1"
        ]);
        let ws = generate_lean_workspace(&cs);
        let paths: Vec<_> =
            ws.files.iter().filter(|f| f.path.starts_with("TautoContracts/contracts/")).collect();
        assert_eq!(paths.len(), 3);
        let unique: std::collections::HashSet<_> = paths.iter().map(|f| &f.path).collect();
        assert_eq!(unique.len(), 3, "all three paths must be distinct");
    }

    #[test]
    fn preserves_rendered_as_comment_block() {
        let cs = ContractSet::new(vec![ContractIR {
            case: "Foo".to_owned(),
            entity: "E".to_owned(),
            operation: "op".to_owned(),
            requires: vec![],
            ensures: vec![],
            forbidden: vec![],
            preserves: vec!["order.history.size > 0".to_owned()],
            assumes: vec![],
            intent: None, examples: Vec::new(), source: None,
        }]);
        let ws = generate_lean_workspace(&cs);
        let f = ws.files.iter().find(|f| f.path.starts_with("TautoContracts/contracts/")).unwrap();
        assert!(f.content.contains("-- Preserved invariants:"));
        assert!(f.content.contains("--   order.history.size > 0"));
    }

    #[test]
    fn assumes_rendered_as_comment_block() {
        let cs = ContractSet::new(vec![ContractIR {
            case: "Foo".to_owned(),
            entity: "E".to_owned(),
            operation: "op".to_owned(),
            requires: vec![],
            ensures: vec![],
            forbidden: vec![],
            preserves: vec![],
            assumes: vec!["payment.verified == true".to_owned()],
            intent: None, examples: Vec::new(), source: None,
        }]);
        let ws = generate_lean_workspace(&cs);
        let f = ws.files.iter().find(|f| f.path.starts_with("TautoContracts/contracts/")).unwrap();
        assert!(f.content.contains("-- Assumed preconditions:"));
        assert!(f.content.contains("--   payment.verified == true"));
    }
}

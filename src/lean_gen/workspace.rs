use crate::contract_ir::{Condition, ContractIR, ContractSet, Expression, ExpressionValue};

#[derive(Debug, Clone, PartialEq)]
pub struct LeanWorkspaceFile {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq)]
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
    match &expr.value {
        ExpressionValue::Str(s) => s.clone(),
        ExpressionValue::Int(n) => n.to_string(),
        ExpressionValue::Bool(b) => b.to_string(),
    }
}

fn theorem_stub(kind: &str, conditions: &[Condition], op_ident: &str) -> String {
    let name = format!("{op_ident}_{kind}");
    let mut lines = vec![
        format!("theorem {name} :"),
        "  -- TODO: formalize the following conditions".to_owned(),
    ];
    for cond in conditions {
        lines.push(format!(
            "  --   {} {} {}",
            render_expr(&cond.left),
            cond.operator,
            render_expr(&cond.right)
        ));
    }
    lines.push("  True := by".to_owned());
    lines.push("  sorry".to_owned());
    lines.push(String::new());
    lines.join("\n")
}

fn comment_block(header: &str, items: &[String]) -> String {
    let mut lines = vec![format!("-- {header}:")];
    for item in items {
        lines.push(format!("--   {item}"));
    }
    lines.push(String::new());
    lines.join("\n")
}

fn contract_file(contract: &ContractIR, module_name: &str) -> LeanWorkspaceFile {
    let op_ident = lean_ident(&contract.operation);
    let mut body = vec![
        format!("namespace Tauto.Contracts.{module_name}"),
        String::new(),
    ];
    if !contract.assumes.is_empty() {
        body.push(comment_block("Assumed preconditions", &contract.assumes));
    }
    if !contract.preserves.is_empty() {
        body.push(comment_block("Preserved invariants", &contract.preserves));
    }
    if !contract.requires.is_empty() {
        body.push(theorem_stub("requires", &contract.requires, &op_ident));
    }
    if !contract.ensures.is_empty() {
        body.push(theorem_stub("ensures", &contract.ensures, &op_ident));
    }
    body.push(format!("end Tauto.Contracts.{module_name}"));
    body.push(String::new());

    LeanWorkspaceFile {
        path: format!("contracts/{module_name}.lean"),
        content: body.join("\n"),
    }
}

fn main_module_file(module_names: &[String]) -> LeanWorkspaceFile {
    let mut lines = vec!["-- Auto-generated import index".to_owned(), String::new()];
    for name in module_names {
        lines.push(format!("import TautoContracts.contracts.{name}"));
    }
    lines.push(String::new());
    LeanWorkspaceFile {
        path: "TautoContracts.lean".to_owned(),
        content: lines.join("\n"),
    }
}

fn lakefile() -> LeanWorkspaceFile {
    let content = r#"# Auto-generated
[package]
name = "TautoContracts"
version = "0.1.0"
defaultTargets = ["TautoContracts"]

[[lean_lib]]
name = "TautoContracts"
"#;
    LeanWorkspaceFile { path: "lakefile.toml".to_owned(), content: content.to_owned() }
}

pub fn generate_lean_workspace(contract_set: &ContractSet) -> LeanWorkspace {
    let module_names = assign_module_names(&contract_set.contracts);
    let contract_files: Vec<LeanWorkspaceFile> = contract_set
        .contracts
        .iter()
        .zip(module_names.iter())
        .map(|(c, name)| contract_file(c, name))
        .collect();
    let main = main_module_file(&module_names);
    let lake = lakefile();
    let mut files = vec![lake, main];
    files.extend(contract_files);
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
            source: None,
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
            .filter(|f| f.path.starts_with("contracts/") && f.path.ends_with(".lean"))
            .collect();
        assert_eq!(contract_files.len(), 1);
    }

    #[test]
    fn contract_file_path_uses_contracts_prefix() {
        let ws = generate_lean_workspace(&minimal_set());
        let f = ws.files.iter().find(|f| f.path.starts_with("contracts/")).unwrap();
        assert!(f.path.starts_with("contracts/"));
        assert!(!f.path.contains("TautoContracts"));
    }

    #[test]
    fn contract_file_contains_namespace() {
        let ws = generate_lean_workspace(&minimal_set());
        let f = ws.files.iter().find(|f| f.path.starts_with("contracts/")).unwrap();
        assert!(f.content.contains("namespace Tauto.Contracts."));
        assert!(f.content.contains("end Tauto.Contracts."));
    }

    #[test]
    fn rich_contract_produces_two_theorem_stubs() {
        let ws = generate_lean_workspace(&rich_set());
        let f = ws.files.iter().find(|f| f.path.starts_with("contracts/")).unwrap();
        let sorry_count = f.content.matches("sorry").count();
        assert_eq!(sorry_count, 2);
    }

    #[test]
    fn theorem_stub_contains_sorry() {
        let ws = generate_lean_workspace(&rich_set());
        let f = ws.files.iter().find(|f| f.path.starts_with("contracts/")).unwrap();
        assert!(f.content.contains("sorry"));
    }

    #[test]
    fn digit_leading_case_gets_c_prefix() {
        let cs = ContractSet::new(vec![ContractIR::new("1InvalidStart", "E", "op")]);
        let ws = generate_lean_workspace(&cs);
        let f = ws.files.iter().find(|f| f.path.starts_with("contracts/")).unwrap();
        assert!(f.path.starts_with("contracts/C"), "digit-leading names must be prefixed with C");
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
            .filter(|f| f.path.starts_with("contracts/"))
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
        let f = ws.files.iter().find(|f| f.path.starts_with("contracts/")).unwrap();
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
            ws.files.iter().filter(|f| f.path.starts_with("contracts/")).collect();
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
            source: None,
        }]);
        let ws = generate_lean_workspace(&cs);
        let f = ws.files.iter().find(|f| f.path.starts_with("contracts/")).unwrap();
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
            source: None,
        }]);
        let ws = generate_lean_workspace(&cs);
        let f = ws.files.iter().find(|f| f.path.starts_with("contracts/")).unwrap();
        assert!(f.content.contains("-- Assumed preconditions:"));
        assert!(f.content.contains("--   payment.verified == true"));
    }
}

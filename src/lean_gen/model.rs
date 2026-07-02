//! Lowering business-rule conditions to real, machine-checkable Lean 4.
//!
//! We cannot prove "requires ⇒ ensures" — tauto has the contract, not the
//! operation's implementation, so there is no transition to reason about. What
//! we CAN prove from the spec alone, with no `sorry`, is:
//!
//!   * **satisfiability** of an individual condition (`∃ x, x = .Paid`), and
//!   * **contradiction / disjointness** between two conditions on the same field
//!     (`∀ x, ¬(x = .Paid ∧ x = .Unpaid)`), which is exactly what confirms (or
//!     refutes) a conflict.
//!
//! Enum types are inferred from the uppercase values used across a field's
//! conditions, so this works even without a glossary. Every theorem is
//! discharged by `rfl` / `decide` / `omega`.

use std::collections::BTreeMap;

use crate::contract_ir::{Condition, ContractSet, ExpressionValue};

/// The inferred kind of a field, keyed below by `(entity, field-name)`.
#[derive(Debug, Clone, PartialEq)]
pub enum FieldKind {
    /// Enum with its members in first-seen (then sorted) order.
    Enum(Vec<String>),
    Int,
    Bool,
}

/// The domain model inferred from a contract set: the kind (and, for enums, the
/// members) of every field referenced in a condition.
#[derive(Debug, Clone, Default)]
pub struct Model {
    /// `(entity, field)` → kind.
    fields: BTreeMap<(String, String), FieldKind>,
}

/// The bare field name of a condition's left side (`order.status` → `status`,
/// `status` → `status`). `None` if the left side is not a field path.
/// The field path with its leading instance/result prefix stripped, so
/// `order.status` and `result.status` both key to `status`, but `billing.status`
/// and `shipping.status` stay distinct. Using the full remaining path (not just
/// the last segment) prevents false conflicts between different nested fields
/// that happen to share a leaf name.
fn field_name(cond: &Condition) -> Option<String> {
    let ExpressionValue::Str(path) = &cond.left.value else {
        return None;
    };
    match path.split_once('.') {
        Some((_prefix, rest)) => Some(rest.to_owned()),
        None => Some(path.clone()),
    }
}

/// True for an identifier that starts uppercase — how the DSL distinguishes an
/// enum member (`Paid`) from a field path.
fn is_enum_value(v: &ExpressionValue) -> Option<&str> {
    match v {
        ExpressionValue::Str(s) if s.chars().next().is_some_and(|c| c.is_ascii_uppercase()) => {
            Some(s)
        }
        _ => None,
    }
}

impl Model {
    /// Infer field kinds from every condition in the set.
    pub fn infer(cs: &ContractSet) -> Self {
        let mut enums: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();
        let mut kinds: BTreeMap<(String, String), FieldKind> = BTreeMap::new();
        for c in &cs.contracts {
            for cond in c.requires.iter().chain(c.ensures.iter()) {
                let Some(field) = field_name(cond) else { continue };
                let key = (c.entity.clone(), field);
                match &cond.right.value {
                    v if is_enum_value(v).is_some() => {
                        let val = is_enum_value(v).unwrap().to_owned();
                        let set = enums.entry(key.clone()).or_default();
                        if !set.contains(&val) {
                            set.push(val);
                        }
                        kinds.insert(key, FieldKind::Enum(vec![]));
                    }
                    ExpressionValue::Int(_) => {
                        kinds.entry(key).or_insert(FieldKind::Int);
                    }
                    ExpressionValue::Bool(_) => {
                        kinds.entry(key).or_insert(FieldKind::Bool);
                    }
                    // A string that isn't an enum member (lowercase) is a field
                    // path on the RHS — not modelled.
                    ExpressionValue::Str(_) => {}
                }
            }
        }
        // Fold the collected enum members into the kinds (sorted for stability).
        let mut fields = kinds;
        for (key, mut members) in enums {
            members.sort();
            members.dedup();
            fields.insert(key, FieldKind::Enum(members));
        }
        Model { fields }
    }

    pub fn kind(&self, entity: &str, field: &str) -> Option<&FieldKind> {
        self.fields.get(&(entity.to_owned(), field.to_owned()))
    }

    /// All enum types, as `(lean_type_name, members)`, sorted.
    pub fn enum_types(&self) -> Vec<(String, Vec<String>)> {
        self.fields
            .iter()
            .filter_map(|((entity, field), kind)| match kind {
                FieldKind::Enum(m) if !m.is_empty() => {
                    Some((enum_type_name(entity, field), m.clone()))
                }
                _ => None,
            })
            .collect()
    }
}

/// Lean type name for an entity's enum field, e.g. `Order` + `status` → `OrderStatus`.
pub fn enum_type_name(entity: &str, field: &str) -> String {
    format!("{}{}", pascal(entity), pascal(field))
}

fn pascal(s: &str) -> String {
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|p| !p.is_empty())
        .map(|p| {
            let mut ch = p.chars();
            match ch.next() {
                Some(f) => f.to_ascii_uppercase().to_string() + &ch.as_str().to_lowercase(),
                None => String::new(),
            }
        })
        .collect()
}

// ── Lean lowering ──────────────────────────────────────────────────────────────

fn lean_cmp(op: &str) -> &'static str {
    match op {
        ">=" => "≥",
        "<=" => "≤",
        ">" => ">",
        "<" => "<",
        "!=" => "≠",
        _ => "=",
    }
}

/// A theorem with a unique name and a body; assembled into files by workspace.rs.
#[derive(Debug, Clone, PartialEq)]
pub struct Theorem {
    pub name: String,
    pub text: String,
}

/// The Lean `inductive` declarations for every inferred enum type.
pub fn render_model(model: &Model) -> String {
    let mut out = vec!["-- Auto-generated domain model (inferred enum types)".to_owned(), String::new()];
    for (ty, members) in model.enum_types() {
        out.push(format!("inductive {ty} where"));
        for m in &members {
            out.push(format!("  | {m}"));
        }
        out.push("  deriving DecidableEq, Repr".to_owned());
        out.push(String::new());
    }
    out.join("\n")
}

/// Enum member of `ty` other than `exclude`, for witnessing a `≠` guard.
fn other_member(model: &Model, entity: &str, field: &str, exclude: &str) -> Option<String> {
    match model.kind(entity, field)? {
        FieldKind::Enum(m) => m.iter().find(|v| *v != exclude).cloned(),
        _ => None,
    }
}

/// A satisfiability theorem for one condition: `∃ x : T, <cond>` proven by a
/// concrete witness. Returns `None` for conditions we can't model (or a `≠`
/// enum guard on a single-member type).
pub fn satisfiability(
    model: &Model,
    entity: &str,
    name: &str,
    cond: &Condition,
) -> Option<Theorem> {
    let field = field_name(cond)?;
    let kind = model.kind(entity, &field)?;
    let op = cond.operator.as_str();
    let (ty, prop, witness) = match kind {
        FieldKind::Enum(_) => {
            let v = is_enum_value(&cond.right.value)?;
            let ty = enum_type_name(entity, &field);
            match op {
                "==" => (ty, format!("x = .{v}"), format!("⟨.{v}, rfl⟩")),
                "!=" => {
                    let w = other_member(model, entity, &field, v)?;
                    (ty, format!("x ≠ .{v}"), format!("⟨.{w}, by decide⟩"))
                }
                _ => return None,
            }
        }
        FieldKind::Int => {
            let ExpressionValue::Int(n) = &cond.right.value else { return None };
            let cmp = lean_cmp(op);
            let witness = match op {
                ">" => format!("({n} + 1)"),
                "<" => format!("({n} - 1)"),
                "!=" => format!("({n} + 1)"),
                _ => n.to_string(),
            };
            ("Int".to_owned(), format!("x {cmp} {n}"), format!("⟨{witness}, by omega⟩"))
        }
        FieldKind::Bool => {
            let ExpressionValue::Bool(b) = &cond.right.value else { return None };
            let ty = "Bool".to_owned();
            match op {
                "==" => (ty, format!("x = {b}"), format!("⟨{b}, rfl⟩")),
                "!=" => (ty, format!("x ≠ {b}"), format!("⟨{}, by decide⟩", !b)),
                _ => return None,
            }
        }
    };
    Some(Theorem {
        name: name.to_owned(),
        text: format!("theorem {name} : ∃ x : {ty}, {prop} := {witness}"),
    })
}

/// Whether two conditions on the same field are contradictory, and the Lean
/// tactic that refutes their conjunction. Mirrors the IR conflict heuristic, but
/// as a real proof.
fn contradiction(a: &Condition, b: &Condition, kind: &FieldKind) -> Option<(String, String, String)> {
    let (oa, ob) = (a.operator.as_str(), b.operator.as_str());
    match kind {
        FieldKind::Enum(_) => {
            let va = is_enum_value(&a.right.value)?;
            let vb = is_enum_value(&b.right.value)?;
            match (oa, ob) {
                ("==", "==") if va != vb => Some((
                    format!("x = .{va}"),
                    format!("x = .{vb}"),
                    "by intro x ⟨h1, h2⟩; subst h1; exact absurd h2 (by decide)".to_owned(),
                )),
                ("==", "!=") | ("!=", "==") if va == vb => {
                    let (eq, ne) = if oa == "==" { (va, vb) } else { (vb, va) };
                    Some((
                        format!("x = .{eq}"),
                        format!("x ≠ .{ne}"),
                        "by intro x ⟨h1, h2⟩; exact absurd h1 h2".to_owned(),
                    ))
                }
                _ => None,
            }
        }
        FieldKind::Int => {
            // Any int pair whose conjunction is linearly unsatisfiable → omega.
            let (ExpressionValue::Int(na), ExpressionValue::Int(nb)) =
                (&a.right.value, &b.right.value)
            else {
                return None;
            };
            let contradictory = match (oa, ob) {
                (">", "<=") | ("<=", ">") | ("<", ">=") | (">=", "<") => na == nb,
                ("==", "==") | ("==", "!=") | ("!=", "==") => (oa == "==" && ob == "==" && na != nb)
                    || (oa != ob && na == nb),
                _ => false,
            };
            if !contradictory {
                return None;
            }
            Some((
                format!("x {} {na}", lean_cmp(oa)),
                format!("x {} {nb}", lean_cmp(ob)),
                "by intro x ⟨h1, h2⟩; omega".to_owned(),
            ))
        }
        FieldKind::Bool => {
            let (ExpressionValue::Bool(ba), ExpressionValue::Bool(bb)) =
                (&a.right.value, &b.right.value)
            else {
                return None;
            };
            match (oa, ob) {
                ("==", "==") if ba != bb => Some((
                    format!("x = {ba}"),
                    format!("x = {bb}"),
                    "by intro x ⟨h1, h2⟩; subst h1; exact absurd h2 (by decide)".to_owned(),
                )),
                ("==", "!=") | ("!=", "==") if ba == bb => {
                    let (eq, ne) = if oa == "==" { (ba, bb) } else { (bb, ba) };
                    Some((
                        format!("x = {eq}"),
                        format!("x ≠ {ne}"),
                        "by intro x ⟨h1, h2⟩; exact absurd h1 h2".to_owned(),
                    ))
                }
                _ => None,
            }
        }
    }
}

/// One conflict/disjointness theorem: for a field constrained contradictorily by
/// two contracts, prove `∀ x : T, ¬ (propA ∧ propB)`.
pub fn conflict_theorem(
    model: &Model,
    entity: &str,
    name: &str,
    field: &str,
    a: &Condition,
    b: &Condition,
) -> Option<Theorem> {
    let kind = model.kind(entity, field)?;
    let (pa, pb, tac) = contradiction(a, b, kind)?;
    let ty = match kind {
        FieldKind::Enum(_) => enum_type_name(entity, field),
        FieldKind::Int => "Int".to_owned(),
        FieldKind::Bool => "Bool".to_owned(),
    };
    Some(Theorem {
        name: name.to_owned(),
        text: format!("theorem {name} : ∀ x : {ty}, ¬ ({pa} ∧ {pb}) := {tac}"),
    })
}

/// The bare field name of a condition (public for workspace.rs pairing).
pub fn condition_field(cond: &Condition) -> Option<String> {
    field_name(cond)
}

/// Render a single condition as a Lean prop over the bound variable `x`.
fn dead_prop(kind: &FieldKind, cond: &Condition) -> Option<String> {
    let op = cond.operator.as_str();
    match kind {
        FieldKind::Enum(_) => {
            let v = is_enum_value(&cond.right.value)?;
            match op {
                "==" => Some(format!("x = .{v}")),
                "!=" => Some(format!("x ≠ .{v}")),
                _ => None,
            }
        }
        FieldKind::Int => {
            let ExpressionValue::Int(n) = &cond.right.value else { return None };
            Some(format!("x {} {n}", lean_cmp(op)))
        }
        FieldKind::Bool => {
            let ExpressionValue::Bool(b) = &cond.right.value else { return None };
            match op {
                "==" => Some(format!("x = {b}")),
                "!=" => Some(format!("x ≠ {b}")),
                _ => None,
            }
        }
    }
}

/// A machine-checked proof that a rule's two contradictory requires cannot both
/// hold: `∀ x : T, ¬ (a ∧ b)`. The Rust dead-rule detector already decided the
/// pair is unsatisfiable; this discharges it (omega for int, cases+simp for the
/// finite enum/bool types — no Fintype/Mathlib needed).
pub fn dead_rule_theorem(
    model: &Model,
    entity: &str,
    name: &str,
    field: &str,
    a: &Condition,
    b: &Condition,
) -> Option<Theorem> {
    let kind = model.kind(entity, field)?;
    let pa = dead_prop(kind, a)?;
    let pb = dead_prop(kind, b)?;
    let (ty, tac) = match kind {
        FieldKind::Int => ("Int".to_owned(), "by intro x ⟨h1, h2⟩; omega"),
        FieldKind::Enum(_) => {
            (enum_type_name(entity, field), "by intro x ⟨h1, h2⟩; cases x <;> simp_all")
        }
        FieldKind::Bool => ("Bool".to_owned(), "by intro x ⟨h1, h2⟩; cases x <;> simp_all"),
    };
    Some(Theorem {
        name: name.to_owned(),
        text: format!("theorem {name} : ∀ x : {ty}, ¬ ({pa} ∧ {pb}) := {tac}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract_ir::{ContractIR, Expression, ExpressionValue};

    fn cond(left: &str, op: &str, right: ExpressionValue) -> Condition {
        Condition {
            left: Expression { kind: "field".into(), value: ExpressionValue::Str(left.into()) },
            operator: op.into(),
            right: Expression { kind: "v".into(), value: right },
        }
    }

    fn order(case: &str, req: Vec<Condition>, ens: Vec<Condition>) -> ContractIR {
        let mut c = ContractIR::new(case, "Order", "ship");
        c.requires = req;
        c.ensures = ens;
        c
    }

    #[test]
    fn infers_enum_members_across_pre_and_post() {
        let cs = ContractSet::new(vec![
            order("A", vec![cond("order.status", "==", ExpressionValue::Str("Paid".into()))],
                       vec![cond("result.status", "==", ExpressionValue::Str("Shipped".into()))]),
            order("B", vec![cond("order.status", "==", ExpressionValue::Str("Unpaid".into()))],
                       vec![cond("result.status", "==", ExpressionValue::Str("Rejected".into()))]),
        ]);
        let m = Model::infer(&cs);
        let FieldKind::Enum(members) = m.kind("Order", "status").unwrap() else { panic!() };
        assert_eq!(members, &["Paid", "Rejected", "Shipped", "Unpaid"]);
    }

    #[test]
    fn infers_int_and_bool() {
        let cs = ContractSet::new(vec![order(
            "A",
            vec![
                cond("loan.credit_score", ">=", ExpressionValue::Int(750)),
                cond("loan.verified", "==", ExpressionValue::Bool(true)),
            ],
            vec![],
        )]);
        let m = Model::infer(&cs);
        assert_eq!(m.kind("Order", "credit_score"), Some(&FieldKind::Int));
        assert_eq!(m.kind("Order", "verified"), Some(&FieldKind::Bool));
    }

    #[test]
    fn enum_type_name_is_pascal() {
        assert_eq!(enum_type_name("Order", "status"), "OrderStatus");
        assert_eq!(enum_type_name("Mortgage", "interest_rate"), "MortgageInterestRate");
    }
}

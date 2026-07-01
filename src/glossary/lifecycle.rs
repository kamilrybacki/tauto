use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::contract_ir::{ContractIR, ContractSet, ExpressionValue};
use crate::glossary::models::Glossary;

/// One guarded transition derived from a contract: the source state it guards on
/// (from `requires <alias>.<state> == X`) and the target state it produces (from
/// `ensures result.<state> == Y`). Either end may be absent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Transition {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    pub contract: String,
}

/// Lifecycle coverage for one (entity, state field): the declared state domain,
/// the transitions the rules define over it, and coverage gaps.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StateCoverage {
    pub entity: String,
    pub state_field: String,
    /// The declared state domain (enum members), in declaration order.
    pub states: Vec<String>,
    pub transitions: Vec<Transition>,
    /// Declared states that are never a transition target (candidate initial /
    /// unreachable states).
    pub no_incoming: Vec<String>,
    /// Declared states that are never a transition source (candidate terminal /
    /// dead-end states).
    pub no_outgoing: Vec<String>,
    /// Declared states touched by no transition at all — a likely gap.
    pub isolated: Vec<String>,
    /// State values used by a transition but not in the declared domain (typo or
    /// a state missing from the glossary — completable from data, cf. L4).
    pub undeclared_states: Vec<String>,
}

/// Analyze the rule set against the glossary, producing one coverage report per
/// entity state field. Entities without state fields are skipped, as is an empty
/// glossary.
pub fn analyze(contract_set: &ContractSet, glossary: &Glossary) -> Vec<StateCoverage> {
    let mut reports = Vec::new();
    for entity in &glossary.entities {
        for state in entity.state_fields() {
            reports.push(analyze_field(contract_set, entity, &state.name, &state.enum_values));
        }
    }
    reports
}

fn analyze_field(
    contract_set: &ContractSet,
    entity: &crate::glossary::models::EntityDef,
    state_field: &str,
    declared: &[String],
) -> StateCoverage {
    let transitions: Vec<Transition> = contract_set
        .contracts
        .iter()
        .filter(|c| c.entity == entity.name)
        .filter_map(|c| transition_for(c, entity, state_field))
        .collect();

    let sources: BTreeSet<&str> =
        transitions.iter().filter_map(|t| t.from.as_deref()).collect();
    let targets: BTreeSet<&str> =
        transitions.iter().filter_map(|t| t.to.as_deref()).collect();
    let declared_set: BTreeSet<&str> = declared.iter().map(String::as_str).collect();

    let no_incoming: Vec<String> = declared
        .iter()
        .filter(|s| !targets.contains(s.as_str()))
        .cloned()
        .collect();
    let no_outgoing: Vec<String> = declared
        .iter()
        .filter(|s| !sources.contains(s.as_str()))
        .cloned()
        .collect();
    let isolated: Vec<String> = declared
        .iter()
        .filter(|s| !sources.contains(s.as_str()) && !targets.contains(s.as_str()))
        .cloned()
        .collect();
    let mut undeclared: Vec<String> = sources
        .union(&targets)
        .filter(|s| !declared_set.contains(*s))
        .map(|s| s.to_string())
        .collect();
    undeclared.sort();

    StateCoverage {
        entity: entity.name.clone(),
        state_field: state_field.to_owned(),
        states: declared.to_vec(),
        transitions,
        no_incoming,
        no_outgoing,
        isolated,
        undeclared_states: undeclared,
    }
}

/// Build a transition for one contract: source from a `requires` equality guard
/// on `<alias>.<state>`, target from an `ensures` equality on `result.<state>`.
/// Returns `None` if the contract touches neither end.
fn transition_for(
    c: &ContractIR,
    entity: &crate::glossary::models::EntityDef,
    state_field: &str,
) -> Option<Transition> {
    let from = eq_value(&c.requires, |prefix| entity.has_alias(prefix), state_field, c);
    let to = eq_value(&c.ensures, |prefix| prefix == "result", state_field, c);
    if from.is_none() && to.is_none() {
        return None;
    }
    Some(Transition {
        from,
        to,
        contract: format!("{}/{}/{}", c.entity, c.operation, c.case),
    })
}

/// Find the value `v` of the first `== v` condition whose left side is
/// `<prefix>.<field>` where `prefix_ok(prefix)` and `field == state_field`.
fn eq_value(
    conditions: &[crate::contract_ir::Condition],
    prefix_ok: impl Fn(&str) -> bool,
    state_field: &str,
    _c: &ContractIR,
) -> Option<String> {
    for cond in conditions {
        if cond.operator != "==" {
            continue;
        }
        let ExpressionValue::Str(path) = &cond.left.value else {
            continue;
        };
        let Some((prefix, field)) = path.split_once('.') else {
            continue;
        };
        if field == state_field && prefix_ok(prefix) {
            if let ExpressionValue::Str(v) = &cond.right.value {
                return Some(v.clone());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract_ir::{Condition, Expression, ExpressionValue};
    use crate::glossary::models::{EntityDef, FieldDef};

    fn cond(left: &str, op: &str, right: &str) -> Condition {
        Condition {
            left: Expression { kind: "field".to_owned(), value: ExpressionValue::Str(left.to_owned()) },
            operator: op.to_owned(),
            right: Expression { kind: "enum".to_owned(), value: ExpressionValue::Str(right.to_owned()) },
        }
    }

    fn order_glossary() -> Glossary {
        let mut e = EntityDef::new("Order");
        e.aka = vec!["order".to_owned()];
        e.fields = vec![FieldDef {
            name: "status".to_owned(),
            type_name: "enum".to_owned(),
            enum_values: vec![
                "Unpaid".to_owned(),
                "Paid".to_owned(),
                "Shipped".to_owned(),
                "Cancelled".to_owned(),
            ],
            state: true,
        }];
        Glossary::new(vec![e])
    }

    fn rule(case: &str, op: &str, requires: Vec<Condition>, ensures: Vec<Condition>) -> ContractIR {
        let mut c = ContractIR::new(case, "Order", op);
        c.requires = requires;
        c.ensures = ensures;
        c
    }

    #[test]
    fn no_state_fields_yields_no_reports() {
        let cs = ContractSet::new(vec![]);
        let mut e = EntityDef::new("Widget");
        e.fields = vec![FieldDef::new("size", "int")];
        assert!(analyze(&cs, &Glossary::new(vec![e])).is_empty());
    }

    #[test]
    fn builds_transition_from_guard_and_target() {
        let cs = ContractSet::new(vec![rule(
            "PayOrder",
            "pay",
            vec![cond("order.status", "==", "Unpaid")],
            vec![cond("result.status", "==", "Paid")],
        )]);
        let r = &analyze(&cs, &order_glossary())[0];
        assert_eq!(r.transitions.len(), 1);
        assert_eq!(r.transitions[0].from.as_deref(), Some("Unpaid"));
        assert_eq!(r.transitions[0].to.as_deref(), Some("Paid"));
    }

    #[test]
    fn reports_no_incoming_and_no_outgoing() {
        // Unpaid -> Paid -> Shipped ; Cancelled untouched.
        let cs = ContractSet::new(vec![
            rule("Pay", "pay", vec![cond("order.status", "==", "Unpaid")], vec![cond("result.status", "==", "Paid")]),
            rule("Ship", "ship", vec![cond("order.status", "==", "Paid")], vec![cond("result.status", "==", "Shipped")]),
        ]);
        let r = &analyze(&cs, &order_glossary())[0];
        // Unpaid is a source but never a target → no_incoming.
        assert!(r.no_incoming.contains(&"Unpaid".to_string()));
        // Shipped is a target but never a source → no_outgoing.
        assert!(r.no_outgoing.contains(&"Shipped".to_string()));
        // Cancelled is touched by nothing → isolated (and both lists).
        assert!(r.isolated.contains(&"Cancelled".to_string()));
        assert!(r.no_incoming.contains(&"Cancelled".to_string()));
    }

    #[test]
    fn covered_state_absent_from_gap_lists() {
        let cs = ContractSet::new(vec![
            rule("Pay", "pay", vec![cond("order.status", "==", "Unpaid")], vec![cond("result.status", "==", "Paid")]),
            rule("Ship", "ship", vec![cond("order.status", "==", "Paid")], vec![cond("result.status", "==", "Shipped")]),
        ]);
        let r = &analyze(&cs, &order_glossary())[0];
        // Paid has both an incoming (from Pay) and outgoing (to Ship).
        assert!(!r.no_incoming.contains(&"Paid".to_string()));
        assert!(!r.no_outgoing.contains(&"Paid".to_string()));
        assert!(!r.isolated.contains(&"Paid".to_string()));
    }

    #[test]
    fn undeclared_target_state_flagged() {
        let cs = ContractSet::new(vec![rule(
            "Weird",
            "op",
            vec![cond("order.status", "==", "Paid")],
            vec![cond("result.status", "==", "Frozen")], // not in domain
        )]);
        let r = &analyze(&cs, &order_glossary())[0];
        assert!(r.undeclared_states.contains(&"Frozen".to_string()));
    }
}

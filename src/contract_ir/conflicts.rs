use std::collections::HashMap;

use crate::contract_ir::{Condition, ContractIR, ContractSet, ExpressionValue};

#[derive(Debug, Clone, PartialEq)]
pub struct ConflictCandidate {
    /// `entity/operation/case` of the first contract
    pub key_a: String,
    /// `entity/operation/case` of the second contract
    pub key_b: String,
    /// Human-readable explanation of the heuristic contradiction
    pub reason: String,
}

/// Find pairs of contracts on the same `(entity, operation)` whose `ensures`
/// conditions are syntactically contradictory *and* whose preconditions can
/// hold simultaneously.
///
/// These are *candidates* — Lean proof verification is required to confirm a
/// real logical conflict. A pair with contradictory `ensures` is **not** a
/// conflict when its `requires` sets are mutually exclusive: the two rules are
/// then guarded transitions from disjoint states (e.g. `ship` of a `Paid` vs an
/// `Unpaid` order), so they can never both fire on the same input.
pub fn find_conflict_candidates(contract_set: &ContractSet) -> Vec<ConflictCandidate> {
    let mut by_op: HashMap<String, Vec<&ContractIR>> = HashMap::new();
    for c in &contract_set.contracts {
        by_op.entry(op_key(c)).or_default().push(c);
    }

    let mut candidates = Vec::new();
    for group in by_op.values() {
        for i in 0..group.len() {
            for j in (i + 1)..group.len() {
                let a = group[i];
                let b = group[j];
                // Only a conflict if the outcomes contradict AND the guards can
                // co-occur. Disjoint preconditions ⇒ different lifecycle edges.
                if let Some(reason) = ensures_contradiction(a, b) {
                    if preconditions_mutually_exclusive(a, b).is_none() {
                        candidates.push(ConflictCandidate {
                            key_a: contract_key(a),
                            key_b: contract_key(b),
                            reason,
                        });
                    }
                }
            }
        }
    }
    // Stable order: sort by key_a then key_b
    candidates.sort_by(|x, y| x.key_a.cmp(&y.key_a).then(x.key_b.cmp(&y.key_b)));
    candidates
}

/// Returns `Some(field)` when the two contracts' `requires` sets cannot hold at
/// the same time — i.e. there is a shared left-hand side whose constraints in
/// `a` and `b` are syntactically contradictory (the same complement analysis
/// used for `ensures`). Such a pair governs disjoint input states.
fn preconditions_mutually_exclusive(a: &ContractIR, b: &ContractIR) -> Option<String> {
    for ca in &a.requires {
        for cb in &b.requires {
            if expr_display(&ca.left) != expr_display(&cb.left) {
                continue;
            }
            if condition_contradiction(ca, cb).is_some() {
                return Some(expr_display(&ca.left));
            }
        }
    }
    None
}

fn op_key(c: &ContractIR) -> String {
    format!("{}::{}", c.entity, c.operation)
}

fn contract_key(c: &ContractIR) -> String {
    format!("{}/{}/{}", c.entity, c.operation, c.case)
}

fn expr_display(e: &crate::contract_ir::Expression) -> String {
    match &e.value {
        ExpressionValue::Str(s) => s.clone(),
        ExpressionValue::Int(n) => n.to_string(),
        ExpressionValue::Bool(b) => b.to_string(),
    }
}

fn ensures_contradiction(a: &ContractIR, b: &ContractIR) -> Option<String> {
    for ca in &a.ensures {
        for cb in &b.ensures {
            let left_a = expr_display(&ca.left);
            let left_b = expr_display(&cb.left);
            if left_a != left_b {
                continue;
            }
            if let Some(reason) = condition_contradiction(ca, cb) {
                return Some(reason);
            }
        }
    }
    None
}

/// Returns Some(reason) when two conditions on the same left-hand side are
/// syntactically contradictory under simple value arithmetic:
/// - `== X` vs `== Y` (X ≠ Y) — cannot simultaneously be two distinct values
/// - `== X` vs `!= X` — direct negation
/// - `> X` vs `<= X` — complement pair (same bound)
/// - `< X` vs `>= X` — complement pair (same bound)
fn condition_contradiction(a: &Condition, b: &Condition) -> Option<String> {
    let left = expr_display(&a.left);
    let ra = expr_display(&a.right);
    let rb = expr_display(&b.right);
    let oa = a.operator.as_str();
    let ob = b.operator.as_str();

    match (oa, ob) {
        ("==", "==") if ra != rb => Some(format!(
            "`{left}` cannot be both `{ra}` and `{rb}`"
        )),
        ("==", "!=") | ("!=", "==") if ra == rb => Some(format!(
            "`{left} == {ra}` directly contradicts `{left} != {rb}`"
        )),
        (">", "<=") | ("<=", ">") if ra == rb => Some(format!(
            "`{left} > {ra}` and `{left} <= {rb}` are complement bounds"
        )),
        ("<", ">=") | (">=", "<") if ra == rb => Some(format!(
            "`{left} < {ra}` and `{left} >= {rb}` are complement bounds"
        )),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract_ir::{Condition, ContractIR, ContractSet, Expression, ExpressionValue};

    fn field(v: &str) -> Expression {
        Expression { kind: "field".to_owned(), value: ExpressionValue::Str(v.to_owned()) }
    }
    fn enom(v: &str) -> Expression {
        Expression { kind: "enum".to_owned(), value: ExpressionValue::Str(v.to_owned()) }
    }
    fn cond(left: &str, op: &str, right: &str) -> Condition {
        Condition { left: field(left), operator: op.to_owned(), right: enom(right) }
    }

    fn contract(case: &str, entity: &str, op: &str, ensures: Vec<Condition>) -> ContractIR {
        ContractIR {
            case: case.to_owned(),
            entity: entity.to_owned(),
            operation: op.to_owned(),
            requires: vec![],
            ensures,
            forbidden: vec![],
            preserves: vec![],
            assumes: vec![],
            intent: None, examples: Vec::new(), source: None,
        }
    }

    fn contract_full(
        case: &str,
        entity: &str,
        op: &str,
        requires: Vec<Condition>,
        ensures: Vec<Condition>,
    ) -> ContractIR {
        let mut c = contract(case, entity, op, ensures);
        c.requires = requires;
        c
    }

    #[test]
    fn no_conflict_when_preconditions_mutually_exclusive() {
        // The Paid/Unpaid case: two transitions from disjoint source states.
        // `ship Paid → Shipped` and `ship Unpaid → Rejected` are different edges
        // of the Order lifecycle, not a contradiction.
        let cs = ContractSet::new(vec![
            contract_full(
                "ShipPaidOrder",
                "Order",
                "ship",
                vec![cond("order.status", "==", "Paid")],
                vec![cond("result.status", "==", "Shipped")],
            ),
            contract_full(
                "CannotShipUnpaidOrder",
                "Order",
                "ship",
                vec![cond("order.status", "==", "Unpaid")],
                vec![cond("result.status", "==", "Rejected")],
            ),
        ]);
        assert!(
            find_conflict_candidates(&cs).is_empty(),
            "disjoint preconditions must suppress the ensures contradiction"
        );
    }

    #[test]
    fn conflict_when_preconditions_overlap() {
        // Same source state, contradictory targets → genuine nondeterminism.
        let cs = ContractSet::new(vec![
            contract_full(
                "Approve",
                "Mortgage",
                "review",
                vec![cond("loan.status", "==", "UnderReview")],
                vec![cond("result.status", "==", "Approved")],
            ),
            contract_full(
                "Reject",
                "Mortgage",
                "review",
                vec![cond("loan.status", "==", "UnderReview")],
                vec![cond("result.status", "==", "Rejected")],
            ),
        ]);
        assert_eq!(
            find_conflict_candidates(&cs).len(),
            1,
            "overlapping preconditions with contradictory ensures is a real conflict"
        );
    }

    #[test]
    fn no_conflict_when_preconditions_mutually_exclusive_on_int_bound() {
        // Guard on a numeric determinant: credit_score >= 750 vs < 750.
        let cs = ContractSet::new(vec![
            contract_full(
                "Prime",
                "Mortgage",
                "price",
                vec![cond("loan.credit_score", ">=", "750")],
                vec![cond("result.tier", "==", "Prime")],
            ),
            contract_full(
                "Subprime",
                "Mortgage",
                "price",
                vec![cond("loan.credit_score", "<", "750")],
                vec![cond("result.tier", "==", "Subprime")],
            ),
        ]);
        assert!(
            find_conflict_candidates(&cs).is_empty(),
            "disjoint numeric guards must suppress the conflict"
        );
    }

    #[test]
    fn conflict_stands_when_only_one_rule_has_preconditions() {
        // One guarded, one unguarded → the unguarded rule can fire in the same
        // state as the guarded one, so the contradiction remains possible.
        let cs = ContractSet::new(vec![
            contract_full(
                "Guarded",
                "Order",
                "ship",
                vec![cond("order.status", "==", "Paid")],
                vec![cond("result.status", "==", "Shipped")],
            ),
            contract_full(
                "Unguarded",
                "Order",
                "ship",
                vec![],
                vec![cond("result.status", "==", "Rejected")],
            ),
        ]);
        assert_eq!(find_conflict_candidates(&cs).len(), 1);
    }

    #[test]
    fn no_conflicts_in_single_contract_set() {
        let cs = ContractSet::new(vec![contract(
            "CancelPaidOrder",
            "Order",
            "cancelOrder",
            vec![cond("result.status", "==", "Cancelled")],
        )]);
        assert!(find_conflict_candidates(&cs).is_empty());
    }

    #[test]
    fn no_conflicts_when_contracts_on_different_operations() {
        let cs = ContractSet::new(vec![
            contract("A", "Order", "cancelOrder", vec![cond("result.status", "==", "Cancelled")]),
            contract("B", "Order", "shipOrder", vec![cond("result.status", "==", "Active")]),
        ]);
        assert!(find_conflict_candidates(&cs).is_empty());
    }

    #[test]
    fn no_conflicts_when_ensures_same_value() {
        let cs = ContractSet::new(vec![
            contract("A", "Order", "cancelOrder", vec![cond("result.status", "==", "Cancelled")]),
            contract("B", "Order", "cancelOrder", vec![cond("result.status", "==", "Cancelled")]),
        ]);
        assert!(find_conflict_candidates(&cs).is_empty());
    }

    #[test]
    fn conflict_detected_ensures_eq_different_values() {
        let cs = ContractSet::new(vec![
            contract("A", "Order", "cancelOrder", vec![cond("result.status", "==", "Cancelled")]),
            contract("B", "Order", "cancelOrder", vec![cond("result.status", "==", "Active")]),
        ]);
        let candidates = find_conflict_candidates(&cs);
        assert_eq!(candidates.len(), 1);
        assert!(candidates[0].reason.contains("cannot be both"));
    }

    #[test]
    fn conflict_detected_eq_and_neq_same_value() {
        let cs = ContractSet::new(vec![
            contract("A", "Order", "cancelOrder", vec![cond("result.status", "==", "Cancelled")]),
            contract("B", "Order", "cancelOrder", vec![cond("result.status", "!=", "Cancelled")]),
        ]);
        let candidates = find_conflict_candidates(&cs);
        assert_eq!(candidates.len(), 1);
        assert!(candidates[0].reason.contains("contradicts"));
    }

    #[test]
    fn no_conflict_eq_and_neq_different_values() {
        // status == Paid vs status != Cancelled — not a contradiction
        let cs = ContractSet::new(vec![
            contract("A", "Order", "cancelOrder", vec![cond("result.status", "==", "Paid")]),
            contract("B", "Order", "cancelOrder", vec![cond("result.status", "!=", "Cancelled")]),
        ]);
        assert!(find_conflict_candidates(&cs).is_empty());
    }

    #[test]
    fn conflict_detected_complement_bounds_gt_lte() {
        let cs = ContractSet::new(vec![
            contract("A", "Account", "withdraw", vec![cond("balance", ">", "0")]),
            contract("B", "Account", "withdraw", vec![cond("balance", "<=", "0")]),
        ]);
        let candidates = find_conflict_candidates(&cs);
        assert_eq!(candidates.len(), 1);
        assert!(candidates[0].reason.contains("complement bounds"));
    }

    #[test]
    fn conflict_detected_complement_bounds_lt_gte() {
        let cs = ContractSet::new(vec![
            contract("A", "Account", "transfer", vec![cond("amount", "<", "1000")]),
            contract("B", "Account", "transfer", vec![cond("amount", ">=", "1000")]),
        ]);
        let candidates = find_conflict_candidates(&cs);
        assert_eq!(candidates.len(), 1);
    }

    #[test]
    fn no_conflict_gt_and_lt_different_bounds() {
        // balance > 0 vs balance < 100 — not complementary
        let cs = ContractSet::new(vec![
            contract("A", "Account", "withdraw", vec![cond("balance", ">", "0")]),
            contract("B", "Account", "withdraw", vec![cond("balance", "<", "100")]),
        ]);
        assert!(find_conflict_candidates(&cs).is_empty());
    }

    #[test]
    fn no_conflicts_when_different_left_hand_sides() {
        let cs = ContractSet::new(vec![
            contract("A", "Order", "cancelOrder", vec![cond("result.status", "==", "Cancelled")]),
            contract("B", "Order", "cancelOrder", vec![cond("result.amount", "==", "0")]),
        ]);
        assert!(find_conflict_candidates(&cs).is_empty());
    }

    #[test]
    fn conflict_candidate_keys_use_slash_format() {
        let cs = ContractSet::new(vec![
            contract("A", "Order", "cancelOrder", vec![cond("result.status", "==", "Cancelled")]),
            contract("B", "Order", "cancelOrder", vec![cond("result.status", "==", "Active")]),
        ]);
        let candidates = find_conflict_candidates(&cs);
        let c = &candidates[0];
        // Both keys should use entity/operation/case format
        assert!(c.key_a.contains('/'));
        assert!(c.key_b.contains('/'));
        assert!(c.key_a.contains("Order"));
        assert!(c.key_b.contains("Order"));
    }
}

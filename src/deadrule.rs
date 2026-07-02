//! Dead-rule detection: a rule whose `requires` can never all hold at once can
//! never fire. Conflict detection is cross-rule and per-condition satisfiability
//! checks each condition alone, so neither catches a *within-rule*
//! contradiction like `credit_score >= 750 AND credit_score < 600`.
//!
//! This is decidable per field: enum (must-equal / must-differ), bool, and int
//! (interval intersection). It returns the two conditions that make the rule
//! dead, so the Lean generator can emit a machine-checked unsatisfiability
//! proof (`∀ x, ¬(a ∧ b)`, discharged by decide/omega).

use serde::{Deserialize, Serialize};

use crate::contract_ir::{Condition, ContractIR, ExpressionValue};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeadRule {
    pub key: String,
    pub field: String,
    pub reason: String,
}

/// The two conflicting conditions plus the field they share — used by the Lean
/// generator; not serialized in the API surface.
#[derive(Debug, Clone, PartialEq)]
pub struct DeadRuleWitness {
    pub contract_index: usize,
    pub field: String,
    pub a: Condition,
    pub b: Condition,
    pub reason: String,
}

fn field_key(path: &str) -> &str {
    path.split_once('.').map(|(_, rest)| rest).unwrap_or(path)
}

fn left_field(cond: &Condition) -> Option<&str> {
    match &cond.left.value {
        ExpressionValue::Str(p) => Some(field_key(p)),
        _ => None,
    }
}

/// Find a within-field contradiction among a rule's requires. Returns the two
/// conditions (and a reason) that cannot both hold.
fn dead_field<'a>(conds: &[&'a Condition]) -> Option<(&'a Condition, &'a Condition, String)> {
    // ── enum: an `== V` fixes the value; conflicts with another `== W` or `!= V`.
    let mut eq: Option<(&Condition, &str)> = None;
    for c in conds {
        if let ExpressionValue::Str(v) = &c.right.value {
            if v.chars().next().is_some_and(|ch| ch.is_ascii_uppercase()) {
                match c.operator.as_str() {
                    "==" => {
                        if let Some((prev, pv)) = eq {
                            if pv != v {
                                return Some((prev, c, format!("must equal both {pv} and {v}")));
                            }
                        }
                        eq = Some((c, v));
                    }
                    "!=" => {
                        if let Some((prev, pv)) = eq {
                            if pv == v {
                                return Some((prev, c, format!("must equal and not equal {v}")));
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    // Two `== V` with different V, or `== V` and `!= V`, may also appear with the
    // `!=` seen before the `==`; a second pass covers that ordering.
    for c in conds {
        if c.operator == "!=" {
            if let (ExpressionValue::Str(v), Some((prev, pv))) = (&c.right.value, eq) {
                if pv == v {
                    return Some((prev, c, format!("must equal and not equal {v}")));
                }
            }
        }
    }

    // ── bool.
    let mut btrue: Option<&Condition> = None;
    let mut bfalse: Option<&Condition> = None;
    for c in conds {
        if let ExpressionValue::Bool(b) = &c.right.value {
            let want_true = (c.operator == "==") == *b; // ==true or !=false ⇒ true
            if want_true {
                btrue = Some(c);
            } else {
                bfalse = Some(c);
            }
        }
    }
    if let (Some(a), Some(b)) = (btrue, bfalse) {
        return Some((a, b, "must be both true and false".to_owned()));
    }

    // ── int: intersect the feasible interval; empty ⇒ dead.
    let mut lo: i128 = i128::MIN;
    let mut hi: i128 = i128::MAX;
    let mut lo_c: Option<&Condition> = None;
    let mut hi_c: Option<&Condition> = None;
    let mut bump_lo = |n: i128, c: &'a Condition, lo: &mut i128, lo_c: &mut Option<&'a Condition>| {
        if n > *lo {
            *lo = n;
            *lo_c = Some(c);
        }
    };
    let mut bump_hi = |n: i128, c: &'a Condition, hi: &mut i128, hi_c: &mut Option<&'a Condition>| {
        if n < *hi {
            *hi = n;
            *hi_c = Some(c);
        }
    };
    for c in conds {
        if let ExpressionValue::Int(n) = &c.right.value {
            let n = *n as i128;
            match c.operator.as_str() {
                ">=" => bump_lo(n, c, &mut lo, &mut lo_c),
                ">" => bump_lo(n + 1, c, &mut lo, &mut lo_c),
                "<=" => bump_hi(n, c, &mut hi, &mut hi_c),
                "<" => bump_hi(n - 1, c, &mut hi, &mut hi_c),
                "==" => {
                    bump_lo(n, c, &mut lo, &mut lo_c);
                    bump_hi(n, c, &mut hi, &mut hi_c);
                }
                _ => {}
            }
        }
    }
    if lo > hi {
        if let (Some(a), Some(b)) = (lo_c, hi_c) {
            return Some((a, b, format!("empty range: lower bound {lo} exceeds upper bound {hi}")));
        }
    }
    None
}

/// Witnesses for every dead rule in the set (with the conflicting conditions).
pub fn find_dead_rule_witnesses(contracts: &[ContractIR]) -> Vec<DeadRuleWitness> {
    let mut out = Vec::new();
    for (idx, c) in contracts.iter().enumerate() {
        // Group requires by field.
        let mut by_field: std::collections::BTreeMap<String, Vec<&Condition>> = Default::default();
        for cond in &c.requires {
            if let Some(f) = left_field(cond) {
                by_field.entry(f.to_owned()).or_default().push(cond);
            }
        }
        for (field, conds) in &by_field {
            if let Some((a, b, reason)) = dead_field(conds) {
                out.push(DeadRuleWitness {
                    contract_index: idx,
                    field: field.clone(),
                    a: a.clone(),
                    b: b.clone(),
                    reason,
                });
                break; // one finding per rule is enough
            }
        }
    }
    out
}

/// Serializable dead-rule findings for the API surface.
pub fn find_dead_rules(contracts: &[ContractIR]) -> Vec<DeadRule> {
    find_dead_rule_witnesses(contracts)
        .into_iter()
        .map(|w| {
            let c = &contracts[w.contract_index];
            DeadRule {
                key: format!("{}/{}/{}", c.entity, c.operation, c.case),
                field: w.field,
                reason: w.reason,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract_ir::Expression;

    fn cond(left: &str, op: &str, right: ExpressionValue) -> Condition {
        Condition {
            left: Expression { kind: "field".into(), value: ExpressionValue::Str(left.into()) },
            operator: op.into(),
            right: Expression { kind: "v".into(), value: right },
        }
    }

    fn rule(reqs: Vec<Condition>) -> ContractIR {
        let mut c = ContractIR::new("R", "E", "op");
        c.requires = reqs;
        c
    }

    #[test]
    fn int_empty_range_is_dead() {
        let c = rule(vec![
            cond("x.score", ">=", ExpressionValue::Int(750)),
            cond("x.score", "<", ExpressionValue::Int(600)),
        ]);
        let d = find_dead_rules(std::slice::from_ref(&c));
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].field, "score");
    }

    #[test]
    fn satisfiable_int_range_is_not_dead() {
        let c = rule(vec![
            cond("x.score", ">=", ExpressionValue::Int(600)),
            cond("x.score", "<", ExpressionValue::Int(750)),
        ]);
        assert!(find_dead_rules(std::slice::from_ref(&c)).is_empty());
    }

    #[test]
    fn enum_two_values_is_dead() {
        let c = rule(vec![
            cond("o.status", "==", ExpressionValue::Str("Paid".into())),
            cond("o.status", "==", ExpressionValue::Str("Unpaid".into())),
        ]);
        assert_eq!(find_dead_rules(std::slice::from_ref(&c)).len(), 1);
    }

    #[test]
    fn enum_eq_and_neq_same_is_dead() {
        let c = rule(vec![
            cond("o.status", "==", ExpressionValue::Str("Paid".into())),
            cond("o.status", "!=", ExpressionValue::Str("Paid".into())),
        ]);
        assert_eq!(find_dead_rules(std::slice::from_ref(&c)).len(), 1);
    }

    #[test]
    fn bool_true_and_false_is_dead() {
        let c = rule(vec![
            cond("o.flag", "==", ExpressionValue::Bool(true)),
            cond("o.flag", "==", ExpressionValue::Bool(false)),
        ]);
        assert_eq!(find_dead_rules(std::slice::from_ref(&c)).len(), 1);
    }

    #[test]
    fn different_fields_not_dead() {
        let c = rule(vec![
            cond("o.a", ">=", ExpressionValue::Int(750)),
            cond("o.b", "<", ExpressionValue::Int(600)),
        ]);
        assert!(find_dead_rules(std::slice::from_ref(&c)).is_empty());
    }
}

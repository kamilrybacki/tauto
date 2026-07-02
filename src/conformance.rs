//! Conformance: evaluate a rule against its own `examples` (the author's intent
//! made checkable). This is correctness *relative to the stated cases* — it
//! catches a formalization that disagrees with what the author said should
//! happen, without needing an operation model. It reuses the rule's conditions
//! as decidable predicates over field values.

use serde::{Deserialize, Serialize};

use crate::contract_ir::{Condition, ContractIR, ExpressionValue, RuleExample};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConformanceStatus {
    /// The rule agrees with the example.
    Pass,
    /// The rule disagrees with the example — a real correctness/formalization gap.
    Fail,
    /// The example doesn't give enough state to decide (missing field, or a
    /// type-incompatible comparison).
    Underspecified,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExampleOutcome {
    pub case: String,
    pub index: usize,
    pub status: ConformanceStatus,
    pub message: String,
}

/// Field path with its leading instance/result prefix stripped, so `order.status`
/// and `result.status` both key to `status` (matches the example's bare keys).
fn field_key(path: &str) -> &str {
    path.split_once('.').map(|(_, rest)| rest).unwrap_or(path)
}

/// Evaluate `actual <op> expected`. `None` when the two values aren't comparable
/// under this operator (e.g. `>` on enums, or a type mismatch).
fn eval_op(actual: &ExpressionValue, op: &str, expected: &ExpressionValue) -> Option<bool> {
    use ExpressionValue::*;
    match (actual, expected) {
        (Int(a), Int(b)) => Some(match op {
            "==" => a == b,
            "!=" => a != b,
            ">=" => a >= b,
            "<=" => a <= b,
            ">" => a > b,
            "<" => a < b,
            _ => return None,
        }),
        (Bool(a), Bool(b)) => match op {
            "==" => Some(a == b),
            "!=" => Some(a != b),
            _ => None,
        },
        (Str(a), Str(b)) => match op {
            "==" => Some(a == b),
            "!=" => Some(a != b),
            _ => None,
        },
        _ => None,
    }
}

/// The left field of a condition, resolved against a state map.
fn condition_left_field(cond: &Condition) -> Option<&str> {
    match &cond.left.value {
        ExpressionValue::Str(path) => Some(field_key(path)),
        _ => None,
    }
}

enum Fire {
    Yes,
    No,
    Undecidable(String),
}

/// Does the rule fire on `given`? Requires all `requires` to hold. Undecidable
/// when a needed field is absent or a comparison is type-incompatible.
fn rule_fires(
    contract: &ContractIR,
    given: &std::collections::BTreeMap<String, ExpressionValue>,
) -> Fire {
    let mut all_hold = true;
    for cond in &contract.requires {
        let Some(field) = condition_left_field(cond) else { continue };
        let Some(actual) = given.get(field) else {
            return Fire::Undecidable(format!("`given` is missing field `{field}` (a precondition)"));
        };
        match eval_op(actual, &cond.operator, &cond.right.value) {
            Some(true) => {}
            Some(false) => all_hold = false,
            None => {
                return Fire::Undecidable(format!(
                    "cannot compare `{field}` ({actual}) with `{}`",
                    cond.right.value
                ))
            }
        }
    }
    if all_hold {
        Fire::Yes
    } else {
        Fire::No
    }
}

/// Check the rule's postconditions against an expected result state.
fn check_ensures(
    contract: &ContractIR,
    then: &std::collections::BTreeMap<String, ExpressionValue>,
) -> Result<Option<String>, String> {
    // Ok(None) = all hold; Ok(Some(msg)) = underspecified; Err(msg) = violated.
    for cond in &contract.ensures {
        let Some(field) = condition_left_field(cond) else { continue };
        let Some(actual) = then.get(field) else {
            return Ok(Some(format!("`then` is missing result field `{field}` (a postcondition)")));
        };
        match eval_op(actual, &cond.operator, &cond.right.value) {
            Some(true) => {}
            Some(false) => {
                return Err(format!(
                    "result `{field} = {actual}` violates ensures `{} {} {}`",
                    field, cond.operator, cond.right.value
                ))
            }
            None => {
                return Ok(Some(format!(
                    "cannot compare result `{field}` ({actual}) with `{}`",
                    cond.right.value
                )))
            }
        }
    }
    Ok(None)
}

fn evaluate_example(contract: &ContractIR, index: usize, ex: &RuleExample) -> ExampleOutcome {
    let mk = |status, message: String| ExampleOutcome {
        case: contract.case.clone(),
        index,
        status,
        message,
    };
    let fires = match rule_fires(contract, &ex.given) {
        Fire::Yes => true,
        Fire::No => false,
        Fire::Undecidable(why) => return mk(ConformanceStatus::Underspecified, why),
    };

    if !ex.applies {
        return if fires {
            mk(
                ConformanceStatus::Fail,
                "rule fires on this state, but the example marks `applies: false`".to_owned(),
            )
        } else {
            mk(ConformanceStatus::Pass, "rule correctly does not apply".to_owned())
        };
    }

    if !fires {
        return mk(
            ConformanceStatus::Fail,
            "rule does not fire on this state, but the example expects it to apply".to_owned(),
        );
    }
    match check_ensures(contract, &ex.then) {
        Ok(None) => mk(ConformanceStatus::Pass, "rule fires and the outcome matches".to_owned()),
        Ok(Some(why)) => mk(ConformanceStatus::Underspecified, why),
        Err(why) => mk(ConformanceStatus::Fail, why),
    }
}

/// Evaluate every example of every contract in the set.
pub fn check_examples(contracts: &[ContractIR]) -> Vec<ExampleOutcome> {
    contracts
        .iter()
        .flat_map(|c| c.examples.iter().enumerate().map(move |(i, ex)| evaluate_example(c, i, ex)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract_ir::{Condition, Expression};

    fn cond(left: &str, op: &str, right: ExpressionValue) -> Condition {
        Condition {
            left: Expression { kind: "field".into(), value: ExpressionValue::Str(left.into()) },
            operator: op.into(),
            right: Expression { kind: "v".into(), value: right },
        }
    }

    fn enum_val(s: &str) -> ExpressionValue {
        ExpressionValue::Str(s.into())
    }

    fn ship_rule() -> ContractIR {
        let mut c = ContractIR::new("ShipPaidOrder", "Order", "ship");
        c.requires = vec![cond("order.status", "==", enum_val("Paid"))];
        c.ensures = vec![cond("result.status", "==", enum_val("Shipped"))];
        c
    }

    fn ex(
        given: &[(&str, ExpressionValue)],
        then: &[(&str, ExpressionValue)],
        applies: bool,
    ) -> RuleExample {
        RuleExample {
            given: given.iter().map(|(k, v)| (k.to_string(), v.clone())).collect(),
            then: then.iter().map(|(k, v)| (k.to_string(), v.clone())).collect(),
            applies,
        }
    }

    #[test]
    fn matching_example_passes() {
        let mut c = ship_rule();
        c.examples = vec![ex(&[("status", enum_val("Paid"))], &[("status", enum_val("Shipped"))], true)];
        let out = check_examples(std::slice::from_ref(&c));
        assert_eq!(out[0].status, ConformanceStatus::Pass, "{}", out[0].message);
    }

    #[test]
    fn wrong_outcome_fails() {
        let mut c = ship_rule();
        // rule ensures Shipped, but the example expects Cancelled → mis-formalization.
        c.examples = vec![ex(&[("status", enum_val("Paid"))], &[("status", enum_val("Cancelled"))], true)];
        let out = check_examples(std::slice::from_ref(&c));
        assert_eq!(out[0].status, ConformanceStatus::Fail);
        assert!(out[0].message.contains("violates ensures"));
    }

    #[test]
    fn applies_false_passes_when_guard_not_met() {
        let mut c = ship_rule();
        c.examples = vec![ex(&[("status", enum_val("Unpaid"))], &[], false)];
        let out = check_examples(std::slice::from_ref(&c));
        assert_eq!(out[0].status, ConformanceStatus::Pass);
    }

    #[test]
    fn applies_false_fails_when_rule_actually_fires() {
        let mut c = ship_rule();
        // Example says the rule shouldn't apply to a Paid order, but it does.
        c.examples = vec![ex(&[("status", enum_val("Paid"))], &[], false)];
        let out = check_examples(std::slice::from_ref(&c));
        assert_eq!(out[0].status, ConformanceStatus::Fail);
    }

    #[test]
    fn missing_field_is_underspecified() {
        let mut c = ship_rule();
        c.examples = vec![ex(&[("other", enum_val("X"))], &[("status", enum_val("Shipped"))], true)];
        let out = check_examples(std::slice::from_ref(&c));
        assert_eq!(out[0].status, ConformanceStatus::Underspecified);
    }
}

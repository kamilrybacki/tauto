use serde::Serialize;

use crate::contract_ir::{Condition, ContractIR, ContractSet, ExpressionValue};

#[derive(Debug, Clone, Serialize)]
pub struct FieldAssignment {
    pub field: String,
    pub value: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TestCase {
    pub id: String,
    pub kind: String,
    pub description: String,
    pub operation: String,
    pub given: Vec<FieldAssignment>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub expect_ensures: Vec<FieldAssignment>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub expect_forbidden_not_called: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub expect_preserved: Vec<String>,
    pub should_pass: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub violated_precondition: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContractTestSuite {
    pub contract: String,
    pub entity: String,
    pub operation: String,
    pub case_name: String,
    pub cases: Vec<TestCase>,
}

pub fn generate_suite(contract_set: &ContractSet) -> Vec<ContractTestSuite> {
    contract_set.contracts.iter().map(suite_for_contract).collect()
}

fn suite_for_contract(contract: &ContractIR) -> ContractTestSuite {
    let key = format!("{}/{}/{}", contract.entity, contract.operation, contract.case);
    let mut cases = Vec::new();

    let happy_given: Vec<FieldAssignment> = contract.requires.iter().map(assign_passing).collect();
    let expect_ensures: Vec<FieldAssignment> = contract.ensures.iter().map(assign_ensures).collect();
    let expect_forbidden: Vec<String> = contract.forbidden.iter().map(|f| f.operation.clone()).collect();

    cases.push(TestCase {
        id: format!("{}_happy_path", to_snake(&contract.case)),
        kind: "happy_path".to_owned(),
        description: "All preconditions satisfied — postconditions and preservation must hold".to_owned(),
        operation: contract.operation.clone(),
        given: happy_given.clone(),
        expect_ensures,
        expect_forbidden_not_called: expect_forbidden,
        expect_preserved: contract.preserves.clone(),
        should_pass: true,
        violated_precondition: None,
    });

    for (i, cond) in contract.requires.iter().enumerate() {
        let cond_str = cond_display(cond);
        let given: Vec<FieldAssignment> = contract.requires.iter()
            .enumerate()
            .map(|(j, c)| if j == i { assign_failing(c) } else { assign_passing(c) })
            .collect();

        cases.push(TestCase {
            id: format!("{}_violate_{}", to_snake(&contract.case), to_snake(&left_field(cond))),
            kind: "precondition_violation".to_owned(),
            description: format!("Violates `{}` — operation must be rejected", cond_str),
            operation: contract.operation.clone(),
            given,
            expect_ensures: vec![],
            expect_forbidden_not_called: vec![],
            expect_preserved: vec![],
            should_pass: false,
            violated_precondition: Some(cond_str),
        });
    }

    ContractTestSuite {
        contract: key,
        entity: contract.entity.clone(),
        operation: contract.operation.clone(),
        case_name: contract.case.clone(),
        cases,
    }
}

fn assign_passing(cond: &Condition) -> FieldAssignment {
    let (val, note) = passing_value(cond);
    FieldAssignment { field: left_field(cond), value: val, note }
}

fn assign_failing(cond: &Condition) -> FieldAssignment {
    let (val, note) = failing_value(cond);
    FieldAssignment { field: left_field(cond), value: val, note }
}

fn assign_ensures(cond: &Condition) -> FieldAssignment {
    FieldAssignment {
        field: left_field(cond),
        value: expr_to_json(&cond.right.value),
        note: None,
    }
}

fn left_field(cond: &Condition) -> String {
    match &cond.left.value {
        ExpressionValue::Str(s) => s.clone(),
        ExpressionValue::Int(n) => n.to_string(),
        ExpressionValue::Bool(b) => b.to_string(),
    }
}

fn cond_display(cond: &Condition) -> String {
    format!("{} {} {}", left_field(cond), cond.operator, expr_display(&cond.right.value))
}

fn expr_display(v: &ExpressionValue) -> String {
    match v {
        ExpressionValue::Bool(b) => b.to_string(),
        ExpressionValue::Int(n) => n.to_string(),
        ExpressionValue::Str(s) => s.clone(),
    }
}

fn expr_to_json(v: &ExpressionValue) -> serde_json::Value {
    match v {
        ExpressionValue::Bool(b) => serde_json::Value::Bool(*b),
        ExpressionValue::Int(n) => serde_json::Value::Number((*n).into()),
        ExpressionValue::Str(s) => serde_json::Value::String(s.clone()),
    }
}

fn passing_value(cond: &Condition) -> (serde_json::Value, Option<String>) {
    match (&cond.operator as &str, &cond.right.value) {
        (">=", ExpressionValue::Int(n)) => ((*n).into(), Some(format!("boundary: exactly {n}"))),
        ("<=", ExpressionValue::Int(n)) => ((*n).into(), Some(format!("boundary: exactly {n}"))),
        (">",  ExpressionValue::Int(n)) => ((*n + 1).into(), Some(format!("{} satisfies >{n}", *n + 1))),
        ("<",  ExpressionValue::Int(n)) => ((*n - 1).into(), Some(format!("{} satisfies <{n}", *n - 1))),
        ("==", ExpressionValue::Int(n)) => ((*n).into(), None),
        ("!=", ExpressionValue::Int(n)) => ((*n + 1).into(), Some(format!("{} ≠ {n}", *n + 1))),
        ("==", ExpressionValue::Bool(b)) => ((*b).into(), None),
        ("!=", ExpressionValue::Bool(b)) => ((!b).into(), None),
        ("==", ExpressionValue::Str(s)) => (s.clone().into(), None),
        ("!=", ExpressionValue::Str(s)) => (
            format!("<any value ≠ {s}>").into(),
            Some(format!("any value other than \"{s}\"")),
        ),
        _ => (
            format!("<satisfies {} {}>", cond.operator, expr_display(&cond.right.value)).into(),
            None,
        ),
    }
}

fn failing_value(cond: &Condition) -> (serde_json::Value, Option<String>) {
    match (&cond.operator as &str, &cond.right.value) {
        (">=", ExpressionValue::Int(n)) => ((*n - 1).into(), Some(format!("{} < {n} — violates ≥{n}", *n - 1))),
        ("<=", ExpressionValue::Int(n)) => ((*n + 1).into(), Some(format!("{} > {n} — violates ≤{n}", *n + 1))),
        (">",  ExpressionValue::Int(n)) => ((*n).into(),     Some(format!("{n} == {n} — equal boundary fails >{n}"))),
        ("<",  ExpressionValue::Int(n)) => ((*n).into(),     Some(format!("{n} == {n} — equal boundary fails <{n}"))),
        ("==", ExpressionValue::Int(n)) => ((*n + 1).into(), Some(format!("{} ≠ required {n}", *n + 1))),
        ("!=", ExpressionValue::Int(n)) => ((*n).into(),     Some(format!("{n} violates ≠{n}"))),
        ("==", ExpressionValue::Bool(b)) => ((!b).into(),    Some(format!("opposite of required {b}"))),
        ("!=", ExpressionValue::Bool(b)) => ((*b).into(),    Some(format!("{b} violates ≠{b}"))),
        ("==", ExpressionValue::Str(s)) => (
            format!("<any value ≠ {s}>").into(),
            Some(format!("any value other than \"{s}\"")),
        ),
        ("!=", ExpressionValue::Str(s)) => (
            s.clone().into(),
            Some(format!("\"{s}\" violates ≠{s}")),
        ),
        _ => (
            format!("<violates {} {}>", cond.operator, expr_display(&cond.right.value)).into(),
            None,
        ),
    }
}

fn to_snake(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            out.push('_');
            out.extend(c.to_lowercase());
        } else {
            out.extend(c.to_lowercase());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract_ir::{ContractIR, ContractSet, Expression, ExpressionValue};

    fn int_cond(field: &str, op: &str, n: i64) -> Condition {
        Condition {
            left: Expression { kind: "field".to_owned(), value: ExpressionValue::Str(field.to_owned()) },
            operator: op.to_owned(),
            right: Expression { kind: "int".to_owned(), value: ExpressionValue::Int(n) },
        }
    }

    fn bool_cond(field: &str, op: &str, b: bool) -> Condition {
        Condition {
            left: Expression { kind: "field".to_owned(), value: ExpressionValue::Str(field.to_owned()) },
            operator: op.to_owned(),
            right: Expression { kind: "bool".to_owned(), value: ExpressionValue::Bool(b) },
        }
    }

    fn str_cond(field: &str, op: &str, s: &str) -> Condition {
        Condition {
            left: Expression { kind: "field".to_owned(), value: ExpressionValue::Str(field.to_owned()) },
            operator: op.to_owned(),
            right: Expression { kind: "enum".to_owned(), value: ExpressionValue::Str(s.to_owned()) },
        }
    }

    #[test]
    fn happy_path_generated_for_each_contract() {
        let mut c = ContractIR::new("ApprovePrime", "Mortgage", "approve");
        c.requires = vec![int_cond("credit_score", ">=", 750)];
        let cs = ContractSet::new(vec![c]);
        let suites = generate_suite(&cs);
        assert_eq!(suites.len(), 1);
        assert!(suites[0].cases.iter().any(|tc| tc.kind == "happy_path"));
    }

    #[test]
    fn violation_case_per_requires_condition() {
        let mut c = ContractIR::new("ApprovePrime", "Mortgage", "approve");
        c.requires = vec![
            int_cond("credit_score", ">=", 750),
            bool_cond("verified", "==", true),
        ];
        let cs = ContractSet::new(vec![c]);
        let suites = generate_suite(&cs);
        let violations: Vec<_> = suites[0].cases.iter().filter(|tc| tc.kind == "precondition_violation").collect();
        assert_eq!(violations.len(), 2);
    }

    #[test]
    fn gte_boundary_passing_is_exact_n() {
        let cond = int_cond("score", ">=", 750);
        let a = assign_passing(&cond);
        assert_eq!(a.value, serde_json::json!(750));
    }

    #[test]
    fn gte_boundary_failing_is_n_minus_one() {
        let cond = int_cond("score", ">=", 750);
        let a = assign_failing(&cond);
        assert_eq!(a.value, serde_json::json!(749));
    }

    #[test]
    fn bool_eq_passing_and_failing_are_opposites() {
        let cond = bool_cond("verified", "==", true);
        assert_eq!(assign_passing(&cond).value, serde_json::json!(true));
        assert_eq!(assign_failing(&cond).value, serde_json::json!(false));
    }

    #[test]
    fn str_eq_failing_is_symbolic() {
        let cond = str_cond("status", "==", "UnderReview");
        let v = assign_failing(&cond).value;
        let s = v.as_str().unwrap();
        assert!(s.starts_with('<'), "expected symbolic placeholder, got: {s}");
        assert!(s.contains("UnderReview"));
    }

    #[test]
    fn to_snake_converts_camel_case() {
        assert_eq!(to_snake("ApprovePrime"), "approve_prime");
        assert_eq!(to_snake("myField"), "my_field");
        assert_eq!(to_snake("simple"), "simple");
    }
}

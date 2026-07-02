use serde::{Deserialize, Serialize};

/// Typed value inside an Expression. Untagged so it round-trips as a JSON primitive.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ExpressionValue {
    Bool(bool),
    Int(i64),
    Str(String),
}

impl std::fmt::Display for ExpressionValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExpressionValue::Bool(b) => write!(f, "{b}"),
            ExpressionValue::Int(n) => write!(f, "{n}"),
            ExpressionValue::Str(s) => write!(f, "{s}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Expression {
    pub kind: String,
    pub value: ExpressionValue,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Condition {
    pub left: Expression,
    pub operator: String,
    pub right: Expression,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ForbiddenOperation {
    pub operation: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<Expression>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceLocation {
    pub document_path: String,
    pub start_line: u32,
    pub end_line: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub category: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

impl Diagnostic {
    pub fn parse_error(message: impl Into<String>, document_path: Option<String>, line: Option<u32>) -> Self {
        Self {
            category: "parse_error".to_owned(),
            message: message.into(),
            document_path,
            line,
            suggestion: None,
        }
    }
}

/// A concrete case the author expects the rule to handle a certain way — the
/// user's *intent* made checkable. `given` is the input entity state, `then` the
/// expected result state (when the rule applies). `applies=false` means the rule
/// should NOT fire on `given`. Conformance evaluates the rule against these.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuleExample {
    #[serde(default)]
    pub given: std::collections::BTreeMap<String, ExpressionValue>,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub then: std::collections::BTreeMap<String, ExpressionValue>,
    /// Whether the rule is expected to fire on `given` (default true).
    #[serde(default = "default_true")]
    pub applies: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContractIR {
    pub case: String,
    pub entity: String,
    pub operation: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires: Vec<Condition>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ensures: Vec<Condition>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub forbidden: Vec<ForbiddenOperation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub preserves: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assumes: Vec<String>,
    /// Free-text statement of what the rule is meant to do (the human intent).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent: Option<String>,
    /// Checkable examples of the intent.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub examples: Vec<RuleExample>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceLocation>,
}

impl ContractIR {
    pub fn new(case: impl Into<String>, entity: impl Into<String>, operation: impl Into<String>) -> Self {
        Self {
            case: case.into(),
            entity: entity.into(),
            operation: operation.into(),
            requires: Vec::new(),
            ensures: Vec::new(),
            forbidden: Vec::new(),
            preserves: Vec::new(),
            assumes: Vec::new(),
            intent: None,
            examples: Vec::new(),
            source: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContractSet {
    pub schema_version: u32,
    pub contracts: Vec<ContractIR>,
}

impl ContractSet {
    pub fn new(contracts: Vec<ContractIR>) -> Self {
        Self { schema_version: 1, contracts }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn str_expr(kind: &str, value: &str) -> Expression {
        Expression { kind: kind.to_owned(), value: ExpressionValue::Str(value.to_owned()) }
    }

    fn condition(left: &str, op: &str, right: &str) -> Condition {
        Condition {
            left: str_expr("field", left),
            operator: op.to_owned(),
            right: str_expr("enum", right),
        }
    }

    #[test]
    fn contract_ir_new_sets_required_fields() {
        let c = ContractIR::new("CancelPaidOrder", "Order", "cancelOrder");
        assert_eq!(c.case, "CancelPaidOrder");
        assert_eq!(c.entity, "Order");
        assert_eq!(c.operation, "cancelOrder");
        assert!(c.requires.is_empty());
        assert!(c.ensures.is_empty());
        assert!(c.forbidden.is_empty());
        assert!(c.preserves.is_empty());
        assert!(c.assumes.is_empty());
        assert!(c.source.is_none());
    }

    #[test]
    fn contract_set_new_sets_schema_version() {
        let cs = ContractSet::new(vec![ContractIR::new("A", "B", "C")]);
        assert_eq!(cs.schema_version, 1);
        assert_eq!(cs.contracts.len(), 1);
    }

    #[test]
    fn source_location_uses_document_path_and_line_range() {
        let loc = SourceLocation {
            document_path: "spec.md".to_owned(),
            start_line: 10,
            end_line: 15,
        };
        assert_eq!(loc.document_path, "spec.md");
        assert_eq!(loc.start_line, 10);
        assert_eq!(loc.end_line, 15);
    }

    #[test]
    fn contract_ir_with_conditions_round_trips_json() {
        let c = ContractIR {
            case: "CancelPaidOrder".to_owned(),
            entity: "Order".to_owned(),
            operation: "cancelOrder".to_owned(),
            requires: vec![condition("order.status", "==", "Paid")],
            ensures: vec![condition("result.status", "==", "Cancelled")],
            forbidden: vec![],
            preserves: vec![],
            assumes: vec![],
            intent: None,
            examples: vec![],
            source: None,
        };
        let json = serde_json::to_string(&c).unwrap();
        let back: ContractIR = serde_json::from_str(&json).unwrap();
        assert_eq!(c, back);
    }

    #[test]
    fn empty_vecs_are_omitted_from_serialization() {
        let c = ContractIR::new("Simple", "E", "op");
        let json = serde_json::to_string(&c).unwrap();
        assert!(!json.contains("requires"));
        assert!(!json.contains("ensures"));
        assert!(!json.contains("forbidden"));
        assert!(!json.contains("preserves"));
        assert!(!json.contains("assumes"));
    }

    #[test]
    fn source_omitted_from_serialization_when_none() {
        let c = ContractIR::new("Simple", "E", "op");
        let json = serde_json::to_string(&c).unwrap();
        assert!(!json.contains("source"));
    }

    #[test]
    fn forbidden_operation_with_args_round_trips() {
        let f = ForbiddenOperation {
            operation: "deleteOrder".to_owned(),
            args: vec![str_expr("field", "order.id")],
        };
        let json = serde_json::to_string(&f).unwrap();
        let back: ForbiddenOperation = serde_json::from_str(&json).unwrap();
        assert_eq!(f, back);
    }

    #[test]
    fn expression_value_bool_round_trips() {
        let e = Expression { kind: "bool".to_owned(), value: ExpressionValue::Bool(true) };
        let json = serde_json::to_string(&e).unwrap();
        let back: Expression = serde_json::from_str(&json).unwrap();
        assert_eq!(e, back);
    }

    #[test]
    fn expression_value_int_round_trips() {
        let e = Expression { kind: "int".to_owned(), value: ExpressionValue::Int(42) };
        let json = serde_json::to_string(&e).unwrap();
        let back: Expression = serde_json::from_str(&json).unwrap();
        assert_eq!(e, back);
    }

    #[test]
    fn diagnostic_parse_error_constructor() {
        let d = Diagnostic::parse_error("Missing case", Some("spec.md".to_owned()), Some(3));
        assert_eq!(d.category, "parse_error");
        assert_eq!(d.document_path.as_deref(), Some("spec.md"));
        assert_eq!(d.line, Some(3));
        assert!(d.suggestion.is_none());
    }
}

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceLocation {
    pub file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Expression {
    pub kind: String,
    pub value: serde_json::Value,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub message: String,
    pub category: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
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
    pub forbids: Vec<ForbiddenOperation>,
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
            forbids: Vec::new(),
            source: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContractSet {
    pub schema_version: String,
    pub contracts: Vec<ContractIR>,
}

impl ContractSet {
    pub fn new(contracts: Vec<ContractIR>) -> Self {
        Self {
            schema_version: "1".to_owned(),
            contracts,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expr(kind: &str, value: &str) -> Expression {
        Expression {
            kind: kind.to_owned(),
            value: serde_json::Value::String(value.to_owned()),
        }
    }

    fn condition(left: &str, op: &str, right: &str) -> Condition {
        Condition {
            left: expr("field", left),
            operator: op.to_owned(),
            right: expr("enum", right),
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
        assert!(c.forbids.is_empty());
        assert!(c.source.is_none());
    }

    #[test]
    fn contract_set_new_sets_schema_version() {
        let cs = ContractSet::new(vec![ContractIR::new("A", "B", "C")]);
        assert_eq!(cs.schema_version, "1");
        assert_eq!(cs.contracts.len(), 1);
    }

    #[test]
    fn source_location_is_optional_in_contract() {
        let mut c = ContractIR::new("Test", "Entity", "op");
        assert!(c.source.is_none());
        c.source = Some(SourceLocation { file: "spec.md".to_owned(), line: Some(10), column: None });
        assert!(c.source.is_some());
    }

    #[test]
    fn contract_ir_with_conditions_round_trips_json() {
        let c = ContractIR {
            case: "CancelPaidOrder".to_owned(),
            entity: "Order".to_owned(),
            operation: "cancelOrder".to_owned(),
            requires: vec![condition("order.status", "==", "Paid")],
            ensures: vec![condition("result.status", "==", "Cancelled")],
            forbids: vec![],
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
        assert!(!json.contains("forbids"));
    }

    #[test]
    fn source_omitted_from_serialization_when_none() {
        let c = ContractIR::new("Simple", "E", "op");
        let json = serde_json::to_string(&c).unwrap();
        assert!(!json.contains("source"));
    }

    #[test]
    fn forbidden_operation_with_reason_round_trips() {
        let f = ForbiddenOperation {
            operation: "deleteOrder".to_owned(),
            reason: Some("immutable after payment".to_owned()),
        };
        let json = serde_json::to_string(&f).unwrap();
        let back: ForbiddenOperation = serde_json::from_str(&json).unwrap();
        assert_eq!(f, back);
    }

    #[test]
    fn diagnostic_fields_are_accessible() {
        let d = Diagnostic {
            message: "unverified theorem".to_owned(),
            category: "lean_sorry".to_owned(),
            document_path: Some("contracts/Foo.lean".to_owned()),
            line: Some(7),
        };
        assert_eq!(d.category, "lean_sorry");
        assert_eq!(d.line, Some(7));
    }
}

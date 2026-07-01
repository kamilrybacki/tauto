use serde::{Deserialize, Serialize};

use crate::contract_ir::{Condition, ContractIR, ContractSet, ExpressionValue};
use crate::glossary::models::Glossary;

/// An advisory glossary finding. Never an error — it flags a rule that
/// references vocabulary the glossary does not know, so an author can reconcile
/// the term (or extend the glossary).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GlossaryWarning {
    /// The contract key (entity/operation/case) the warning is about.
    pub contract: String,
    /// A machine category: `unknown_entity`, `unknown_operation`,
    /// `unknown_field`, `cross_entity_reference`, `unknown_prefix`,
    /// `unknown_enum_value`.
    pub category: String,
    pub message: String,
}

/// The universal field-path prefix for an operation's result / post-state.
const RESULT_PREFIX: &str = "result";

/// Validate a contract set against the glossary, returning advisory warnings.
/// An empty glossary yields no warnings (no vocabulary declared = nothing to
/// check against), so the feature is inert until a glossary exists.
pub fn validate(contract_set: &ContractSet, glossary: &Glossary) -> Vec<GlossaryWarning> {
    if glossary.is_empty() {
        return Vec::new();
    }
    let mut warnings = Vec::new();
    for c in &contract_set.contracts {
        validate_contract(c, glossary, &mut warnings);
    }
    warnings
}

fn key(c: &ContractIR) -> String {
    format!("{}/{}/{}", c.entity, c.operation, c.case)
}

fn validate_contract(c: &ContractIR, glossary: &Glossary, out: &mut Vec<GlossaryWarning>) {
    let k = key(c);
    let entity_def = glossary.entity(&c.entity);

    match entity_def {
        None => out.push(GlossaryWarning {
            contract: k.clone(),
            category: "unknown_entity".to_owned(),
            message: format!("entity '{}' is not defined in the glossary", c.entity),
        }),
        Some(e) => {
            if !e.operations.is_empty() && !e.operations.contains(&c.operation) {
                out.push(GlossaryWarning {
                    contract: k.clone(),
                    category: "unknown_operation".to_owned(),
                    message: format!(
                        "operation '{}' is not declared for entity '{}'",
                        c.operation, c.entity
                    ),
                });
            }
        }
    }

    for cond in c.requires.iter().chain(c.ensures.iter()) {
        validate_condition(c, cond, glossary, entity_def.is_some(), &k, out);
    }
}

fn validate_condition(
    c: &ContractIR,
    cond: &Condition,
    glossary: &Glossary,
    entity_known: bool,
    k: &str,
    out: &mut Vec<GlossaryWarning>,
) {
    // The left side of a well-formed condition is a field path string.
    let ExpressionValue::Str(path) = &cond.left.value else {
        return;
    };
    let Some((prefix, field)) = path.split_once('.') else {
        return; // not a dotted field path; nothing to resolve
    };

    let entity_def = glossary.entity(&c.entity);

    // `result.*` is the operation's post-state; its fields share the entity's
    // vocabulary.
    if prefix == RESULT_PREFIX {
        if let Some(e) = entity_def {
            check_field_and_enum(e, field, cond, k, out);
        }
        return;
    }

    // Prefix is an alias of the contract's own entity → check the field.
    if let Some(e) = entity_def {
        if e.has_alias(prefix) {
            check_field_and_enum(e, field, cond, k, out);
            return;
        }
    }

    // Prefix is an alias of a *different* entity → the Order-vs-Package
    // distinction: the rule reaches into another entity's vocabulary.
    if let Some(other) = glossary.entity_by_alias(prefix) {
        if other.name != c.entity {
            out.push(GlossaryWarning {
                contract: k.to_owned(),
                category: "cross_entity_reference".to_owned(),
                message: format!(
                    "field path '{path}' refers to entity '{}' (alias '{prefix}'), but this contract is about '{}'",
                    other.name, c.entity
                ),
            });
            return;
        }
    }

    // Prefix matches nothing. Only warn when the contract's entity is known;
    // otherwise the unknown_entity warning already covers the confusion.
    if entity_known {
        out.push(GlossaryWarning {
            contract: k.to_owned(),
            category: "unknown_prefix".to_owned(),
            message: format!(
                "field path '{path}' uses prefix '{prefix}', which is not '{RESULT_PREFIX}' nor a declared alias of any entity"
            ),
        });
    }
}

fn check_field_and_enum(
    entity: &crate::glossary::models::EntityDef,
    field: &str,
    cond: &Condition,
    k: &str,
    out: &mut Vec<GlossaryWarning>,
) {
    let Some(field_def) = entity.field(field) else {
        out.push(GlossaryWarning {
            contract: k.to_owned(),
            category: "unknown_field".to_owned(),
            message: format!("field '{field}' is not declared on entity '{}'", entity.name),
        });
        return;
    };

    // If the field is an enum and the comparison value is an enum literal, it
    // must be a declared member.
    if field_def.type_name == "enum" {
        if let ExpressionValue::Str(val) = &cond.right.value {
            if !field_def.enum_values.iter().any(|v| v == val) {
                out.push(GlossaryWarning {
                    contract: k.to_owned(),
                    category: "unknown_enum_value".to_owned(),
                    message: format!(
                        "'{val}' is not a declared value of enum field '{}.{field}' (allowed: {})",
                        entity.name,
                        field_def.enum_values.join(", ")
                    ),
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract_ir::{Expression, ExpressionValue};
    use crate::glossary::models::{EntityDef, FieldDef};

    fn cond(left: &str, op: &str, right: ExpressionValue) -> Condition {
        Condition {
            left: Expression { kind: "field".to_owned(), value: ExpressionValue::Str(left.to_owned()) },
            operator: op.to_owned(),
            right: Expression { kind: "lit".to_owned(), value: right },
        }
    }

    fn mortgage() -> EntityDef {
        EntityDef {
            name: "Mortgage".to_owned(),
            aka: vec!["loan".to_owned()],
            describes: None,
            fields: vec![
                FieldDef { name: "credit_score".to_owned(), type_name: "int".to_owned(), enum_values: vec![] },
                FieldDef { name: "status".to_owned(), type_name: "enum".to_owned(), enum_values: vec!["UnderReview".to_owned(), "Approved".to_owned()] },
            ],
            operations: vec!["approveApplication".to_owned()],
        }
    }

    fn package() -> EntityDef {
        let mut e = EntityDef::new("Package");
        e.aka = vec!["package".to_owned()];
        e.fields = vec![FieldDef { name: "weight".to_owned(), type_name: "int".to_owned(), enum_values: vec![] }];
        e
    }

    fn contract_with(requires: Vec<Condition>, ensures: Vec<Condition>) -> ContractIR {
        let mut c = ContractIR::new("ApprovePrime", "Mortgage", "approveApplication");
        c.requires = requires;
        c.ensures = ensures;
        c
    }

    fn run(c: ContractIR, g: Glossary) -> Vec<GlossaryWarning> {
        validate(&ContractSet::new(vec![c]), &g)
    }

    #[test]
    fn empty_glossary_yields_no_warnings() {
        let c = contract_with(vec![cond("loan.credit_score", ">=", ExpressionValue::Int(750))], vec![]);
        assert!(run(c, Glossary::default()).is_empty());
    }

    #[test]
    fn clean_contract_no_warnings() {
        let c = contract_with(
            vec![cond("loan.credit_score", ">=", ExpressionValue::Int(750))],
            vec![cond("result.status", "==", ExpressionValue::Str("Approved".to_owned()))],
        );
        let w = run(c, Glossary::new(vec![mortgage()]));
        assert!(w.is_empty(), "unexpected: {w:?}");
    }

    #[test]
    fn unknown_entity_flagged() {
        let mut c = ContractIR::new("X", "Spaceship", "launch");
        c.requires = vec![];
        let w = run(c, Glossary::new(vec![mortgage()]));
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].category, "unknown_entity");
    }

    #[test]
    fn unknown_operation_flagged() {
        let mut c = contract_with(vec![], vec![]);
        c.operation = "teleport".to_owned();
        let w = run(c, Glossary::new(vec![mortgage()]));
        assert!(w.iter().any(|x| x.category == "unknown_operation"));
    }

    #[test]
    fn unknown_field_flagged() {
        let c = contract_with(vec![cond("loan.shoe_size", "==", ExpressionValue::Int(9))], vec![]);
        let w = run(c, Glossary::new(vec![mortgage()]));
        assert!(w.iter().any(|x| x.category == "unknown_field"));
    }

    #[test]
    fn cross_entity_reference_flagged() {
        // A Mortgage rule that reaches into Package's vocabulary.
        let c = contract_with(vec![cond("package.weight", ">", ExpressionValue::Int(10))], vec![]);
        let w = run(c, Glossary::new(vec![mortgage(), package()]));
        let x = w.iter().find(|x| x.category == "cross_entity_reference").expect("expected cross-entity warning");
        assert!(x.message.contains("Package"));
        assert!(x.message.contains("Mortgage"));
    }

    #[test]
    fn unknown_prefix_flagged() {
        let c = contract_with(vec![cond("widget.color", "==", ExpressionValue::Str("Red".to_owned()))], vec![]);
        let w = run(c, Glossary::new(vec![mortgage()]));
        assert!(w.iter().any(|x| x.category == "unknown_prefix"));
    }

    #[test]
    fn unknown_enum_value_flagged() {
        let c = contract_with(
            vec![cond("loan.status", "==", ExpressionValue::Str("Frozen".to_owned()))],
            vec![],
        );
        let w = run(c, Glossary::new(vec![mortgage()]));
        assert!(w.iter().any(|x| x.category == "unknown_enum_value"));
    }

    #[test]
    fn declared_enum_value_ok() {
        let c = contract_with(
            vec![cond("loan.status", "==", ExpressionValue::Str("Approved".to_owned()))],
            vec![],
        );
        let w = run(c, Glossary::new(vec![mortgage()]));
        assert!(w.is_empty(), "unexpected: {w:?}");
    }
}

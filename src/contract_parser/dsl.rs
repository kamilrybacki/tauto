use crate::contract_ir::{
    Condition, ContractIR, Diagnostic, Expression, ExpressionValue, ForbiddenOperation, RuleExample,
};

use super::markdown::ContractBlock;

const ALLOWED_SECTIONS: &[&str] = &[
    "entity", "operation", "requires", "ensures", "forbidden", "preserves", "assumes", "intent",
    "examples",
];

#[derive(Debug, Clone, PartialEq)]
pub struct ParseResult {
    pub contract: Option<ContractIR>,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn parse_contract_block(block: &ContractBlock) -> ParseResult {
    let mut case_name: Option<String> = None;
    let mut sections: std::collections::HashMap<String, Vec<(u32, String)>> =
        std::collections::HashMap::new();
    let mut current_section: Option<String> = None;
    let mut diagnostics: Vec<Diagnostic> = Vec::new();

    let doc = &block.source.document_path;

    for (offset, raw_line) in block.raw_block.lines().enumerate() {
        let line_number = block.source.start_line + offset as u32;
        let stripped = raw_line.trim();
        if stripped.is_empty() {
            continue;
        }
        if let Some(rest) = stripped.strip_prefix("case ") {
            case_name = Some(rest.trim().to_owned());
            continue;
        }
        if let Some(section) = stripped.strip_suffix(':') {
            if ALLOWED_SECTIONS.contains(&section) {
                current_section = Some(section.to_owned());
                sections.entry(section.to_owned()).or_default();
            } else {
                diagnostics.push(Diagnostic::parse_error(
                    format!("Unknown section: {section}"),
                    Some(doc.clone()),
                    Some(line_number),
                ));
                current_section = None;
            }
            continue;
        }
        match &current_section {
            Some(sec) => {
                sections
                    .entry(sec.clone())
                    .or_default()
                    .push((line_number, stripped.to_owned()));
            }
            None => {
                diagnostics.push(Diagnostic::parse_error(
                    format!("Line is outside a section: {stripped}"),
                    Some(doc.clone()),
                    Some(line_number),
                ));
            }
        }
    }

    let entity = single_value("entity", &sections, doc, block.source.start_line, &mut diagnostics);
    let operation =
        single_value("operation", &sections, doc, block.source.start_line, &mut diagnostics);

    if case_name.is_none() {
        diagnostics.push(Diagnostic::parse_error(
            "Missing case declaration",
            Some(doc.clone()),
            Some(block.source.start_line),
        ));
    }

    let requires: Vec<Condition> = sections
        .get("requires")
        .map(|items| items.iter().filter_map(|i| parse_condition(i, doc, &mut diagnostics)).collect())
        .unwrap_or_default();

    let ensures: Vec<Condition> = sections
        .get("ensures")
        .map(|items| items.iter().filter_map(|i| parse_condition(i, doc, &mut diagnostics)).collect())
        .unwrap_or_default();

    let forbidden: Vec<ForbiddenOperation> = sections
        .get("forbidden")
        .map(|items| {
            items.iter().filter_map(|i| parse_forbidden(i, doc, &mut diagnostics)).collect()
        })
        .unwrap_or_default();

    let preserves: Vec<String> = sections
        .get("preserves")
        .map(|items| items.iter().map(|(_, v)| v.clone()).collect())
        .unwrap_or_default();

    let assumes: Vec<String> = sections
        .get("assumes")
        .map(|items| items.iter().map(|(_, v)| v.clone()).collect())
        .unwrap_or_default();

    // Free-text intent: join the section's lines.
    let intent: Option<String> = sections.get("intent").and_then(|items| {
        if items.is_empty() {
            None
        } else {
            Some(items.iter().map(|(_, v)| v.as_str()).collect::<Vec<_>>().join(" "))
        }
    });

    let examples: Vec<RuleExample> = sections
        .get("examples")
        .map(|items| items.iter().filter_map(|i| parse_example(i, doc, &mut diagnostics)).collect())
        .unwrap_or_default();

    if !diagnostics.is_empty() || case_name.is_none() || entity.is_none() || operation.is_none() {
        return ParseResult { contract: None, diagnostics };
    }

    ParseResult {
        contract: Some(ContractIR {
            case: case_name.unwrap(),
            entity: entity.unwrap(),
            operation: operation.unwrap(),
            requires,
            ensures,
            forbidden,
            preserves,
            assumes,
            intent,
            examples,
            source: Some(block.source.clone()),
        }),
        diagnostics: vec![],
    }
}

/// Parse one `examples:` item. Grammar (semicolon-separated clauses):
///   given: field=Value, field=N; then: field=Value
///   given: field=Value; applies: false
fn parse_example(
    item: &(u32, String),
    doc: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<RuleExample> {
    let (line, text) = item;
    // Strip the list marker: `- given: ...` → `given: ...`.
    let text = text.trim_start().trim_start_matches('-').trim();
    let mut given = std::collections::BTreeMap::new();
    let mut then = std::collections::BTreeMap::new();
    let mut applies = true;
    let mut applies_explicit = false;

    for clause in text.split(';') {
        let clause = clause.trim();
        if clause.is_empty() {
            continue;
        }
        let Some((key, rest)) = clause.split_once(':') else {
            diagnostics.push(Diagnostic::parse_error(
                format!("Malformed example clause (expected `key: ...`): {clause}"),
                Some(doc.to_owned()),
                Some(*line),
            ));
            return None;
        };
        match key.trim() {
            "given" => parse_assignments(rest, &mut given),
            "then" => {
                parse_assignments(rest, &mut then);
                applies = true;
            }
            "applies" => {
                applies = rest.trim() == "true";
                applies_explicit = true;
            }
            other => {
                diagnostics.push(Diagnostic::parse_error(
                    format!("Unknown example clause `{other}` (use given/then/applies)"),
                    Some(doc.to_owned()),
                    Some(*line),
                ));
                return None;
            }
        }
    }

    if given.is_empty() {
        diagnostics.push(Diagnostic::parse_error(
            format!("Example has no `given` state: {text}"),
            Some(doc.to_owned()),
            Some(*line),
        ));
        return None;
    }
    // `then` present implies the rule applies unless applies was set false.
    if !then.is_empty() && !applies_explicit {
        applies = true;
    }
    Some(RuleExample { given, then, applies })
}

/// Parse `field=Value, field=N` into a map of typed values.
fn parse_assignments(raw: &str, into: &mut std::collections::BTreeMap<String, ExpressionValue>) {
    for pair in raw.split(',') {
        if let Some((k, v)) = pair.split_once('=') {
            let k = k.trim();
            if !k.is_empty() {
                into.insert(k.to_owned(), parse_expression(v.trim()).value);
            }
        }
    }
}

fn single_value(
    section: &str,
    sections: &std::collections::HashMap<String, Vec<(u32, String)>>,
    doc: &str,
    start_line: u32,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<String> {
    match sections.get(section).map(|v| v.as_slice()) {
        Some([(_, value)]) => Some(value.clone()),
        _ => {
            diagnostics.push(Diagnostic::parse_error(
                format!("Section '{section}' must contain exactly one value"),
                Some(doc.to_owned()),
                Some(start_line),
            ));
            None
        }
    }
}

fn parse_condition(
    item: &(u32, String),
    doc: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Condition> {
    let (line, text) = item;
    // Operators ordered longest-first to avoid `=` matching `==`
    for op in &["==", "!=", ">=", "<=", ">", "<"] {
        if let Some(pos) = text.find(op) {
            let left = text[..pos].trim();
            let right = text[pos + op.len()..].trim();
            if !left.is_empty() && !right.is_empty() {
                return Some(Condition {
                    left: parse_expression(left),
                    operator: op.to_string(),
                    right: parse_expression(right),
                });
            }
        }
    }
    diagnostics.push(Diagnostic::parse_error(
        format!("Malformed condition: {text}"),
        Some(doc.to_owned()),
        Some(*line),
    ));
    None
}

fn parse_forbidden(
    item: &(u32, String),
    doc: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<ForbiddenOperation> {
    let (line, text) = item;
    if let Some(paren_pos) = text.find('(') {
        let operation = text[..paren_pos].trim();
        if !text.ends_with(')') {
            diagnostics.push(Diagnostic::parse_error(
                format!("Malformed forbidden operation call: {text}"),
                Some(doc.to_owned()),
                Some(*line),
            ));
            return None;
        }
        let raw_args = &text[paren_pos + 1..text.len() - 1];
        let args: Vec<Expression> = if raw_args.trim().is_empty() {
            vec![]
        } else {
            raw_args.split(',').map(|a| parse_expression(a.trim())).collect()
        };
        return Some(ForbiddenOperation { operation: operation.to_owned(), args });
    }
    diagnostics.push(Diagnostic::parse_error(
        format!("Malformed forbidden operation call: {text}"),
        Some(doc.to_owned()),
        Some(*line),
    ));
    None
}

fn parse_expression(raw: &str) -> Expression {
    if raw == "true" {
        return Expression { kind: "bool".to_owned(), value: ExpressionValue::Bool(true) };
    }
    if raw == "false" {
        return Expression { kind: "bool".to_owned(), value: ExpressionValue::Bool(false) };
    }
    if let Ok(n) = raw.parse::<i64>() {
        return Expression { kind: "int".to_owned(), value: ExpressionValue::Int(n) };
    }
    if raw.contains('.') {
        return Expression {
            kind: "field".to_owned(),
            value: ExpressionValue::Str(raw.to_owned()),
        };
    }
    if raw.chars().next().map(|c| c.is_lowercase()).unwrap_or(false) {
        return Expression {
            kind: "variable".to_owned(),
            value: ExpressionValue::Str(raw.to_owned()),
        };
    }
    Expression { kind: "enum".to_owned(), value: ExpressionValue::Str(raw.to_owned()) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract_ir::SourceLocation;
    use crate::contract_parser::markdown::{ContractBlock, extract_contract_blocks};

    fn block(raw: &str) -> ContractBlock {
        ContractBlock {
            raw_block: raw.to_owned(),
            source: SourceLocation {
                document_path: "test.md".to_owned(),
                start_line: 1,
                end_line: raw.lines().count() as u32,
            },
        }
    }

    const FULL_BLOCK: &str = "\
case CancelPaidOrder
entity:
  Order
operation:
  cancelOrder
requires:
  order.status == Paid
ensures:
  result.status == Cancelled
";

    #[test]
    fn parse_full_block_succeeds() {
        let result = parse_contract_block(&block(FULL_BLOCK));
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        let contract = result.contract.unwrap();
        assert_eq!(contract.case, "CancelPaidOrder");
        assert_eq!(contract.entity, "Order");
        assert_eq!(contract.operation, "cancelOrder");
    }

    #[test]
    fn parse_requires_condition_correctly() {
        let result = parse_contract_block(&block(FULL_BLOCK));
        let contract = result.contract.unwrap();
        assert_eq!(contract.requires.len(), 1);
        assert_eq!(contract.requires[0].operator, "==");
    }

    #[test]
    fn parse_ensures_condition_correctly() {
        let result = parse_contract_block(&block(FULL_BLOCK));
        let contract = result.contract.unwrap();
        assert_eq!(contract.ensures.len(), 1);
        assert_eq!(contract.ensures[0].operator, "==");
    }

    #[test]
    fn missing_case_produces_diagnostic() {
        let no_case = "entity:\n  Order\noperation:\n  op\n";
        let result = parse_contract_block(&block(no_case));
        assert!(result.contract.is_none());
        assert!(result.diagnostics.iter().any(|d| d.message.contains("Missing case")));
    }

    #[test]
    fn missing_entity_produces_diagnostic() {
        let no_entity = "case Foo\noperation:\n  op\n";
        let result = parse_contract_block(&block(no_entity));
        assert!(result.contract.is_none());
        assert!(result.diagnostics.iter().any(|d| d.message.contains("entity")));
    }

    #[test]
    fn unknown_section_produces_diagnostic() {
        let bad_section =
            "case Foo\nentity:\n  E\noperation:\n  op\nweirdstuff:\n  something\n";
        let result = parse_contract_block(&block(bad_section));
        assert!(result.contract.is_none());
        assert!(result.diagnostics.iter().any(|d| d.message.contains("Unknown section")));
    }

    #[test]
    fn malformed_condition_produces_diagnostic() {
        let bad_cond = "case Foo\nentity:\n  E\noperation:\n  op\nrequires:\n  this is not a condition\n";
        let result = parse_contract_block(&block(bad_cond));
        assert!(result.contract.is_none());
        assert!(result.diagnostics.iter().any(|d| d.message.contains("Malformed condition")));
    }

    #[test]
    fn expression_field_has_dot() {
        let expr = parse_expression("order.status");
        assert_eq!(expr.kind, "field");
    }

    #[test]
    fn expression_bool_true() {
        let expr = parse_expression("true");
        assert_eq!(expr.kind, "bool");
        assert_eq!(expr.value, ExpressionValue::Bool(true));
    }

    #[test]
    fn expression_int() {
        let expr = parse_expression("42");
        assert_eq!(expr.kind, "int");
        assert_eq!(expr.value, ExpressionValue::Int(42));
    }

    #[test]
    fn expression_enum_starts_uppercase() {
        let expr = parse_expression("Paid");
        assert_eq!(expr.kind, "enum");
    }

    #[test]
    fn round_trip_via_markdown_extraction() {
        let md = "# Title\n\n```contract\ncase RoundTrip\nentity:\n  E\noperation:\n  op\n```\n";
        let blocks = extract_contract_blocks(md, "rt.md");
        assert_eq!(blocks.len(), 1);
        let result = parse_contract_block(&blocks[0]);
        assert!(result.contract.is_some());
        assert_eq!(result.contract.unwrap().case, "RoundTrip");
    }

    #[test]
    fn parse_intent_and_examples() {
        let raw = "\
case ShipPaidOrder
entity:
  Order
operation:
  ship
requires:
  order.status == Paid
ensures:
  result.status == Shipped
intent:
  A paid order should ship.
examples:
  - given: status=Paid; then: status=Shipped
  - given: status=Unpaid; applies: false
";
        let c = parse_contract_block(&block(raw)).contract.unwrap();
        assert_eq!(c.intent.as_deref(), Some("A paid order should ship."));
        assert_eq!(c.examples.len(), 2);
        assert_eq!(c.examples[0].given.get("status"), Some(&ExpressionValue::Str("Paid".into())));
        assert_eq!(c.examples[0].then.get("status"), Some(&ExpressionValue::Str("Shipped".into())));
        assert!(c.examples[0].applies);
        assert!(!c.examples[1].applies);
    }

    #[test]
    fn forbidden_operation_parsed_correctly() {
        let with_forbidden =
            "case Foo\nentity:\n  E\noperation:\n  op\nforbidden:\n  deleteOrder(order.id)\n";
        let result = parse_contract_block(&block(with_forbidden));
        let contract = result.contract.unwrap();
        assert_eq!(contract.forbidden.len(), 1);
        assert_eq!(contract.forbidden[0].operation, "deleteOrder");
        assert_eq!(contract.forbidden[0].args.len(), 1);
    }
}

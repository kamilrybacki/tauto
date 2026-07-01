use crate::glossary::models::{EntityDef, FieldDef};

/// Extract the inner text of every ```glossary fenced block in a markdown doc.
pub fn extract_glossary_blocks(markdown: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut in_block = false;
    let mut current: Vec<&str> = Vec::new();
    for line in markdown.lines() {
        if !in_block && line.trim() == "```glossary" {
            in_block = true;
            current.clear();
            continue;
        }
        if in_block && line.trim() == "```" {
            blocks.push(current.join("\n"));
            in_block = false;
            continue;
        }
        if in_block {
            current.push(line);
        }
    }
    // An unclosed fence produces no block (mirrors contract extraction).
    blocks
}

/// Parse one glossary block into an `EntityDef`. Returns `None` if the block has
/// no `entity <Name>` header. The block form is:
///
/// ```text
/// entity Mortgage
/// aka: loan
/// describes: A home loan moving through underwriting to closing.
/// fields:
///   credit_score: int
///   status: enum(UnderReview, Approved, Rejected)
/// operations:
///   approveApplication
/// ```
pub fn parse_glossary_block(block: &str) -> Option<EntityDef> {
    let mut lines = block.lines().peekable();

    // Header: first non-empty line must be `entity <Name>`.
    let name = loop {
        let line = lines.next()?;
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        let rest = t.strip_prefix("entity")?;
        let name = rest.trim();
        if name.is_empty() {
            return None;
        }
        break name.to_owned();
    };

    let mut entity = EntityDef::new(name);

    while let Some(line) = lines.next() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        // Section labels end in ':' with no value (list follows) OR carry an
        // inline value after the colon.
        let (label, inline) = match t.split_once(':') {
            Some((l, v)) => (l.trim(), v.trim()),
            None => continue, // not a recognized line; skip
        };

        match label {
            "aka" => {
                entity.aka.extend(split_list(inline));
                consume_indented(&mut lines, |item| entity.aka.extend(split_list(item)));
            }
            "describes" => {
                if !inline.is_empty() {
                    entity.describes = Some(inline.to_owned());
                }
            }
            "operations" => {
                if !inline.is_empty() {
                    entity.operations.extend(split_list(inline));
                }
                consume_indented(&mut lines, |item| entity.operations.extend(split_list(item)));
            }
            "fields" => {
                if !inline.is_empty() {
                    if let Some(f) = parse_field(inline) {
                        entity.fields.push(f);
                    }
                }
                consume_indented(&mut lines, |item| {
                    if let Some(f) = parse_field(item) {
                        entity.fields.push(f);
                    }
                });
            }
            _ => {}
        }
    }

    Some(entity)
}

/// Consume subsequent indented lines (list items under a section), passing each
/// trimmed item to `push`. Stops at the first non-indented / blank boundary.
fn consume_indented<'a, I, F>(lines: &mut std::iter::Peekable<I>, mut push: F)
where
    I: Iterator<Item = &'a str>,
    F: FnMut(&str),
{
    while let Some(peek) = lines.peek() {
        let is_indented = peek.starts_with(' ') || peek.starts_with('\t');
        if peek.trim().is_empty() || !is_indented {
            break;
        }
        let item = lines.next().unwrap().trim();
        if !item.is_empty() {
            push(item);
        }
    }
}

/// Split a comma-separated inline list, trimming and dropping empties.
fn split_list(s: &str) -> Vec<String> {
    s.split(',')
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Parse a `name: type` field item. `type` is optional; `enum(A, B, C)` yields
/// the enum members.
fn parse_field(item: &str) -> Option<FieldDef> {
    let (name, ty) = match item.split_once(':') {
        Some((n, t)) => (n.trim(), t.trim()),
        None => (item.trim(), ""),
    };
    if name.is_empty() {
        return None;
    }
    let (type_name, enum_values) = if let Some(inner) = ty.strip_prefix("enum") {
        let inner = inner.trim().trim_start_matches('(').trim_end_matches(')');
        ("enum".to_owned(), split_list(inner))
    } else {
        (ty.to_owned(), Vec::new())
    };
    Some(FieldDef {
        name: name.to_owned(),
        type_name,
        enum_values,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const BLOCK: &str = "\
entity Mortgage
aka: loan
describes: A home loan moving through underwriting to closing.
fields:
  credit_score: int
  status: enum(UnderReview, Approved, Rejected)
  employment_verified: bool
  applicant_id
operations:
  approveApplication
  disburseFunds";

    #[test]
    fn extracts_glossary_fence() {
        let md = "# Glossary\n\n```glossary\nentity Order\n```\n\ntext";
        let blocks = extract_glossary_blocks(md);
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].contains("entity Order"));
    }

    #[test]
    fn plain_and_contract_fences_ignored() {
        let md = "```\ncode\n```\n```contract\ncase X\n```";
        assert!(extract_glossary_blocks(md).is_empty());
    }

    #[test]
    fn unclosed_fence_yields_no_block() {
        assert!(extract_glossary_blocks("```glossary\nentity Dangling").is_empty());
    }

    #[test]
    fn parses_entity_name() {
        let e = parse_glossary_block(BLOCK).unwrap();
        assert_eq!(e.name, "Mortgage");
    }

    #[test]
    fn parses_aka_and_describes() {
        let e = parse_glossary_block(BLOCK).unwrap();
        assert_eq!(e.aka, vec!["loan"]);
        assert!(e.describes.unwrap().contains("home loan"));
    }

    #[test]
    fn parses_fields_with_types() {
        let e = parse_glossary_block(BLOCK).unwrap();
        assert_eq!(e.fields.len(), 4);
        assert_eq!(e.field("credit_score").unwrap().type_name, "int");
        assert_eq!(e.field("employment_verified").unwrap().type_name, "bool");
        // untyped field
        assert_eq!(e.field("applicant_id").unwrap().type_name, "");
    }

    #[test]
    fn parses_enum_members() {
        let e = parse_glossary_block(BLOCK).unwrap();
        let status = e.field("status").unwrap();
        assert_eq!(status.type_name, "enum");
        assert_eq!(status.enum_values, vec!["UnderReview", "Approved", "Rejected"]);
    }

    #[test]
    fn parses_operations() {
        let e = parse_glossary_block(BLOCK).unwrap();
        assert_eq!(e.operations, vec!["approveApplication", "disburseFunds"]);
    }

    #[test]
    fn block_without_entity_header_is_none() {
        assert!(parse_glossary_block("aka: loan\nfields:\n  x: int").is_none());
    }

    #[test]
    fn aka_accepts_comma_list() {
        let e = parse_glossary_block("entity Order\naka: order, ord").unwrap();
        assert_eq!(e.aka, vec!["order", "ord"]);
    }
}

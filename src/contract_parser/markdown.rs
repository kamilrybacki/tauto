use crate::contract_ir::SourceLocation;

#[derive(Debug, Clone, PartialEq)]
pub struct ContractBlock {
    pub raw_block: String,
    pub source: SourceLocation,
}

pub fn extract_contract_blocks(markdown: &str, document_path: &str) -> Vec<ContractBlock> {
    let lines: Vec<&str> = markdown.lines().collect();
    let mut blocks = Vec::new();
    let mut in_block = false;
    let mut start_line: u32 = 0;
    let mut block_lines: Vec<&str> = Vec::new();

    for (idx, &line) in lines.iter().enumerate() {
        let line_number = (idx + 1) as u32;
        if !in_block && line.trim() == "```contract" {
            in_block = true;
            start_line = line_number + 1;
            block_lines.clear();
            continue;
        }
        if in_block && line.trim() == "```" {
            blocks.push(ContractBlock {
                raw_block: block_lines.join("\n"),
                source: SourceLocation {
                    document_path: document_path.to_owned(),
                    start_line,
                    end_line: line_number - 1,
                },
            });
            in_block = false;
            continue;
        }
        if in_block {
            block_lines.push(line);
        }
    }

    blocks
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIMPLE_DOC: &str = "\
# Orders

```contract
case CancelPaidOrder
entity:
  Order
operation:
  cancelOrder
```

Some text after.
";

    const TWO_BLOCKS: &str = "\
```contract
case First
entity:
  E
operation:
  op
```

```contract
case Second
entity:
  F
operation:
  op2
```
";

    #[test]
    fn extracts_one_block_from_simple_doc() {
        let blocks = extract_contract_blocks(SIMPLE_DOC, "spec.md");
        assert_eq!(blocks.len(), 1);
    }

    #[test]
    fn block_raw_content_matches_inner_lines() {
        let blocks = extract_contract_blocks(SIMPLE_DOC, "spec.md");
        assert!(blocks[0].raw_block.contains("case CancelPaidOrder"));
    }

    #[test]
    fn source_document_path_is_set() {
        let blocks = extract_contract_blocks(SIMPLE_DOC, "spec.md");
        assert_eq!(blocks[0].source.document_path, "spec.md");
    }

    #[test]
    fn source_start_line_points_after_fence() {
        let blocks = extract_contract_blocks(SIMPLE_DOC, "spec.md");
        // ```contract is line 3 → start_line = 4
        assert_eq!(blocks[0].source.start_line, 4);
    }

    #[test]
    fn source_end_line_points_before_closing_fence() {
        let blocks = extract_contract_blocks(SIMPLE_DOC, "spec.md");
        // closing ``` is line 9 → end_line = 8
        assert_eq!(blocks[0].source.end_line, 8);
    }

    #[test]
    fn extracts_two_blocks() {
        let blocks = extract_contract_blocks(TWO_BLOCKS, "multi.md");
        assert_eq!(blocks.len(), 2);
    }

    #[test]
    fn no_contract_fences_returns_empty() {
        let blocks = extract_contract_blocks("# No contracts here\n\nJust prose.", "empty.md");
        assert!(blocks.is_empty());
    }

    #[test]
    fn unclosed_fence_is_ignored() {
        let unclosed = "```contract\ncase Dangling\n";
        let blocks = extract_contract_blocks(unclosed, "bad.md");
        assert!(blocks.is_empty(), "unclosed fence must not produce a block");
    }

    #[test]
    fn plain_backtick_fence_not_extracted() {
        let doc = "```\nsome code\n```\n";
        let blocks = extract_contract_blocks(doc, "code.md");
        assert!(blocks.is_empty());
    }
}

from tauto_contract_parser.markdown import extract_contract_blocks


def test_empty_markdown_has_no_contract_blocks() -> None:
    assert extract_contract_blocks("# Rules\n", "rules.md") == []


def test_extracts_contract_block_with_source_lines() -> None:
    markdown = """# Cancel paid order

Intro text.

```contract
case CancelPaidOrder

entity:
  Order
```
"""

    blocks = extract_contract_blocks(markdown, "rules.md")

    assert len(blocks) == 1
    assert blocks[0].raw_block == "case CancelPaidOrder\n\nentity:\n  Order"
    assert blocks[0].source.document_path == "rules.md"
    assert blocks[0].source.start_line == 5
    assert blocks[0].source.end_line == 9

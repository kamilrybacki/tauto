from tauto_contract_ir.models import SourceLocation
from tauto_contract_ir.serialization import semantic_contract_set_hash
from tauto_contract_parser.dsl import parse_contract_block
from tauto_contract_parser.markdown import ContractBlock, extract_contract_blocks


_COMPACT_BLOCK = """\
case CancelPaidOrder
entity:
  Order
operation:
  cancelOrder
requires:
  order.status == Paid
"""

_VERBOSE_BLOCK = """\
case CancelPaidOrder


entity:
  Order


operation:
  cancelOrder


requires:
  order.status == Paid

"""


def _parse_block(raw: str, start_line: int) -> object:
    block = ContractBlock(
        raw_block=raw,
        source=SourceLocation(
            document_path="rules.md",
            start_line=start_line,
            end_line=start_line + len(raw.splitlines()),
        ),
    )
    result = parse_contract_block(block)
    assert result.contract is not None, result.diagnostics
    return result.contract


def test_semantic_hash_stable_across_formatting_noise() -> None:
    compact = _parse_block(_COMPACT_BLOCK, start_line=1)
    verbose = _parse_block(_VERBOSE_BLOCK, start_line=100)

    from tauto_contract_ir.models import ContractSet

    set_a = ContractSet(contracts=[compact])
    set_b = ContractSet(contracts=[verbose])

    assert semantic_contract_set_hash(set_a) == semantic_contract_set_hash(set_b)


def test_semantic_hash_differs_on_semantic_change() -> None:
    original = _parse_block(_COMPACT_BLOCK, start_line=1)
    modified_block = _COMPACT_BLOCK.replace("Paid", "Pending")
    modified = _parse_block(modified_block, start_line=1)

    from tauto_contract_ir.models import ContractSet

    set_a = ContractSet(contracts=[original])
    set_b = ContractSet(contracts=[modified])

    assert semantic_contract_set_hash(set_a) != semantic_contract_set_hash(set_b)


def test_semantic_hash_stable_across_markdown_line_offsets() -> None:
    md_early = "# Section\n\n```contract\n" + _COMPACT_BLOCK + "```\n"
    md_late = "# Section\n\n" + ("\n" * 50) + "```contract\n" + _COMPACT_BLOCK + "```\n"

    blocks_early = extract_contract_blocks(md_early, "rules.md")
    blocks_late = extract_contract_blocks(md_late, "rules.md")

    assert len(blocks_early) == 1
    assert len(blocks_late) == 1

    contract_early = parse_contract_block(blocks_early[0]).contract
    contract_late = parse_contract_block(blocks_late[0]).contract

    assert contract_early is not None
    assert contract_late is not None

    from tauto_contract_ir.models import ContractSet

    assert semantic_contract_set_hash(
        ContractSet(contracts=[contract_early])
    ) == semantic_contract_set_hash(
        ContractSet(contracts=[contract_late])
    )

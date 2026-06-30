from tauto_contract_ir.models import SourceLocation
from tauto_contract_parser.dsl import parse_contract_block
from tauto_contract_parser.markdown import ContractBlock


def test_parse_cancel_paid_order_contract() -> None:
    block = ContractBlock(
        raw_block="""case CancelPaidOrder

entity:
  Order

operation:
  cancelOrder

requires:
  order.status == Paid
  order.shipped == false

ensures:
  result.status == Cancelled
  result.total == order.total
  result.id == order.id

forbidden:
  shipOrder(result)

preserves:
  ValidOrder
  CancelledOrderCannotShip
""",
        source=SourceLocation(document_path="rules.md", start_line=1, end_line=24),
    )

    parsed = parse_contract_block(block)

    assert parsed.diagnostics == []
    assert parsed.contract is not None
    assert parsed.contract.case == "CancelPaidOrder"
    assert parsed.contract.entity == "Order"
    assert parsed.contract.operation == "cancelOrder"
    assert parsed.contract.requires[1].right.kind == "bool"
    assert parsed.contract.requires[1].right.value is False
    assert parsed.contract.ensures[1].right.kind == "field"
    assert parsed.contract.forbidden[0].operation == "shipOrder"
    assert parsed.contract.preserves == ["ValidOrder", "CancelledOrderCannotShip"]


def test_parse_reports_malformed_condition_with_source_line() -> None:
    block = ContractBlock(
        raw_block="""case CancelPaidOrder
entity:
  Order
operation:
  cancelOrder
requires:
  order.status Paid
""",
        source=SourceLocation(document_path="rules.md", start_line=10, end_line=16),
    )

    parsed = parse_contract_block(block)

    assert parsed.contract is None
    assert parsed.diagnostics[0].category == "parse_error"
    assert parsed.diagnostics[0].line == 16
    assert "Malformed condition" in parsed.diagnostics[0].message

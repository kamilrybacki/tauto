from tauto_contract_ir.models import (
    Condition,
    ContractIR,
    ContractSet,
    Expression,
    ForbiddenOperation,
    SourceLocation,
)


def test_zero_contract_set_is_valid() -> None:
    contract_set = ContractSet()

    assert contract_set.schema_version == 1
    assert contract_set.contracts == []


def test_contract_ir_holds_language_independent_contract_parts() -> None:
    contract = ContractIR(
        case="CancelPaidOrder",
        entity="Order",
        operation="cancelOrder",
        requires=[
            Condition(
                left=Expression(kind="field", value="order.status"),
                operator="==",
                right=Expression(kind="enum", value="Paid"),
            )
        ],
        ensures=[
            Condition(
                left=Expression(kind="field", value="result.total"),
                operator="==",
                right=Expression(kind="field", value="order.total"),
            )
        ],
        forbidden=[
            ForbiddenOperation(
                operation="shipOrder",
                args=[Expression(kind="variable", value="result")],
            )
        ],
        preserves=["ValidOrder"],
        assumes=[],
        source=SourceLocation(
            document_path="business-cases/orders/cancel-paid-order.md",
            start_line=3,
            end_line=22,
        ),
    )

    assert contract.requires[0].left.kind == "field"
    assert contract.forbidden[0].operation == "shipOrder"
    assert contract.source.start_line == 3

from tauto_contract_ir.models import (
    Condition,
    ContractIR,
    ContractSet,
    Expression,
    ForbiddenOperation,
)
from tauto_lean_gen.workspace import LeanWorkspace, LeanWorkspaceFile, generate_lean_workspace


def _cancel_paid_order() -> ContractIR:
    return ContractIR(
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
                left=Expression(kind="field", value="result.status"),
                operator="==",
                right=Expression(kind="enum", value="Cancelled"),
            )
        ],
        forbidden=[ForbiddenOperation(operation="shipOrder", args=[])],
        preserves=["ValidOrder"],
    )


def test_workspace_has_lakefile() -> None:
    ws = generate_lean_workspace(ContractSet(contracts=[_cancel_paid_order()]))
    paths = {f.path for f in ws.files}
    assert "lakefile.toml" in paths


def test_workspace_has_main_module() -> None:
    ws = generate_lean_workspace(ContractSet(contracts=[_cancel_paid_order()]))
    paths = {f.path for f in ws.files}
    assert any("TautoContracts.lean" in p for p in paths)


def test_workspace_has_one_file_per_contract() -> None:
    cs = ContractSet(contracts=[_cancel_paid_order()])
    ws = generate_lean_workspace(cs)
    lean_files = [f for f in ws.files if f.path.endswith(".lean") and "TautoContracts" not in f.path]
    assert len(lean_files) == 1


def test_contract_file_contains_namespace() -> None:
    ws = generate_lean_workspace(ContractSet(contracts=[_cancel_paid_order()]))
    contract_file = next(
        f for f in ws.files
        if f.path.endswith(".lean") and "CancelPaidOrder" in f.path
    )
    assert "namespace" in contract_file.content
    assert "CancelPaidOrder" in contract_file.content


def test_contract_file_contains_requires_theorem() -> None:
    ws = generate_lean_workspace(ContractSet(contracts=[_cancel_paid_order()]))
    contract_file = next(
        f for f in ws.files
        if f.path.endswith(".lean") and "CancelPaidOrder" in f.path
    )
    assert "theorem" in contract_file.content
    assert "requires" in contract_file.content.lower() or "requires" in contract_file.content


def test_contract_file_contains_forbidden_comment() -> None:
    ws = generate_lean_workspace(ContractSet(contracts=[_cancel_paid_order()]))
    contract_file = next(
        f for f in ws.files
        if f.path.endswith(".lean") and "CancelPaidOrder" in f.path
    )
    assert "shipOrder" in contract_file.content


def test_contract_file_contains_sorry() -> None:
    ws = generate_lean_workspace(ContractSet(contracts=[_cancel_paid_order()]))
    contract_file = next(
        f for f in ws.files
        if f.path.endswith(".lean") and "CancelPaidOrder" in f.path
    )
    assert "sorry" in contract_file.content


def test_workspace_is_deterministic() -> None:
    cs = ContractSet(contracts=[_cancel_paid_order()])
    ws_a = generate_lean_workspace(cs)
    ws_b = generate_lean_workspace(cs)
    assert sorted(f.path for f in ws_a.files) == sorted(f.path for f in ws_b.files)
    for fa, fb in zip(
        sorted(ws_a.files, key=lambda f: f.path),
        sorted(ws_b.files, key=lambda f: f.path),
    ):
        assert fa.content == fb.content


def test_workspace_scales_to_multiple_contracts() -> None:
    cs = ContractSet(
        contracts=[
            ContractIR(case="CancelPaidOrder", entity="Order", operation="cancelOrder"),
            ContractIR(case="ShipReadyOrder", entity="Order", operation="shipOrder"),
        ]
    )
    ws = generate_lean_workspace(cs)
    lean_files = [f for f in ws.files if f.path.endswith(".lean") and "TautoContracts" not in f.path]
    assert len(lean_files) == 2


def test_workspace_files_are_immutable() -> None:
    ws = generate_lean_workspace(ContractSet(contracts=[_cancel_paid_order()]))
    import pydantic
    try:
        ws.files = []  # type: ignore[misc]
        raise AssertionError("should be frozen")
    except (pydantic.ValidationError, TypeError, AttributeError):
        pass

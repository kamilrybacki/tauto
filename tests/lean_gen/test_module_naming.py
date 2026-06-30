import pytest

from tauto_contract_ir.models import ContractIR, ContractSet
from tauto_lean_gen.workspace import generate_lean_workspace


def _ws(case: str):
    return generate_lean_workspace(
        ContractSet(contracts=[ContractIR(case=case, entity="E", operation="op")])
    )


def _contract_paths(case: str) -> set[str]:
    ws = _ws(case)
    return {f.path for f in ws.files if f.path.endswith(".lean") and "TautoContracts" not in f.path}


def test_digit_leading_case_produces_valid_path() -> None:
    paths = _contract_paths("1InvalidStart")
    assert len(paths) == 1
    path = next(iter(paths))
    stem = path.split("/")[-1].removesuffix(".lean")
    assert stem[0].isalpha() or stem[0] == "_", f"Invalid leading char in {stem!r}"


def test_hyphenated_case_produces_valid_path() -> None:
    paths = _contract_paths("Cancel-Paid-Order")
    assert len(paths) == 1
    path = next(iter(paths))
    stem = path.split("/")[-1].removesuffix(".lean")
    assert all(c.isalnum() or c == "_" for c in stem)


def test_distinct_cases_do_not_collide() -> None:
    cs = ContractSet(
        contracts=[
            ContractIR(case="A-B", entity="E", operation="op"),
            ContractIR(case="AB", entity="E", operation="op"),
        ]
    )
    ws = generate_lean_workspace(cs)
    lean_paths = [f.path for f in ws.files if f.path.endswith(".lean") and "TautoContracts" not in f.path]
    assert len(lean_paths) == len(set(lean_paths)), f"Colliding paths: {lean_paths}"


def test_operation_with_spaces_produces_valid_theorem_name() -> None:
    ws = generate_lean_workspace(
        ContractSet(
            contracts=[ContractIR(case="MyContract", entity="E", operation="cancel order")]
        )
    )
    contract_file = next(f for f in ws.files if "MyContract" in f.path and f.path.endswith(".lean"))
    # theorem names must contain only alphanumeric and underscore
    import re
    theorem_lines = [l for l in contract_file.content.splitlines() if l.startswith("theorem ")]
    for line in theorem_lines:
        name = line.split()[1].rstrip(":")
        assert re.match(r"^[A-Za-z_][A-Za-z0-9_]*$", name), f"Invalid theorem name: {name!r}"

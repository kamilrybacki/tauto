from tauto_contract_ir import ContractIR, ContractSet
from tauto_contract_parser import extract_contract_blocks, parse_contract_block


def test_tauto_packages_export_core_symbols() -> None:
    assert ContractSet().contracts == []
    assert ContractIR(case="A", entity="Order", operation="cancelOrder").case == "A"
    assert extract_contract_blocks("# Empty\n", "rules.md") == []
    assert callable(parse_contract_block)

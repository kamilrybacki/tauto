from tauto_contract_ir.models import ContractSet


def test_zero_contract_set_is_valid() -> None:
    contract_set = ContractSet()

    assert contract_set.schema_version == 1
    assert contract_set.contracts == []

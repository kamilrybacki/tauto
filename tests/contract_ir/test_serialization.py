from tauto_contract_ir.models import ContractIR, ContractSet
from tauto_contract_ir.serialization import canonical_contract_set_json, contract_set_hash


def test_zero_contract_set_canonical_json_is_stable() -> None:
    assert canonical_contract_set_json(ContractSet()) == '{"contracts":[],"schema_version":1}'


def test_contract_set_hash_changes_when_semantic_content_changes() -> None:
    first = ContractSet(contracts=[ContractIR(case="A", entity="Order", operation="cancelOrder")])
    second = ContractSet(contracts=[ContractIR(case="B", entity="Order", operation="cancelOrder")])

    assert contract_set_hash(first) != contract_set_hash(second)

import hashlib
import json

from tauto_contract_ir.models import ContractIR, ContractSet


def canonical_contract_json(contract: ContractIR) -> str:
    return json.dumps(
        contract.model_dump(exclude_none=True),
        sort_keys=True,
        separators=(",", ":"),
    )


def canonical_contract_set_json(contract_set: ContractSet) -> str:
    return json.dumps(
        contract_set.model_dump(exclude_none=True),
        sort_keys=True,
        separators=(",", ":"),
    )


def contract_hash(contract: ContractIR) -> str:
    return hashlib.sha256(canonical_contract_json(contract).encode("utf-8")).hexdigest()


def contract_set_hash(contract_set: ContractSet) -> str:
    return hashlib.sha256(canonical_contract_set_json(contract_set).encode("utf-8")).hexdigest()


def _semantic_contract_dict(contract: ContractIR) -> dict:
    return contract.model_dump(exclude={"source"}, exclude_none=True)


def semantic_contract_set_json(contract_set: ContractSet) -> str:
    payload = {
        "schema_version": contract_set.schema_version,
        "contracts": [_semantic_contract_dict(c) for c in contract_set.contracts],
    }
    return json.dumps(payload, sort_keys=True, separators=(",", ":"))


def semantic_contract_set_hash(contract_set: ContractSet) -> str:
    return hashlib.sha256(semantic_contract_set_json(contract_set).encode("utf-8")).hexdigest()

import hashlib
import json

from pydantic import BaseModel, ConfigDict

from tauto_contract_ir import ContractSet, semantic_contract_set_hash


class DeterministicContext(BaseModel):
    model_config = ConfigDict(frozen=True)

    entries: dict[str, str]
    context_hash: str


def build_deterministic_context(
    contract_set: ContractSet,
    *,
    generator_intent: str,
) -> DeterministicContext:
    entries: dict[str, str] = dict(
        sorted(
            {
                "contract_count": str(len(contract_set.contracts)),
                "contract_set_hash": semantic_contract_set_hash(contract_set),
                "generator_intent": generator_intent,
            }.items()
        )
    )
    context_hash = hashlib.sha256(
        json.dumps(entries, sort_keys=True, separators=(",", ":")).encode("utf-8")
    ).hexdigest()
    return DeterministicContext(entries=entries, context_hash=context_hash)

from pydantic import BaseModel, ConfigDict

from tauto_contract_ir.models import ContractSet
from tauto_contract_ir.serialization import semantic_contract_set_hash
from tauto_preprocessing.context_builder import DeterministicContext
from tauto_slm.provider import ArtifactKind, SlmProviderRef


class ArtifactTraceability(BaseModel):
    model_config = ConfigDict(frozen=True)

    contract_set_hash: str
    provider: SlmProviderRef
    target_language: str
    artifact_kind: ArtifactKind
    deterministic_context_hash: str


def build_traceability(
    *,
    contract_set: ContractSet,
    provider: SlmProviderRef,
    target_language: str,
    artifact_kind: ArtifactKind,
    deterministic_context: DeterministicContext,
) -> ArtifactTraceability:
    return ArtifactTraceability(
        contract_set_hash=semantic_contract_set_hash(contract_set),
        provider=provider,
        target_language=target_language,
        artifact_kind=artifact_kind,
        deterministic_context_hash=deterministic_context.context_hash,
    )

from pydantic import BaseModel, ConfigDict

from tauto_preprocessing.context_builder import DeterministicContext
from tauto_slm.provider import ArtifactKind, SlmProviderRef

__all__ = [
    "ArtifactTraceability",
    "build_traceability",
]


class ArtifactTraceability(BaseModel):
    model_config = ConfigDict(frozen=True)

    contract_set_hash: str
    provider: SlmProviderRef
    target_language: str
    artifact_kind: ArtifactKind
    deterministic_context_hash: str


def build_traceability(
    *,
    provider: SlmProviderRef,
    target_language: str,
    artifact_kind: ArtifactKind,
    deterministic_context: DeterministicContext,
) -> ArtifactTraceability:
    return ArtifactTraceability(
        contract_set_hash=deterministic_context.entries["contract_set_hash"],
        provider=provider,
        target_language=target_language,
        artifact_kind=artifact_kind,
        deterministic_context_hash=deterministic_context.context_hash,
    )

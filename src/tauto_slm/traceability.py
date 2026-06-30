from pydantic import BaseModel, ConfigDict

from tauto_slm.provider import ArtifactKind, SlmProviderRef

__all__ = [
    "ArtifactTraceability",
    "build_traceability",
]


class ArtifactTraceability(BaseModel):
    model_config = ConfigDict(frozen=True)

    contract_set_hash: str
    provider: SlmProviderRef | None = None
    target_language: str
    artifact_kind: ArtifactKind
    deterministic_context_hash: str


def build_traceability(
    *,
    contract_set_hash: str,
    deterministic_context_hash: str,
    target_language: str,
    artifact_kind: ArtifactKind,
    provider: SlmProviderRef | None = None,
) -> ArtifactTraceability:
    return ArtifactTraceability(
        contract_set_hash=contract_set_hash,
        provider=provider,
        target_language=target_language,
        artifact_kind=artifact_kind,
        deterministic_context_hash=deterministic_context_hash,
    )

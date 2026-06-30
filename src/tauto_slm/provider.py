from typing import Literal, Protocol

from pydantic import BaseModel, ConfigDict, Field

from tauto_contract_ir import ContractSet, Diagnostic


ArtifactKind = Literal["validator", "test", "implementation", "schema", "summary"]


class SlmProviderRef(BaseModel):
    model_config = ConfigDict(frozen=True)

    name: str
    model: str
    endpoint: str | None = None


class GeneratedCodeArtifact(BaseModel):
    model_config = ConfigDict(frozen=True)

    language: str
    path: str
    content: str


class AstCodeGenerationRequest(BaseModel):
    model_config = ConfigDict(frozen=True)

    contract_set: ContractSet
    target_language: str
    artifact_kind: ArtifactKind
    instructions: str = ""
    deterministic_context: dict[str, str] = Field(default_factory=dict)


class AstCodeGenerationResult(BaseModel):
    model_config = ConfigDict(frozen=True)

    provider: SlmProviderRef
    artifacts: list[GeneratedCodeArtifact] = Field(default_factory=list)
    diagnostics: list[Diagnostic] = Field(default_factory=list)


class SlmCodeGenerator(Protocol):
    def generate_code_from_ast(
        self, request: AstCodeGenerationRequest
    ) -> AstCodeGenerationResult:
        """Generate code artifacts from normalized Tauto AST/IR."""


def generate_code_from_ast(
    provider: SlmCodeGenerator,
    request: AstCodeGenerationRequest,
) -> AstCodeGenerationResult:
    return provider.generate_code_from_ast(request)

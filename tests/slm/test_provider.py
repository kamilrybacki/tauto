from tauto_contract_ir import ContractIR, ContractSet
from tauto_slm import (
    AstCodeGenerationRequest,
    AstCodeGenerationResult,
    GeneratedCodeArtifact,
    SlmProviderRef,
    generate_code_from_ast,
)


class FakeProvider:
    def __init__(self) -> None:
        self.received_request: AstCodeGenerationRequest | None = None

    def generate_code_from_ast(
        self, request: AstCodeGenerationRequest
    ) -> AstCodeGenerationResult:
        self.received_request = request
        return AstCodeGenerationResult(
            provider=SlmProviderRef(name="local", model="fake-codegen"),
            artifacts=[
                GeneratedCodeArtifact(
                    language=request.target_language,
                    path="validators/order_validators.py",
                    content="# generated from AST\n",
                )
            ],
            diagnostics=[],
        )


def test_generate_code_from_ast_delegates_to_provider_without_provider_coupling() -> None:
    contract_set = ContractSet(
        contracts=[ContractIR(case="CancelPaidOrder", entity="Order", operation="cancelOrder")]
    )
    request = AstCodeGenerationRequest(
        contract_set=contract_set,
        target_language="python",
        artifact_kind="validator",
        instructions="Generate runtime validators only.",
    )
    provider = FakeProvider()

    result = generate_code_from_ast(provider, request)

    assert provider.received_request == request
    assert result.provider.name == "local"
    assert result.artifacts[0].language == "python"
    assert result.artifacts[0].content == "# generated from AST\n"

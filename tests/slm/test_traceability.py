from tauto_contract_ir.models import ContractIR, ContractSet
from tauto_preprocessing.context_builder import build_deterministic_context
from tauto_slm.traceability import ArtifactTraceability, build_traceability
from tauto_slm.provider import ArtifactKind, SlmProviderRef


def _make_set() -> ContractSet:
    return ContractSet(
        contracts=[ContractIR(case="CancelPaidOrder", entity="Order", operation="cancelOrder")]
    )


def _provider() -> SlmProviderRef:
    return SlmProviderRef(name="deepseek", model="deepseek-coder-v2", endpoint=None)


def test_traceability_carries_all_metadata() -> None:
    cs = _make_set()
    ctx = build_deterministic_context(cs, generator_intent="lean_verification")
    t = build_traceability(
        contract_set=cs,
        provider=_provider(),
        target_language="lean4",
        artifact_kind="validator",
        deterministic_context=ctx,
    )

    assert t.contract_set_hash == ctx.entries["contract_set_hash"]
    assert t.provider.name == "deepseek"
    assert t.target_language == "lean4"
    assert t.artifact_kind == "validator"
    assert t.deterministic_context_hash == ctx.context_hash


def test_traceability_is_immutable() -> None:
    cs = _make_set()
    ctx = build_deterministic_context(cs, generator_intent="lean_verification")
    t = build_traceability(
        contract_set=cs,
        provider=_provider(),
        target_language="lean4",
        artifact_kind="validator",
        deterministic_context=ctx,
    )

    import pydantic

    try:
        t.target_language = "python"  # type: ignore[misc]
        raise AssertionError("should be frozen")
    except (pydantic.ValidationError, TypeError):
        pass


def test_traceability_differs_on_provider_change() -> None:
    cs = _make_set()
    ctx = build_deterministic_context(cs, generator_intent="lean_verification")
    t_a = build_traceability(
        contract_set=cs,
        provider=SlmProviderRef(name="deepseek", model="v2"),
        target_language="lean4",
        artifact_kind="validator",
        deterministic_context=ctx,
    )
    t_b = build_traceability(
        contract_set=cs,
        provider=SlmProviderRef(name="anthropic", model="claude-opus-4-8"),
        target_language="lean4",
        artifact_kind="validator",
        deterministic_context=ctx,
    )

    assert t_a.provider.name != t_b.provider.name
    assert t_a.contract_set_hash == t_b.contract_set_hash
    assert t_a.deterministic_context_hash == t_b.deterministic_context_hash

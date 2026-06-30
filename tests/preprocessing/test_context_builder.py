import pytest

from tauto_contract_ir.models import ContractIR, ContractSet, SourceLocation
from tauto_preprocessing.context_builder import (
    DeterministicContext,
    build_deterministic_context,
)


def _make_set(*cases: str) -> ContractSet:
    return ContractSet(
        contracts=[
            ContractIR(case=c, entity="Order", operation="cancelOrder") for c in cases
        ]
    )


def test_context_has_contract_set_hash() -> None:
    cs = _make_set("CancelPaidOrder")
    ctx = build_deterministic_context(cs, generator_intent="lean_verification")
    assert "contract_set_hash" in ctx.entries
    assert len(ctx.entries["contract_set_hash"]) == 64  # sha256 hex


def test_context_has_generator_intent() -> None:
    cs = _make_set("CancelPaidOrder")
    ctx = build_deterministic_context(cs, generator_intent="lean_verification")
    assert ctx.entries["generator_intent"] == "lean_verification"


def test_context_has_contract_count() -> None:
    cs = _make_set("A", "B", "C")
    ctx = build_deterministic_context(cs, generator_intent="lean_verification")
    assert ctx.entries["contract_count"] == "3"


def test_context_is_stable_across_source_locations() -> None:
    cs_with_source = ContractSet(
        contracts=[
            ContractIR(
                case="CancelPaidOrder",
                entity="Order",
                operation="cancelOrder",
                source=SourceLocation(document_path="rules.md", start_line=1, end_line=10),
            )
        ]
    )
    cs_different_source = ContractSet(
        contracts=[
            ContractIR(
                case="CancelPaidOrder",
                entity="Order",
                operation="cancelOrder",
                source=SourceLocation(document_path="other.md", start_line=99, end_line=110),
            )
        ]
    )

    ctx_a = build_deterministic_context(cs_with_source, generator_intent="lean_verification")
    ctx_b = build_deterministic_context(cs_different_source, generator_intent="lean_verification")

    assert ctx_a.entries["contract_set_hash"] == ctx_b.entries["contract_set_hash"]
    assert ctx_a.context_hash == ctx_b.context_hash


def test_context_hash_changes_when_content_changes() -> None:
    ctx_a = build_deterministic_context(_make_set("A"), generator_intent="lean_verification")
    ctx_b = build_deterministic_context(_make_set("B"), generator_intent="lean_verification")
    assert ctx_a.context_hash != ctx_b.context_hash


def test_context_hash_changes_when_intent_changes() -> None:
    cs = _make_set("A")
    ctx_a = build_deterministic_context(cs, generator_intent="lean_verification")
    ctx_b = build_deterministic_context(cs, generator_intent="runtime_validator")
    assert ctx_a.context_hash != ctx_b.context_hash


def test_context_entries_are_sorted_strings() -> None:
    ctx = build_deterministic_context(_make_set("A"), generator_intent="lean_verification")
    assert all(isinstance(k, str) and isinstance(v, str) for k, v in ctx.entries.items())
    keys = list(ctx.entries.keys())
    assert keys == sorted(keys)

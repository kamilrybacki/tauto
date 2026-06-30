# Tauto Handoff For Claude Code

## Current State

Worktree:

```text
/home/kamil-rybacki/Code/tauto/.worktrees/tauto-phase-0-2
```

Branch:

```text
tauto-phase-0-2
```

Latest commit at handoff:

```text
98e81ff feat: add provider agnostic slm codegen boundary
```

The branch implements the first Tauto slice: Phase 0 foundation, Phase 1 local project/document primitives, Phase 2 contract parser/IR, plus a provider-agnostic SLM AST-to-code generation boundary.

## Product Direction

Tauto is a Lean-backed contract verification platform for business rules.

Important architecture decisions:

- The source of truth is a language-independent `ContractIR` / `ContractSet`.
- Lean is a future verification backend generated from the IR, not the core representation.
- SLM support is required for AST/IR-to-code generation, but must be provider agnostic.
- Deterministic preprocessing comes before SLM codegen.
- Generated code must remain traceable to normalized IR, deterministic context, provider metadata, and later validation results.
- The deterministic core must not call OpenAI, Anthropic, Deepseek, local runtime, or self-trained model SDKs directly.

Provider adapters should plug into the `tauto_slm` protocol. This should support self-trained models, locally hosted models, and external providers such as Deepseek Flash.

## Implemented Packages

### `tauto_contract_ir`

Files:

- `src/tauto_contract_ir/models.py`
- `src/tauto_contract_ir/serialization.py`
- `src/tauto_contract_ir/__init__.py`

Provides:

- `Expression`
- `Condition`
- `ForbiddenOperation`
- `SourceLocation`
- `Diagnostic`
- `ContractIR`
- `ContractSet`
- canonical JSON helpers
- SHA-256 hash helpers

The IR is intentionally language-independent. Do not add Lean-specific or Python-specific concepts here.

### `tauto_contract_parser`

Files:

- `src/tauto_contract_parser/markdown.py`
- `src/tauto_contract_parser/dsl.py`
- `src/tauto_contract_parser/__init__.py`

Provides:

- Markdown fenced `contract` block extraction.
- Minimal DSL parser for:
  - `case`
  - `entity`
  - `operation`
  - `requires`
  - `ensures`
  - `forbidden`
  - `preserves`
  - `assumes`
- Structured parse diagnostics.

Important bug already fixed:

- Extracted Markdown diagnostics now point to the actual contract line, not the opening fence.
- Unknown sections such as `requirez:` now produce a parse diagnostic instead of silently dropping content.

### `tauto_project_store`

Files:

- `src/tauto_project_store/models.py`
- `src/tauto_project_store/file_store.py`
- `src/tauto_project_store/__init__.py`

Provides immutable local models:

- `Project`
- `ContractDocument`

And simple file-backed helpers:

- `save_document`
- `load_document`

This is intentionally not a database layer.

### `tauto_slm`

Files:

- `src/tauto_slm/provider.py`
- `src/tauto_slm/__init__.py`

Provides the provider-neutral AST-to-code generation seam:

- `AstCodeGenerationRequest`
- `AstCodeGenerationResult`
- `GeneratedCodeArtifact`
- `SlmProviderRef`
- `SlmCodeGenerator`
- `generate_code_from_ast`

Adapters for specific providers should implement `SlmCodeGenerator`. Keep provider SDK imports out of the deterministic core.

## Planning Docs

Design and phase plans live in:

- `docs/superpowers/specs/2026-06-30-tauto-phase-0-2-design.md`
- `docs/superpowers/plans/2026-06-30-tauto-phase-0-foundation.md`
- `docs/superpowers/plans/2026-06-30-tauto-phase-1-project-document-store.md`
- `docs/superpowers/plans/2026-06-30-tauto-phase-2-contract-parser-ir.md`

The design spec has been updated to require provider-agnostic SLM AST/IR-to-code generation after deterministic preprocessing.

## Tests

Current test suites:

- `tests/contract_ir/test_models.py`
- `tests/contract_ir/test_serialization.py`
- `tests/contract_parser/test_markdown.py`
- `tests/contract_parser/test_dsl.py`
- `tests/project_store/test_models.py`
- `tests/project_store/test_file_store.py`
- `tests/slm/test_provider.py`
- `tests/test_imports.py`

Last verified command:

```bash
pytest -q
```

Last result:

```text
16 passed
```

Ruff is configured in `pyproject.toml`, but is not installed in the ambient environment:

```text
/usr/bin/python: No module named ruff
```

Install dev dependencies before relying on lint status.

## Known Gaps

Not implemented yet:

- Lean workspace generation.
- Lean safety scanning.
- Runtime validator generation backend.
- Test generation backend.
- Concrete SLM provider adapters.
- Deterministic preprocessing builder for SLM requests beyond the current typed request field.
- API server.
- UI.
- PostgreSQL persistence.
- Worker orchestration.
- CI/GitHub integration.

Reviewer residual gaps:

- No direct test yet that canonical JSON/hash is stable across equivalent formatting noise after parsing.
- Lint status is unverified until Ruff is installed.

## Recommended Next Phase

Recommended next work is Phase 3: Lean workspace generation.

Suggested order:

1. Add tests for parsed contract formatting-noise stability:
   - two differently formatted Markdown contract blocks;
   - same semantic `ContractIR`;
   - same canonical JSON/hash.
2. Add deterministic preprocessing package for codegen:
   - input: `ContractSet`;
   - output: stable context map for SLM requests;
   - include contract-set hash and generator intent.
3. Define artifact traceability metadata:
   - contract set hash;
   - provider ref;
   - target language;
   - artifact kind;
   - deterministic context hash.
4. Start Lean generation package:
   - no Lean execution yet;
   - generate deterministic workspace files from `ContractSet`;
   - keep Lean generator separate from IR and parser.

## Development Rules To Preserve

- Use TDD: write failing tests first, then minimal code.
- Keep pure functional core behavior in package functions.
- Keep IO at boundaries.
- Keep SLM providers behind `tauto_slm` interfaces.
- Do not let SLM output become trusted without deterministic validation.
- Do not add Lean-specific concepts to `tauto_contract_ir`.
- Keep user-facing parser failures as structured `Diagnostic` values.


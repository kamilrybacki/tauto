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
2b9c2b5 fix: address code review findings from phase 3
```

Phase 3 is complete. The branch now includes Phase 0 foundation, Phase 1 local
project/document primitives, Phase 2 contract parser/IR, Phase 2.5 provider-agnostic
SLM codegen boundary, and Phase 3 semantic hash, deterministic preprocessing,
artifact traceability, and Lean workspace generation.

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

- `Expression`, `Condition`, `ForbiddenOperation`, `SourceLocation`, `Diagnostic`
- `ContractIR`, `ContractSet`
- `canonical_contract_json` / `canonical_contract_set_json` — includes `source`
- `contract_hash` / `contract_set_hash` — includes `source` (provenance hash)
- `semantic_contract_set_json` / `semantic_contract_set_hash` — **excludes `source`**
  for stable caching and traceability keys across formatting changes

The IR is intentionally language-independent. Do not add Lean-specific or Python-specific concepts here.

### `tauto_contract_parser`

Files:

- `src/tauto_contract_parser/markdown.py`
- `src/tauto_contract_parser/dsl.py`
- `src/tauto_contract_parser/__init__.py`

Provides:

- Markdown fenced `contract` block extraction.
- Minimal DSL parser for: `case`, `entity`, `operation`, `requires`, `ensures`,
  `forbidden`, `preserves`, `assumes`
- Structured parse diagnostics with source line numbers.

### `tauto_project_store`

Files:

- `src/tauto_project_store/models.py`
- `src/tauto_project_store/file_store.py`
- `src/tauto_project_store/__init__.py`

Provides immutable local models (`Project`, `ContractDocument`) and simple
file-backed helpers (`save_document`, `load_document`).

### `tauto_slm`

Files:

- `src/tauto_slm/provider.py`
- `src/tauto_slm/traceability.py`
- `src/tauto_slm/__init__.py`

Provides the provider-neutral AST-to-code generation seam:

- `AstCodeGenerationRequest`, `AstCodeGenerationResult`, `GeneratedCodeArtifact`
- `SlmProviderRef`, `SlmCodeGenerator`, `generate_code_from_ast`
- `ArtifactTraceability`, `build_traceability` — ties contract_set_hash,
  provider, target_language, artifact_kind, and deterministic_context_hash

`build_traceability` reads `contract_set_hash` from `DeterministicContext.entries`
rather than recomputing it — consistent with the preprocessing layer as the authority.

### `tauto_preprocessing`

Files:

- `src/tauto_preprocessing/context_builder.py`
- `src/tauto_preprocessing/__init__.py`

Provides:

- `DeterministicContext` — frozen Pydantic model; `entries: dict[str, str]` sorted,
  `context_hash: str` SHA-256 over serialized entries
- `build_deterministic_context(contract_set, *, generator_intent)` — builds a stable
  context from a `ContractSet` + intent string. Entries include:
  - `contract_set_hash` (semantic, source-excluded)
  - `contract_count`
  - `generator_intent`

### `tauto_lean_gen`

Files:

- `src/tauto_lean_gen/workspace.py`
- `src/tauto_lean_gen/__init__.py`

Provides deterministic Lean 4 workspace generation from `ContractSet`:

- `LeanWorkspaceFile` — frozen `(path, content)` pair
- `LeanWorkspace` — frozen list of files
- `generate_lean_workspace(contract_set)` — pure function; no IO, no SLM

Generated workspace layout:

```
lakefile.toml
TautoContracts.lean            # import index
contracts/<ModuleName>.lean    # one per contract
```

Each `.lean` file contains:
- namespace declaration
- `theorem <op>_requires :` stub with `sorry`
- `theorem <op>_ensures :` stub with `sorry`
- forbidden/preserves as comments

Identifier sanitization: `_lean_ident()` strips non-alphanumeric characters and
prefixes `C` on digit-leading names. Collision disambiguation: contracts whose
sanitized names collide get `_1`, `_2` suffixes in declaration order.

Imports only `tauto_contract_ir` — no SLM, preprocessing, or provider SDK imports.

## Planning Docs

Design and phase plans live in:

- `docs/superpowers/specs/2026-06-30-tauto-phase-0-2-design.md`
- `docs/superpowers/plans/2026-06-30-tauto-phase-0-foundation.md`
- `docs/superpowers/plans/2026-06-30-tauto-phase-1-project-document-store.md`
- `docs/superpowers/plans/2026-06-30-tauto-phase-2-contract-parser-ir.md`

## Tests

Current test suites:

- `tests/contract_ir/test_models.py`
- `tests/contract_ir/test_serialization.py`
- `tests/contract_ir/test_semantic_hash_stability.py`
- `tests/contract_parser/test_markdown.py`
- `tests/contract_parser/test_dsl.py`
- `tests/project_store/test_models.py`
- `tests/project_store/test_file_store.py`
- `tests/slm/test_provider.py`
- `tests/slm/test_traceability.py`
- `tests/preprocessing/test_context_builder.py`
- `tests/lean_gen/test_workspace.py`
- `tests/lean_gen/test_module_naming.py`
- `tests/test_imports.py`

Last verified command:

```bash
python3 -m pytest /home/kamil-rybacki/Code/tauto/.worktrees/tauto-phase-0-2/ -q
```

Last result:

```text
43 passed
```

Ruff is configured in `pyproject.toml`, but is not installed in the ambient environment:

```text
/usr/bin/python: No module named ruff
```

Install dev dependencies before relying on lint status.

## Known Gaps

Not implemented yet:

- Lean workspace writing to disk (currently pure in-memory; add a thin IO layer
  at boundary, e.g. `write_lean_workspace(ws, path)`)
- Lean safety scanning
- Runtime validator generation backend
- Test generation backend
- Concrete SLM provider adapters
- Expanded deterministic preprocessing (e.g. per-contract normalized summaries
  for richer SLM context)
- API server
- UI
- PostgreSQL persistence
- Worker orchestration
- CI/GitHub integration

When an API server is added, wrap the pure-function packages in a **Service Layer**
that owns error handling, structured logging, and response envelopes — do not grow
these packages to absorb those concerns.

## Recommended Next Phase

Recommended next work is Phase 4: Lean workspace I/O + safety scanning.

Suggested order:

1. Add a thin `write_lean_workspace(ws: LeanWorkspace, base_path: Path) -> None`
   IO boundary in `tauto_lean_gen`; keep it separate from the pure generator.
2. Add Lean safety scanning: parse generated `.lean` files to detect sorry-free
   theorems and report them as diagnostics (pure transform, no Lean execution).
3. Optionally expand `DeterministicContext` to include per-contract normalized
   summaries for richer SLM prompt construction.
4. Add a concrete SLM provider adapter stub (e.g. Deepseek Flash no-op stub
   that returns empty artifacts) to validate the protocol end-to-end.

## Development Rules To Preserve

- Use TDD: write failing tests first, then minimal code.
- Keep pure functional core behavior in package functions.
- Keep IO at boundaries.
- Keep SLM providers behind `tauto_slm` interfaces.
- Do not let SLM output become trusted without deterministic validation.
- Do not add Lean-specific concepts to `tauto_contract_ir`.
- Keep user-facing parser failures as structured `Diagnostic` values.
- `semantic_contract_set_hash` (not `contract_set_hash`) must be used for
  cache keys, traceability, and preprocessing — the semantic hash excludes
  source locations so formatting changes don't invalidate artifacts.
- `DeterministicContext` is the single authority for `contract_set_hash` within
  a generation pipeline — `build_traceability` reads from it, not from the
  contract set directly.

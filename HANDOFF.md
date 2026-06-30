# Tauto — Handoff Document

_Updated after Phase R1 completion_

---

## Decision Log

### Why Rust?

Tauto is a **CLI binary** (`tauto verify contracts/`). Rust was chosen over Python because:
- Algebraic data types and pattern matching are a better fit for IR modeling than Pydantic
- Ships as a single binary with no runtime dependency
- Immutability is guaranteed by the type system, not enforced by convention
- `BTreeMap` replaces `sort_keys=True` JSON hacks — key ordering is structural

The Python prototype (Phases 0-2, 44 tests, 3 reviewed phases) is in git history at `fb9d8a9`.

---

## Current State

**Branch:** `tauto-phase-0-2`
**Tests:** 94 passing
**Binary:** `cargo build` → `./target/debug/tauto`

### CLI Commands

```
tauto verify <path> [--output <dir>] [--strict]
    Parse markdown contracts, generate Lean 4 workspace, scan for sorry stubs.
    --strict exits 1 if any sorry stubs found (for CI gate).

tauto hash <path>
    Print semantic hash (excludes source locations) and provenance hash.
    Semantic hash is stable across reformatting — use as cache key.

tauto list <path>
    List parsed contracts (entity/operation/case + source location).
```

---

## Project Structure

```
Cargo.toml              # single crate, lib + bin
src/
  lib.rs                # pub mod declarations
  main.rs               # CLI: verify / hash / list (clap)
  contract_ir/
    mod.rs
    models.rs           # ContractIR, ContractSet, Condition, Expression,
                        # ExpressionValue, ForbiddenOperation, SourceLocation,
                        # Diagnostic
    serialization.rs    # semantic_contract_set_hash (excludes source),
                        # contract_set_hash (provenance, includes source)
  contract_parser/
    mod.rs
    markdown.rs         # extract_contract_blocks → Vec<ContractBlock>
    dsl.rs              # parse_contract_block → ParseResult
  lean_gen/
    mod.rs
    workspace.rs        # generate_lean_workspace → LeanWorkspace
    safety.rs           # scan_lean_workspace → Vec<Diagnostic>
                        # tokens: sorry/axiom/native_decide/unsafe (word-bounded)
    io.rs               # write_lean_workspace (path-uniqueness assert before write)
  preprocessing/
    mod.rs
    context_builder.rs  # build_deterministic_context → DeterministicContext
                        # (BTreeMap entries — structural key ordering)
  slm/
    mod.rs
    provider.rs         # SlmCodeGenerator trait, ArtifactKind, SlmProviderRef,
                        # CodeGenerationRequest, GeneratedArtifact
    traceability.rs     # build_traceability → ArtifactTraceability
                        # (provider: Option<SlmProviderRef> — None for Lean)
    stub.rs             # DeterministicStubProvider for testing
  project_store/
    mod.rs
    models.rs           # Project, ContractDocument
    file_store.rs       # save_document / load_document (JSON sidecar metadata)
```

---

## Architecture Invariants

1. **Layer ordering** (no upward imports):
   - `contract_ir` → no tauto deps
   - `contract_parser`, `lean_gen`, `preprocessing` → only `contract_ir`
   - `slm` → only `contract_ir` (not `preprocessing`)
   - `project_store` → only `contract_ir`

2. **`BTreeMap` for deterministic context** — never `HashMap` where key order is hashed.

3. **`source` excluded from semantic hash** — cache keys survive reformatting.

4. **`provider: Option<_>` in `ArtifactTraceability`** — Lean generator attaches traceability without SLM.

5. **Safety scan word-bounded** — `line_contains_token` checks alphanumeric/underscore boundaries; `unsafeMethod` does not trigger `unsafe`.

6. **Suffix namespace disjoint from base namespace** — `lean_ident` strips underscores; disambiguation suffixes `_1`/`_2` are unreachable from any case string. Proven by `suffix_namespace_is_disjoint_from_base_namespace` test.

---

## Test Count: 94

| Module | Tests |
|--------|-------|
| `contract_ir::models` | 10 |
| `contract_ir::serialization` | 6 |
| `preprocessing::context_builder` | 7 |
| `slm::provider` | 2 |
| `slm::traceability` | 4 |
| `slm::stub` | 5 |
| `lean_gen::workspace` | 14 |
| `lean_gen::safety` | 11 |
| `lean_gen::io` | 4 |
| `contract_parser::markdown` | 8 |
| `contract_parser::dsl` | 13 |
| `project_store::models` | 4 |
| `project_store::file_store` | 5 |

---

## Phase R2 — Next Steps

1. **Real SLM integration** — wire an actual LLM API (e.g. Deepseek, Ollama) behind `SlmCodeGenerator`. The trait is in place; add a concrete HTTP adapter in `slm/` with `reqwest` (add to deps).

2. **Lean proof attempt pipeline** — after generating sorry stubs, submit them to the SLM for proof attempts. Replace `sorry` with an actual proof term when the SLM succeeds. Re-scan to verify.

3. **CI artifact** — `cargo build --release` + GitHub Actions workflow producing a static binary.

4. **Integration tests** — spawn the binary via `assert_cmd` crate and assert stdout/exit codes against fixture markdown files. Currently all tests are unit tests inside modules.

5. **`--format json` for `verify` and `hash`** — machine-readable output for tooling integration.

6. **`project store` integration in CLI** — persist parsed contracts under a project slug for incremental re-verification.

---

## Known Deferred Items

- Clippy is not installed in the current toolchain (`.rustup` has no component for it). Add it with `rustup component add clippy` when the full toolchain is available.
- `Expression.kind` in Lean renderer is currently unused — conditions render via `ExpressionValue::Display`, not the kind discriminant. A typed Lean term renderer (Phase R2) would use kind to choose `Nat`, `Bool`, etc.
- `project_store` is not integrated into the CLI — saving/loading projects is implemented but not wired to any subcommand.

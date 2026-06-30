# Tauto — Handoff Document

_Updated after Phase R2 testable slice_

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
**Tests:** 135 passing (116 unit, 19 integration)
**Binary:** `cargo build` → `./target/debug/tauto`

### CLI Commands

```
tauto verify <path> [--output <dir>] [--strict] [--format text|json]
    Parse markdown contracts, generate Lean 4 workspace, scan for sorry stubs.
    --strict exits 1 if any sorry stubs found (for CI gate).
    --format json emits a JSON summary (contracts, files, conflicts, sorry_count, workspace).

tauto hash <path> [--format text|json]
    Print semantic hash (excludes source locations) and provenance hash.
    Semantic hash is stable across reformatting — use as cache key.
    --format json emits { contracts, files, semantic, provenance }.

tauto list <path>
    List parsed contracts (entity/operation/case + source location).

tauto diff <base> <new> [--strict]
    Structural diff between two contract sets; heuristic conflict detection on changed contracts.
    --strict exits 1 if diff is not expansion-only.

tauto store <path> --project <slug> [--store-root <dir>] [--format text|json]
    Store contract markdown files under a project slug for incremental re-verification.
    Slug normalized to lowercase-hyphen. Writes a JSON sidecar alongside each markdown file.
    --format json emits { project, stored: [...paths] }.
    Default store-root: tauto-store/
```

### Integration tests

```
tests/
  cli_integration.rs       # 15 tests — assert_cmd + predicates
  fixtures/
    orders.md              # 2 clean contracts
    conflicts.md           # 2 contracts with conflicting ensures (conflict detection smoke test)
    base/orders.md         # 1 contract (diff baseline)
    expanded/orders.md     # 2 contracts (adds ShipApprovedOrder — expansion-only diff)
```

---

## Project Structure

```
Cargo.toml              # single crate, lib + bin (assert_cmd/predicates in dev-deps)
tests/
  cli_integration.rs    # 15 integration tests (assert_cmd)
  fixtures/             # canonical fixture markdown files
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

## Test Count: 131

| Module | Tests |
|--------|-------|
| `contract_ir::models` | 10 |
| `contract_ir::serialization` | 6 |
| `contract_ir::diff` | 10 |
| `contract_ir::conflicts` | 10 |
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
| **integration (cli_integration)** | **19** |

---

## Phase R2 — Completed

- **Integration tests** — 15 tests via `assert_cmd`+`predicates`, fixture markdown files in `tests/fixtures/`.
- **`--format json`** — added to `verify` and `hash` subcommands. JSON schema documented above.

## Phase R3 — Completed

- **`tauto store` subcommand** — `run_store` wires `project_store::{save_document, ContractDocument}` to the CLI.
  Stores markdown files under `<store-root>/<project-slug>/<filename>` with JSON sidecar metadata.
  `--format json` emits `{ project, stored: [...paths] }`.
  4 new integration tests: document file creation, JSON output, sidecar metadata, slug normalization.

## Phase R4 — Next Steps

1. **`--format json` for `list` and `diff`** — extend JSON output to remaining subcommands (small, same pattern as verify/hash).

2. **CI artifact** — `cargo build --release` + GitHub Actions workflow. Fully testable locally with `cargo build --release`.

3. **Real SLM integration** — wire an actual LLM API behind `SlmCodeGenerator`.
   `DEEPSEEK_API_KEY`, `GROQ_API_KEY`, `NVIDIA_API_KEY` are available in the environment.
   Add `reqwest` (with `json`, `blocking` features) to `Cargo.toml`.
   Implement `src/slm/http_provider.rs` (DeepSeek adapter: one POST to the chat completions endpoint).
   Wire into `tauto verify --model <name>`.
   **Constraint**: Lean/Lake not installed — proof compilation cannot be validated. SLM generates candidates only.

4. **Lean proof attempt pipeline** — after generating sorry stubs, submit to SLM for proof terms.
   Blocked until Lean/Lake installed.

---

## Known Deferred Items

- Clippy is not installed in the current toolchain (`.rustup` has no component for it). Add it with `rustup component add clippy` when the full toolchain is available.
- `Expression.kind` in Lean renderer is currently unused — conditions render via `ExpressionValue::Display`, not the kind discriminant. A typed Lean term renderer (Phase R2) would use kind to choose `Nat`, `Bool`, etc.
- `project_store` is not integrated into the CLI — saving/loading projects is implemented but not wired to any subcommand.

# Tauto — Handoff Document

_Updated after Rust rewrite decision (Phase R0)_

---

## Decision Log

### Why Rust?

Tauto is a **CLI binary** (`tauto verify contracts/`). Rust was chosen over Python because:

- Algebraic data types and pattern matching are a better fit for IR modeling than Pydantic
- Ships as a single binary with no runtime dependency
- Immutability is guaranteed by the type system, not enforced by convention
- `BTreeMap` replaces `sort_keys=True` JSON hacks — key ordering is structural
- The Python implementation (Phases 0-2) served as a design prototype; it is in git history on this branch

### Python Phase Archive

The Python prototype (3 reviewed phases, 44 tests) is preserved in git history at commit `fb9d8a9` (docs: update handoff for phase 3 completion). The design decisions it produced are ported here.

---

## Project Structure (Rust)

```
Cargo.toml              # single crate, lib + bin
src/
  lib.rs                # pub mod declarations
  main.rs               # CLI (clap)
  contract_ir/
    mod.rs
    models.rs           # ContractIR, ContractSet, Condition, Expression, …
    serialization.rs    # semantic_contract_set_hash, contract_set_hash
  contract_parser/      # stub (Python Phase 2 port pending)
    mod.rs
  lean_gen/
    mod.rs
    workspace.rs        # generate_lean_workspace, LeanWorkspace, LeanWorkspaceFile
    safety.rs           # scan_lean_workspace → Vec<Diagnostic>
  preprocessing/
    mod.rs
    context_builder.rs  # build_deterministic_context → DeterministicContext
  slm/
    mod.rs
    provider.rs         # SlmCodeGenerator trait, ArtifactKind, SlmProviderRef, …
    traceability.rs     # build_traceability → ArtifactTraceability
  project_store/        # stub (Python Phase 2 port pending)
    mod.rs
```

---

## Test Count: 49 passing

| Module | Tests |
|--------|-------|
| `contract_ir::models` | 8 |
| `contract_ir::serialization` | 6 |
| `preprocessing::context_builder` | 7 |
| `slm::provider` | 2 |
| `slm::traceability` | 4 |
| `lean_gen::workspace` | 11 |
| `lean_gen::safety` | 11 |

---

## Architecture Invariants

These constraints must hold across all future work:

1. **Layer ordering** (no upward imports):
   - `contract_ir` → no tauto deps
   - `lean_gen` → only imports `contract_ir`
   - `preprocessing` → only imports `contract_ir`
   - `slm` → only imports `contract_ir` (not `preprocessing`)
   - `contract_parser` → only imports `contract_ir`
   - `project_store` → imports `contract_ir`

2. **`BTreeMap` for all deterministic context** — never `HashMap` where ordering is hashed.

3. **`source` excluded from semantic hash** — `semantic_contract_set_hash` omits `SourceLocation`; formatting-change cache invalidation is prevented by design.

4. **`provider: Option<SlmProviderRef>`** in `ArtifactTraceability` — deterministic generators (Lean) must be able to attach traceability without an SLM provider.

5. **Token-bounded safety scan** — `line_contains_token` checks word boundaries; scanning inside identifier names is a false positive.

---

## Next Steps (Phase R1)

1. **Port contract parser** — Markdown block extraction + DSL parser from Python Phase 2 to Rust. No external parser crate needed.

2. **Port project store** — disk persistence (`ContractDocument`, `Project`) + file store. Add `tempfile` dev-dep for tests.

3. **Lean workspace IO** — `write_lean_workspace(ws: &LeanWorkspace, base_path: &Path) -> Result<(), IoError>`. Separate from pure generator. Assert path uniqueness before writing (guards against collision hole in `assign_module_names`).

4. **Provider stub for SLM round-trip** — non-empty deterministic stub that returns a `GeneratedArtifact` so `build_traceability` gets its first real caller in tests.

5. **CLI `verify` command** — wire up `tauto verify <path>` → parse markdown → generate Lean → scan → report diagnostics.

---

## Known Issues / Deferred

- **Collision hole in `assign_module_names`**: cases `["AB", "AB", "AB_1"]` → second AB → `AB_1` → collision with existing `AB_1`. The path-uniqueness assert in the IO writer (Phase R1 step 3) catches this at disk write time. Fix the root cause in `assign_module_names` in Phase R1.
- **`contract_parser` and `project_store` are stubs** — they compile but have no implementation.
- **`Expression.value` uses `serde_json::Value`** — untyped. Consider a typed `ExpressionValue` enum in Phase R1.
- **No clippy CI** — add `cargo clippy -- -D warnings` to the development workflow.

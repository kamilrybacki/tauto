# Tauto Phase 0-2 Design

## Goal

Build the first implementation slice of Tauto: a testable, functional-core foundation for managing business-rule contracts as Markdown, parsing structured contract blocks, and emitting deterministic, language-independent Contract IR.

## Scope

This design covers Phase 0 through Phase 2 only:

- Phase 0: repository foundation and Python package skeleton.
- Phase 1: project/document storage primitives as pure local models and file-backed helpers.
- Phase 2: Markdown contract extraction, minimal DSL parsing, deterministic Contract IR JSON, and hashing.

Lean generation, Lean execution, validators, generated tests, API persistence, worker orchestration, and UI are intentionally deferred. The Phase 2 output must be suitable for those later phases.

## Name

The platform name is `Tauto`.

Python import roots use `tauto_*` package names to keep boundaries explicit:

- `tauto_contract_ir`
- `tauto_contract_parser`
- `tauto_project_store`

## Architecture

Tauto uses a language-independent Contract IR as its source of truth. Markdown contract blocks parse into a typed AST/IR. Lean is a future verification backend generated from that IR, not the core representation. Other backends, such as Python or TypeScript validators, will consume the same IR.

The core is functional:

- parsing functions accept strings and return values plus diagnostics;
- serialization functions accept typed IR values and return deterministic strings or hashes;
- validation functions accept IR and vocabulary values and return diagnostics;
- filesystem interaction is isolated in small shell modules.

This keeps behavior testable without a database, API server, worker, or Lean installation.

## Data Flow

```text
Markdown document
  -> extract_contract_blocks(markdown, path)
  -> parse_contract_block(raw_block)
  -> ContractIR
  -> ContractSet
  -> canonical_contract_json / canonical_contract_set_json
  -> stable SHA-256 hashes
```

The `ContractSet` type exists from the beginning so later phases can validate an incoming business case against the full accepted base of contracts and invariants.

## Contract DSL MVP

The first DSL supports:

- `case`
- `entity`
- `operation`
- `requires`
- `ensures`
- `forbidden`
- `preserves`
- `assumes`

Supported comparison operators:

- `==`
- `!=`
- `>=`
- `<=`
- `>`
- `<`

Supported expression kinds:

- field references such as `order.status`
- variables such as `result`
- enum literals such as `Paid`
- integer literals such as `100`
- boolean literals `true` and `false`

Supported forbidden calls:

- operation calls such as `shipOrder(result)`

The MVP deliberately excludes arithmetic, quantifiers, temporal logic, nested calls, and arbitrary Lean syntax.

## Error Handling

All parser and extraction failures return structured diagnostics. User-facing diagnostics include:

- category;
- message;
- source path;
- line number when available;
- optional suggestion.

The parser should not raise exceptions for normal user errors such as unknown sections or malformed conditions.

## Testing Strategy

Development must follow TDD:

1. Write a focused failing test.
2. Run it and confirm the expected failure.
3. Implement the smallest code to pass.
4. Run the test again.
5. Refactor only while tests stay green.

Required test areas:

- empty Markdown produces no blocks;
- fenced `contract` blocks preserve source line numbers;
- valid `CancelPaidOrder` DSL parses to typed IR;
- malformed conditions return diagnostics;
- canonical JSON is stable across formatting noise;
- hashes are stable and change when semantic content changes;
- zero-contract `ContractSet` serializes to `{"schema_version":1,"contracts":[]}`.

## Non-Goals

This slice does not implement:

- API server;
- UI;
- PostgreSQL persistence;
- Lean workspace generation;
- Lean safety scanning;
- runtime validator generation;
- implementation conformance tests;
- GitHub/GitLab integration.

## Acceptance Criteria

- The repository has a working Python project skeleton.
- `pytest` runs locally.
- Contract IR models are typed and language-independent.
- Markdown contract blocks can be extracted with line numbers.
- The MVP DSL can parse `CancelPaidOrder`.
- A zero-contract project is valid.
- Contract IR and ContractSet JSON are deterministic and hashable.
- All implemented behavior is covered by focused tests written first.

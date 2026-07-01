# tauto

A Lean-backed toolchain for authoring, validating, and testing business-rule
**contracts**. Rules are written in a small declarative language embedded in
Markdown; tauto flags conflicts between them, generates a conformance test
suite, and emits a Lean 4 proof workspace.

**📄 Documentation: <https://kamilrybacki.github.io/tauto/>**

## Quickstart

```bash
# put contracts in ./rules/*.md, then serve the workspace + web UI:
tauto serve ./rules --port 4000
# open http://localhost:4000
```

## What a contract looks like

````markdown
```contract
case ApprovePrimeMortgage
entity:
  Mortgage
operation:
  approveApplication
requires:
  loan.credit_score >= 750
  loan.employment_verified == true
  loan.status == UnderReview
ensures:
  result.status == Approved
forbidden:
  disburseFunds(loan.id)
preserves:
  loan.applicant_id
```
````

## Checking a proposed rule (for agents)

`POST /api/v1/check` validates a proposed rule against the current set **without
persisting anything** and returns a compatibility verdict plus a generated test
suite:

```bash
curl -X POST http://localhost:4000/api/v1/check \
  -H 'Content-Type: text/plain' \
  --data-binary @proposed-rule.md
```

## Commands

| Command | Purpose |
|---------|---------|
| `tauto verify <path>` | Parse contracts, generate the Lean workspace (`--lean-check` runs `lake build`) |
| `tauto list <path>` | List parsed contracts |
| `tauto hash <path>` | Semantic + provenance hashes (CI cache keys) |
| `tauto diff <base> <new>` | Structural diff + conflict candidates |
| `tauto store <path> --project <slug>` | Store contracts under a project slug |
| `tauto retrieve --project <slug>` | Retrieve stored contracts |
| `tauto serve <path>` | Start the web UI and HTTP API |

All commands accept `--format json`.

## Building from source

```bash
cargo build --release                 # the tauto binary
(cd ui && npm ci && npm run build)    # the web UI (served by `tauto serve`)
```

## Limitations

Conflict detection is a **heuristic** that surfaces candidates for review, not a
decision procedure. Generated Lean theorems are `sorry`-stubbed: a passing build
certifies well-formedness, not the truth of the rules. See the
[documentation](https://kamilrybacki.github.io/tauto/) for detail.

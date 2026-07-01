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

## Domain glossary

Declare your domain vocabulary in ` ```glossary ` blocks (e.g. a `_glossary.md`
in the contracts dir): each entity's canonical name, its instance-prefix aliases
(`loan` for `Mortgage`, so `loan.credit_score` resolves), fields, enum members,
operations, and — under a `states:` section — the enum-valued **state**
(determinant) fields that drive the entity's lifecycle. `POST /api/v1/check`
then returns advisory `glossary_warnings` for a proposed rule — unknown fields,
undeclared enum values, a `package.*` reference inside a `Mortgage` rule (the
Order-vs-Package distinction), or a `missing_state_guard` when a rule on a
stateful entity doesn't name its source state. Warnings never block; an empty
glossary is inert. `GET /api/v1/glossary` returns it, and the MCP `get_glossary`
tool exposes it to authoring agents.

Declaring state fields also sharpens conflict detection: two rules on the same
operation guarding **disjoint** states (`status == Paid` → Shipped vs
`status == Unpaid` → Rejected) are distinct lifecycle transitions, not a
conflict — tauto suppresses that false positive.

Because each state field carries its full domain, the rules read as a **state
machine**. `GET /api/v1/lifecycle` (and the MCP `state_coverage` tool) reports
each entity's transitions and coverage gaps — states with no incoming/outgoing
transition, or **isolated** states no rule touches at all (a declared state you
haven't written a rule for yet).

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

## Local testing

`scripts/dev.sh` runs the whole stack locally — no cluster needed. It serves the
example rules in `examples/rules/` and exercises the HTTP API and the MCP
`check_rule` flow.

```bash
scripts/dev.sh build            # build binary + UI
scripts/dev.sh serve            # serve examples/rules — UI at http://localhost:4000
# in another terminal:
scripts/dev.sh check examples/proposed/compatible-refinance.md   # dry-run a proposed rule
scripts/dev.sh mcp-call list_contracts                           # one-shot MCP tool call
scripts/dev.sh demo             # compatible + conflicting + malformed proposals end-to-end
```

`examples/proposed/` holds sample rules to check (a compatible one and one that
conflicts with the seeded set). `TAUTO_SKIP_LEAN_CHECK=1` is set automatically so
no Lean toolchain is required.

### The MCP server and the rule-authoring skill

`tauto mcp --api-url <serve URL>` is a stdio JSON-RPC MCP server exposing the
contracts to LLMs (`list_contracts`, `search_contracts`, `find_conflicts`,
`graph_neighbors`, `verify_contract`, and `check_rule` for dry-run validation of
a proposed rule). The `.claude/skills/tauto-rules/` skill drives the full loop:
converse about a new rule, translate it to the DSL, and check it via `check_rule`.

## Limitations

Conflict detection is a **heuristic** that surfaces candidates for review, not a
decision procedure. Generated Lean theorems are `sorry`-stubbed: a passing build
certifies well-formedness, not the truth of the rules. See the
[documentation](https://kamilrybacki.github.io/tauto/) for detail.

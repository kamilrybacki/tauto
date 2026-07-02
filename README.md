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

## Prose → DSL (SLM front door)

The hard part — turning natural-language rules into formal ones — is where an SLM
helps. `POST /api/v1/translate` (MCP `translate_rule`) takes prose and returns
**DSL** (not Lean): a legible contract you review for faithfulness, then feed to
the verified pipeline (`/check` → proofs). The SLM never emits proofs and nothing
is persisted — "it compiles" is not "it's faithful", so the DSL review is the
checkpoint, and the deterministic DSL→Lean→lake path runs downstream.

The SLM is a **required** capability: the real provider (DeepSeek) is the
default and `/translate` returns **503** if it is not configured — it never
silently degrades to the stub. `DEEPSEEK_API_KEY` is read from a secret (never
committed). The deterministic stub is available only for offline/testing via an
explicit `TAUTO_SLM_PROVIDER=stub`. For rules the DSL can't express, extend the
DSL/IR rather than emit unverifiable Lean.

The endpoint and model are configurable, so tauto can point at **any
OpenAI-compatible server** (a hosted API, or a local Ollama/vLLM/llama.cpp):
`SLM_BASE_URL` is a **base** URL — tauto appends `/v1/chat/completions` (e.g.
`SLM_BASE_URL=http://ollama:11434`) — and `DEEPSEEK_MODEL` picks the model
(default `deepseek-chat`, the cheapest). Note this differs from `TAUTO_LAKE_URL`
(the Lean build service), which is the **full** build URL, not a base.

## Checking a proposed rule (for agents)

`POST /api/v1/check` validates a proposed rule against the current set **without
persisting anything** and returns a compatibility verdict plus a generated test
suite.

**Correctness vs intent (conformance).** A rule can carry an `intent:` line (what
it's meant to do) and `examples:` — concrete cases the author expects. tauto
evaluates the rule against its own examples (reusing the conditions as decidable
predicates) and reports `conformant` + per-example outcomes. A `fail` means the
**formalization contradicts the stated intent** — e.g. the rule ensures
`Shipped` but an example says a paid order becomes `Cancelled`. This is
correctness *relative to the stated cases*, no operation model needed, and the
examples double as grounded test cases:

````markdown
```contract
case ShipPaidOrder
entity:
  Order
operation:
  ship
requires:
  order.status == Paid
ensures:
  result.status == Shipped
intent:
  A paid order should ship.
examples:
  - given: status=Paid; then: status=Shipped
  - given: status=Unpaid; applies: false
```
````

So `/check` gives the full triad: **compatibility** (rules vs rules) ·
**correctness** (rules vs intent-examples) · **tests** (the suite). Example:

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

When real data exists, `GET /api/v1/reconcile` (MCP `reconcile_states`) completes
the declared state domains against observed ones. A **state source** yields the
observed states, and tauto reports per field the `observed_not_declared`
(suggested completions) and `declared_not_observed` (unseen). Source precedence:

1. **live database** (when configured) — see below;
2. **ODCS data contracts** — any `*.odcs.yaml` (Open Data Contract Standard) in
   the contracts dir; allowed values are read from each property's
   `invalidValues` quality rule (`arguments.validValues`) and mapped to a
   glossary entity/state field (see `examples/odcs/`);
3. **native descriptor** — a `_observed_states.json` mapping `Entity.field: [values]`;
4. **none**.

Advisory and additive: it proposes completions, never rewrites the glossary —
logic set up before data stays valid.

The live-database source is behind the `database` Cargo feature (off by default,
keeping the core build dependency-free):

```bash
cargo build --release --features database
DATABASE_URL=postgres://user:pass@host/db tauto serve ./rules
```

It reads the distinct values of each state field's column — mapping entity
`Mortgage` → table `mortgage` and a state field `status` → column `status` — via
`SELECT DISTINCT`. Identifiers are validated and quoted; a missing table/column
is skipped, and if no database is reachable it falls back to the file descriptor.

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

## Lean build service (Proofs compilation)

The Proofs panel's `lake build` runs in a **separate, pluggable build service**,
not the web pod — so the ~800 MB Lean toolchain never bloats or slows the web
image. `tauto serve` POSTs the generated workspace to `TAUTO_LAKE_URL`; if unset
or unreachable, Proofs degrades gracefully (`build_available: false`) and still
shows the sorry-stubbed obligations.

The backend is any service implementing a minimal HTTP contract, so you can run
the bundled reference worker or point at your **own Lake deployment**:

```
POST <TAUTO_LAKE_URL>   { "files": [ { "path": "...", "content": "..." }, ... ] }
                     -> { "success": bool, "stdout": "...", "stderr": "..." }
GET  /health            -> 200
```

Reference implementation: `tauto lake-worker --port 4001` (shipped as the
`tauto-lake` image, which bundles Lean; runs `lake build` against whatever
toolchain is on PATH). In the Helm chart it's a gated `lakeWorker` Deployment +
Service; set `TAUTO_LAKE_URL=http://tauto-lake:4001/build`.

## Limitations

Conflict detection is a **heuristic** that surfaces candidates for review, not a
decision procedure. Generated Lean theorems are `sorry`-stubbed: a passing build
certifies well-formedness, not the truth of the rules. See the
[documentation](https://kamilrybacki.github.io/tauto/) for detail.

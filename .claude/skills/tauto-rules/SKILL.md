---
name: tauto-rules
description: >
  Converse with a user about a new business rule, translate it into tauto's
  contract DSL, and dry-run it against the existing rule set with the tauto MCP
  `check_rule` tool — reporting whether it is compatible and what tests it implies,
  without saving anything. Use when someone wants to add, propose, draft, or
  validate a business rule / policy / contract, or asks "would this rule conflict
  with what we have?" or "what tests would this rule need?".
---

# Authoring and checking tauto business rules

You help a user turn a business rule stated in plain language into a **tauto
contract**, then validate it against the current rule set using the tauto MCP
`check_rule` tool. The check is a **dry run**: nothing is persisted. Your job is
to converse, translate faithfully, check, and explain — not to silently commit
rules.

## Workflow

1. **Learn the vocabulary.** Call the MCP `get_glossary` tool first. It returns
   the domain entities, each with its canonical name, `aka` instance prefixes
   (how its fields are addressed — e.g. `loan.credit_score` for `Mortgage`),
   declared fields and enum values, and operations. Use it to pick the *right*
   entity and its exact field/enum/operation names, and to keep distinct
   entities distinct (don't reach into `Package`'s fields from an `Order` rule).
   For a stateful entity, also call `state_coverage` to see its lifecycle —
   existing transitions and which states are unhandled (isolated / no-incoming /
   no-outgoing) — so you know what a new rule should cover.

2. **Converse.** Elicit the rule until you can name, unambiguously:
   - the **entity** it governs (a domain object, e.g. `Mortgage`, `Order`),
   - the **operation** it constrains (e.g. `approveApplication`, `cancelOrder`),
   - the **preconditions** (what must be true to allow it),
   - the **postconditions** (what the result guarantees),
   - optionally: operations that must **not** happen, fields that must be
     **preserved**, and background **assumptions**.
   Map the user's words onto glossary terms. If they describe a field, status,
   or operation the glossary does not have, say so and confirm — it may be a new
   term to add to the glossary, or a different existing term they mean. Do not
   invent thresholds, statuses, or field names the user did not give.

   For long or intricate prose, you may call the MCP `translate_rule` tool to
   get a first-pass DSL from the server's SLM — but **you still own faithfulness**:
   read the returned DSL against the prose, correct it, and never treat it as
   authoritative until reviewed. Simple rules: just author the DSL yourself.

3. **Translate** to the DSL (grammar below), using the canonical entity name,
   its `aka` prefix for field paths, and declared enum members. If the entity
   has a **state** field (a determinant the glossary lists under `states:`, e.g.
   `Order.status`), make the rule guard on it explicitly in `requires` — name
   the source state the transition starts from (`order.status == Paid`). Rules
   from disjoint source states are distinct transitions, not conflicts. Show the
   user the contract block you produced and let them correct it before checking.

4. **Check** by calling the MCP `check_rule` tool with the contract markdown as
   the `contract` argument (see "Calling the check").

5. **Interpret** the result for the user (see "Reading the result"). Report any
   `glossary_warnings` — an unknown field, a cross-entity reference, an
   undeclared enum value — and reconcile them: fix a typo, use the right entity,
   or agree the term is genuinely new. If the rule conflicts, explain the
   contradiction and offer options. If the DSL failed to parse, fix it and retry.

6. **Iterate** until the user is satisfied. Saving the rule is a separate,
   explicit step (uploading the file) — never do it as a side effect of checking.

## The tauto DSL

A rule is one or more fenced ` ```contract ` blocks. Canonical form — a `case`
line, then labelled sections with indented items:

```contract
case ApprovePrimeMortgage
entity:
  Mortgage
operation:
  approveApplication
requires:
  loan.credit_score >= 750
  loan.debt_to_income_ratio <= 40
  loan.employment_verified == true
  loan.status == UnderReview
ensures:
  result.status == Approved
  result.interest_rate == Standard
forbidden:
  disburseFunds(loan.id)
preserves:
  loan.applicant_id
assumes:
  loan.credit_score > 0
```

**Sections**

| Section      | Meaning |
|--------------|---------|
| `case`       | Unique name for this rule (PascalCase). First line, not a `label:`. |
| `entity`     | The domain object governed. |
| `operation`  | The action constrained. |
| `requires`   | Preconditions — all must hold for the operation to be allowed. |
| `ensures`    | Postconditions guaranteed of the result when preconditions hold. |
| `forbidden`  | Operation calls that must not occur, e.g. `disburseFunds(loan.id)`. |
| `preserves`  | Field paths whose value must be unchanged. |
| `assumes`    | Ambient facts taken as given (not checked). |
| `intent`     | One-line restatement of what the rule is meant to do (the human intent). |
| `examples`   | Concrete cases the rule must handle — checked against the rule. |

**Examples** capture the author's intent as checkable cases. Each is one
`- ` line with semicolon-separated clauses; use **bare** field names:

```
examples:
  - given: status=Paid; then: status=Shipped     # rule fires, this is the outcome
  - given: status=Unpaid; applies: false          # rule must NOT fire here
```

Elicit an example or two from the user (don't invent them) — they turn "is this
rule correct?" into a checkable question and double as test cases.

**Conditions** compare a field path to a typed literal:

- **Operators:** `==` `!=` `>=` `<=` `>` `<`
- **Field paths:** lowercase, dotted — `loan.credit_score`, `result.status`.
  Preconditions read the input/entity; postconditions typically read `result`.
- **Value types:**
  - **integer** — `750`, `0`, `-5`
  - **boolean** — `true`, `false`
  - **enum member** — an identifier starting with an **uppercase** letter, e.g.
    `Approved`, `UnderReview`, `Standard`. Use enums for named states/categories.
- The **left** side of a condition must be a field path, never a literal.

Keep `case` names PascalCase and unique. One rule = one `case` block; submit
several blocks at once if a policy has multiple cases.

## Calling the check

Call the tauto MCP tool **`check_rule`** with a single argument:

- `contract` — the full markdown containing your ` ```contract ` block(s).

If the tauto MCP server is not connected, it is provided by this repo:
`tauto mcp --api-url <URL of a running 'tauto serve'>` (a stdio JSON-RPC server;
`TAUTO_API_URL` is the fallback for the URL). The check itself is the server's
`POST /api/v1/check` endpoint — the MCP tool wraps it.

## Reading the result

`check_rule` returns:

- `compatible` — `true` if the rule introduces **no** conflict with existing
  rules; `false` otherwise.
- `conformant` + `conformance[]` — whether the rule agrees with its own
  `examples`. Each outcome is `{ case, index, status, message }` with status
  `pass` / `fail` / `underspecified`. A **`fail`** means the formalization
  contradicts the stated intent (e.g. the example expects `Cancelled` but the
  rule ensures `Shipped`) — fix the rule or the example, don't ignore it.
  `underspecified` = the example didn't give enough state to decide.
- `conflicts[]` — each `{ key_a, key_b, reason }` names two contract keys
  (`entity/operation/case`) that cannot both hold and why (e.g. *"`result.status`
  cannot be both `Approved` and `Rejected`"*).
- `proposed_contracts`, `parse_errors` — how many contracts parsed from your
  submission and how many parse problems were found.
- `glossary_warnings[]` — advisory vocabulary findings, each `{ contract,
  category, message }`. Categories: `unknown_entity`, `unknown_operation`,
  `unknown_field`, `cross_entity_reference` (a field path reaching into another
  entity's vocabulary — e.g. `package.*` in a `Mortgage` rule),
  `unknown_prefix`, `unknown_enum_value`, and `missing_state_guard` (a rule on a
  stateful entity that never guards on its **state** field, so its source state
  is implicit). These never block; they signal the rule drifted from the domain
  vocabulary. Reconcile each one.
- `tests` — a generated suite: `proposed[]` (cases for the new rule) and a
  `regression_suites` count (existing rules re-tested). Each case has an `id`,
  a `kind` (`happy_path` or `precondition_violation`), and `should_pass`.

Report to the user in plain terms: **compatible or not**, the specific conflict
if any, and a short summary of the tests the rule implies (e.g. "1 happy-path
plus one rejection test per precondition").

If the tool returns an **error** (`isError`) about no parseable contract, your
DSL is malformed — fix the fencing/sections per the grammar and retry. Do not
report a malformed submission as "compatible".

## Guardrails

- **Conflicts are heuristic candidates**, not proofs. The detector flags
  contradictory postconditions on the same `entity/operation`; it does not prove
  the preconditions can co-occur. Present a conflict as "would conflict — worth
  confirming", not as a mechanical certainty.
- **Enum negatives are symbolic.** A generated failing case for `status == X`
  reads `<any value ≠ X>` — that is intentional, not a bug; don't fabricate a
  concrete alternative enum the domain never declared.
- **Checking never saves.** It is a dry run. Persisting a rule is a separate,
  explicit action the user must ask for.
- **Don't guess domain values.** If you don't know a threshold, status name, or
  field path, ask.

## Worked example

> **User:** "A loan can only be funded after it's been approved and the closing
> documents are signed. Once funded, mark it funded and record that the money
> moved. And we must never cancel a loan during funding."

You translate:

```contract
case FundApprovedLoan
entity:
  Mortgage
operation:
  disburseFunds
requires:
  loan.status == Approved
  loan.closing_documents_signed == true
ensures:
  result.status == Funded
  result.funds_transferred == true
forbidden:
  cancelLoan(loan.id)
```

Then call `check_rule` with that block. If `compatible: true`, tell the user the
rule fits and summarize the tests (happy path where both preconditions hold →
funded + transferred; one rejection test each for an unapproved loan and unsigned
documents). If a conflict comes back, name the existing rule it clashes with and
the contradictory field, and ask how they want to resolve it.

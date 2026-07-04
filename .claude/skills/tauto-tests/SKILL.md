---
name: tauto-tests
description: >
  Turn tauto's generated test specifications (JSON from the MCP
  `get_verification_report` or `check_rule` tools) into real, executable tests
  in the current project's language and test framework — arrange from `given`,
  act via the project's actual operation code, assert the `ensures`. Use when
  someone asks to "implement the tauto tests", "generate tests from the rules",
  "make the spec suite executable", or wants conformance tests for a rule set
  backed by tauto.
---

# Implementing executable tests from tauto specs

tauto generates **test specifications** — structured JSON, deliberately not
code. Your job is the translation it cannot do: bind each spec to the
project's real functions and emit idiomatic tests in the project's own
framework. The spec is the contract; the binding is your judgment; the test is
the output.

## Workflow

1. **Fetch the specs.** Call the tauto MCP tool `get_verification_report`
   (whole rule set; each rule carries `tests[]`) or `check_rule` (one proposed
   rule). No MCP? `GET <tauto>/api/v1/report?project=<slug>` gives the same
   JSON. Each case looks like:

   ```json
   { "id": "approve_requested_player__violation_1",
     "kind": "precondition_violation",
     "description": "...",
     "given": [ { "field": "status", "value": "Requested" },
                { "field": "seatsAvailable", "value": false } ],
     "expect_ensures": [ { "field": "status", "value": "Approved" } ],
     "expect_forbidden_not_called": [ "cancel(order.id)" ],
     "expect_preserved": [ "order.customer_id" ],
     "violated_precondition": "request.seatsAvailable == true",
     "should_pass": false }
   ```

2. **Detect the stack and its conventions.** Look for the project's existing
   test setup (`package.json` → vitest/jest/playwright, `Cargo.toml` → `#[test]`,
   `pyproject.toml` → pytest, etc.) and MATCH IT — file layout, naming,
   fixtures, harness helpers. If the project has an integration/scenario
   harness (seeded DB, actors), prefer it over unit mocks: these are
   behavioral rules, and DB triggers/RLS are often part of the behavior.

3. **Build the operation binding — the hard part.** Map each rule's
   `entity · operation` to the project's real entry point (server action, API
   route, service method). Start from the rule's `source` citation and the
   glossary. Write the binding down as a small table in the test file header.
   **If a binding is ambiguous, ask the user — never guess.**

4. **Translate each case, 1 spec = 1 test:**
   - `given` → arrange: construct the entity state (fixtures/factories/seeds).
     Derived flags (e.g. `seatsAvailable=false`) must be realized through real
     state (fill the seats), not by mocking the flag away.
   - act → call the bound operation.
   - `should_pass: true` → assert every `expect_ensures` field on the result,
     assert `expect_forbidden_not_called` operations were not invoked (spies
     or state checks), assert `expect_preserved` fields unchanged.
   - `should_pass: false` → assert the operation is **rejected** (error return,
     exception, 4xx — whatever the project's failure convention is) and that
     no `ensures` effect occurred. Name the violated precondition in the test
     name or a comment.
   - Symbolic enum negatives (`<any value ≠ X>`) → pick a concrete member from
     the glossary's declared domain; never invent enum values.

5. **Traceability.** Each test carries the spec id and rule key
   (e.g. `// tauto: Participation/approve/ApproveRequestedPlayer · approve__violation_1`)
   so a failing test points straight back to the rule, and rule changes are
   greppable to their tests.

6. **Run the suite.** Then report honestly:
   - All green → done; list the binding table and coverage (N specs → N tests).
   - A test fails → **that is a finding, not a test bug**: the implementation
     disagrees with the rule set. Report which rule and which precondition;
     do NOT weaken the assertion or patch app code to force green without the
     user deciding which side (code or rule) is wrong.

## Guardrails

- The spec is data extracted from the rules — do not add assertions the spec
  doesn't imply, and do not skip cases because they seem redundant.
- Don't fabricate setup the domain forbids (e.g. constructing a state the
  lifecycle says is unreachable); if arranging `given` is impossible through
  legitimate paths, that's a finding about the rule or the code — surface it.
- Idempotence: re-running the skill should update the generated file(s) in
  place (keyed by spec id), not duplicate tests.
- Keep generated tests in a clearly-marked file (e.g.
  `*.tauto.test.ts` / `tauto_conformance_test.rs`) so hand-written tests are
  never mixed in or clobbered.

## Worked example (MeepMap, vitest + scenario harness)

Spec: `ApproveRequestedPlayer` violation case (`seatsAvailable=false`).
Binding: `Participation·approve` → `respondRequest(host, sessionId, userId, 'approved')`
via the scenario harness (real DB, overflow-guard trigger active).

```ts
// tauto: Participation/approve/ApproveRequestedPlayer · approve__violation_seats
it('rejects approval when no player seats remain', async () => {
  const { host, sessionId } = await seedSession({ seats: 2 })      // host + 1 player seat
  const [p1, p2] = await seedRequests(sessionId, 2)                 // given: status=Requested
  await respondRequest(host, sessionId, p1.id, 'approved')          // fills the table
  await expect(                                                     // given: seatsAvailable=false
    respondRequest(host, sessionId, p2.id, 'approved'),
  ).rejects.toThrow(/session_seats_full/)                           // should_pass: false
  expect(await statusOf(sessionId, p2.id)).toBe('requested')        // no ensures effect
})
```

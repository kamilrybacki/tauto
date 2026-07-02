# tauto capability benchmark

A rich, intertwined e-commerce fulfilment domain (CustomerAccount → Order →
Payment → InventoryReservation → Shipment, chained via snapshot fields) that
deliberately exercises **every tauto capability**, plus ODCS data-product
contracts. It doubles as a **regression guard**: `run_benchmark.py` runs tauto
against it and asserts a scorecard.

```bash
TAUTO_BIN=./target/release/tauto python3 examples/benchmark/run_benchmark.py
```

## What each stress case checks

| # | Case | Rule(s) | tauto capability |
|---|------|---------|------------------|
| 1 | Genuine conflict | `ApproveSubmittedOrder` vs `…Conflict` | conflict detection (+ Lean proof) |
| 2 | Non-conflict | `CancelInitiatedPayment` vs `VoidAuthorizedPayment` | precondition-aware suppression (disjoint guards) |
| 3 | Dead rule | `DeadPreferredReview` | unsatisfiable requires (+ Lean proof) |
| 4 | Cross-entity trap | `ShipWithCrossEntityPaymentTrap` | glossary `cross_entity_reference` |
| 5 | Unknown field | `UnknownPriorityExpedite` | glossary `unknown_field` |
| 6 | Lifecycle gap | `Payment.paymentStatus.Settled` | isolated/uncovered state |
| 7 | Conformance fail | `DeliverSignedShipment` | rule vs intent (mis-formalized example) |
| 8 | Conformance underspecified | `MarkReadyForFulfillment` | example missing a guarded field |
| 9 | Conformance pass | `VerifyTrustedProspect`, … | correct examples conform |
| 10–12 | Types / clauses / workflow | all 25 rules | parse + typecheck + coherent chain |
| 13 | ODCS drift | `payment.odcs.yaml` | `observed_not_declared` / `declared_not_observed` |

The intentional bad rules (cases 1, 3, 4, 5) and bad examples (7, 8) are
ground-truth stress cases, not authoring mistakes.

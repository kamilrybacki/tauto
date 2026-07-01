# Proposed rule — no explicit state guard (L2 demo)

`Mortgage.status` is a declared **state** field, so every rule on a Mortgage
should explicitly guard on it (state its source state). This rule guards only on
`credit_score`, leaving the source state implicit — `check_rule` returns a
`missing_state_guard` advisory warning. It is still conflict-free, so
`compatible` stays true; the warning simply asks the author to name the state
the transition starts from.

```contract
case ApproveByScoreOnly
entity:
  Mortgage
operation:
  approveApplication
requires:
  loan.credit_score >= 720
ensures:
  result.status == Approved
```

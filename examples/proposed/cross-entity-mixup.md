# Proposed rule — a Mortgage rule that mistakenly reaches into Package (glossary demo)

This rule is about `Mortgage` but references `package.weight_kg` and an
undeclared `loan.shoe_size`, and uses an enum value `Frozen` that the glossary
does not list for `loan.status`. It has no *conflict* (so `compatible: true`),
but `check_rule` returns advisory `glossary_warnings` flagging the vocabulary
drift — the Order-vs-Package distinction in action.

```contract
case ConfusedApproval
entity:
  Mortgage
operation:
  approveApplication
requires:
  loan.credit_score >= 700
  loan.status == Frozen
  package.weight_kg > 5
  loan.shoe_size == 9
ensures:
  result.status == Approved
```

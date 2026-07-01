# Case 4 — Legacy Reject Prime Application (conflicts with Case 1)
#
# Same entity/operation as ApprovePrimeMortgage but ensures result.status == Rejected
# while Case 1 ensures result.status == Approved.
# The upload endpoint must return 409 and roll this file back.

```contract
case LegacyRejectPrimeApplication
entity:
  Mortgage
operation:
  approveApplication
requires:
  loan.credit_score >= 750
  loan.status == UnderReview
ensures:
  result.status == Rejected
  result.rejection_reason == LegacySystemOverride
forbidden:
  disburseFunds(loan.id)
```

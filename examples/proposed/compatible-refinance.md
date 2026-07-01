# Proposed rule — refinance a funded mortgage (compatible)

A new operation on a different `entity/operation` than the seeded rules, so it
should check as **compatible** and come back with a generated test suite.

```contract
case RefinanceFundedMortgage
entity:
  Mortgage
operation:
  refinance
requires:
  loan.status == Funded
  loan.outstanding_balance >= 10000
  loan.credit_score >= 700
ensures:
  result.status == Refinanced
  result.interest_rate == Reduced
forbidden:
  disburseFunds(loan.id)
preserves:
  loan.applicant_id
  loan.property_address
```

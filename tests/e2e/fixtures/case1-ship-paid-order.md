# Case 1 — Approve Prime Mortgage

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
  loan.income >= 60000
  loan.status == UnderReview
ensures:
  result.status == Approved
  result.interest_rate == Standard
  result.max_term_years == 30
forbidden:
  disburseFunds(loan.id)
preserves:
  loan.applicant_id
  loan.property_address
assumes:
  loan.credit_score > 0
```

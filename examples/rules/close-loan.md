# Case 3 — Close Completed Loan

```contract
case CloseCompletedLoan
entity:
  Mortgage
operation:
  closeLoan
requires:
  loan.status == Funded
  loan.final_payment_received == true
  loan.outstanding_balance == 0
ensures:
  result.status == Closed
  result.account_settled == true
forbidden:
  disburseFunds(loan.id)
  extendLoanTerm(loan.id)
preserves:
  loan.applicant_id
  loan.property_address
```

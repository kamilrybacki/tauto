# Case 2 — Disburse Funds After Closing

```contract
case DisburseFundsAfterClosing
entity:
  Mortgage
operation:
  disburseFunds
requires:
  loan.status == Approved
  loan.closing_documents_signed == true
  loan.appraisal_cleared == true
  loan.title_clear == true
ensures:
  result.status == Funded
  result.funds_transferred == true
forbidden:
  cancelLoan(loan.id)
preserves:
  loan.applicant_id
  loan.property_address
  loan.interest_rate
assumes:
  loan.funds_available == true
```

# Domain glossary

Canonical vocabulary for the rule set. Each ` ```glossary ` block defines one
entity: its name, the `aka` instance prefixes used in field paths, its fields
(with types / enum members), and its operations. The `result` prefix is the
universal post-state of an operation and shares the entity's fields.

`check_rule` validates a proposed rule's references against this glossary and
returns advisory warnings — e.g. an unknown field, or a `package.*` reference
inside a `Mortgage` rule (the Order-vs-Package distinction).

```glossary
entity Mortgage
aka: loan
describes: A home loan moving through underwriting, funding, and closing.
states:
  status: enum(UnderReview, Approved, Rejected, Funded, Closed, Refinanced)
fields:
  credit_score: int
  debt_to_income_ratio: int
  employment_verified: bool
  income: int
  interest_rate: enum(Standard, Reduced, Premium)
  max_term_years: int
  closing_documents_signed: bool
  appraisal_cleared: bool
  title_clear: bool
  funds_available: bool
  funds_transferred: bool
  final_payment_received: bool
  outstanding_balance: int
  account_settled: bool
  rejection_reason: enum(LowCreditScore, HighDTI, Unverified, LegacySystemOverride)
  applicant_id: string
  property_address: string
operations:
  approveApplication
  disburseFunds
  closeLoan
  refinance
```

```glossary
entity Package
aka: package, parcel
describes: A physical shipment moving through a fulfillment pipeline. Defined
  here to demonstrate cross-entity disambiguation — a Mortgage rule that
  references package.* fields is flagged.
states:
  status: enum(Pending, Dispatched, InTransit, Delivered, Returned)
fields:
  weight_kg: int
  tracking_number: string
operations:
  dispatch
  deliver
```

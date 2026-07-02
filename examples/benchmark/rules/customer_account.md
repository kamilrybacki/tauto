```contract
case VerifyTrustedProspect
entity:
  CustomerAccount
operation:
  verify
requires:
  customer.accountStatus == Prospect
  customer.verified == true
  customer.consentToTerms == true
  customer.riskScore >= 600
ensures:
  result.accountStatus == Active
intent:
  A verified prospect with acceptable risk and accepted terms becomes an active customer.
examples:
  - given: accountStatus=Prospect, verified=true, consentToTerms=true, riskScore=720; then: accountStatus=Active
  - given: accountStatus=Prospect, verified=false, consentToTerms=true, riskScore=720; applies: false
```

```contract
case SuspendAfterChargebacks
entity:
  CustomerAccount
operation:
  suspend
requires:
  customer.accountStatus == Active
  customer.chargebackCount > 2
ensures:
  result.accountStatus == Suspended
forbidden:
  placeOrder(customer.id)
intent:
  Customers with repeated chargebacks are suspended and cannot place new orders.
examples:
  - given: accountStatus=Active, chargebackCount=3; then: accountStatus=Suspended
  - given: accountStatus=Active, chargebackCount=1; applies: false
```

```contract
case DeadPreferredReview
entity:
  CustomerAccount
operation:
  grantPreferred
requires:
  customer.accountStatus == Active
  customer.riskScore >= 750
  customer.riskScore < 600
ensures:
  result.accountStatus == Active
intent:
  Intentionally dead benchmark rule: impossible risk-score window.
examples:
  - given: accountStatus=Active, riskScore=800; applies: false
```

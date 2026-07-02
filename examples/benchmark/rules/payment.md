```contract
case AuthorizeInitiatedPayment
entity:
  Payment
operation:
  authorize
requires:
  payment.paymentStatus == Initiated
  payment.customerStatusSnapshot == Active
  payment.amountCents > 0
  payment.authorizationCodePresent == true
  payment.threeDSecure == true
ensures:
  result.paymentStatus == Authorized
intent:
  An initiated payment may be authorized only for an active customer with a valid authorization path.
examples:
  - given: paymentStatus=Initiated, customerStatusSnapshot=Active, amountCents=12999, authorizationCodePresent=true, threeDSecure=true; then: paymentStatus=Authorized
  - given: paymentStatus=Initiated, customerStatusSnapshot=Suspended, amountCents=12999, authorizationCodePresent=true, threeDSecure=true; applies: false
```

```contract
case CaptureAuthorizedPayment
entity:
  Payment
operation:
  capture
requires:
  payment.paymentStatus == Authorized
  payment.orderStatusSnapshot == Approved
  payment.amountCents > 0
ensures:
  result.paymentStatus == Captured
  result.refundableAmountCents > 0
intent:
  Authorized funds are captured only after the order is approved.
examples:
  - given: paymentStatus=Authorized, orderStatusSnapshot=Approved, amountCents=12999; then: paymentStatus=Captured
  - given: paymentStatus=Authorized, orderStatusSnapshot=Draft, amountCents=12999; applies: false
```

```contract
case CancelInitiatedPayment
entity:
  Payment
operation:
  cancel
requires:
  payment.paymentStatus == Initiated
ensures:
  result.paymentStatus == Voided
intent:
  An uncaptured initiated payment can be voided before authorization.
examples:
  - given: paymentStatus=Initiated; then: paymentStatus=Voided
  - given: paymentStatus=Authorized; applies: false
```

```contract
case VoidAuthorizedPayment
entity:
  Payment
operation:
  cancel
requires:
  payment.paymentStatus == Authorized
  payment.orderStatusSnapshot != Shipped
ensures:
  result.paymentStatus == Voided
intent:
  An authorized payment can be voided while the order has not yet shipped.
examples:
  - given: paymentStatus=Authorized, orderStatusSnapshot=Approved; then: paymentStatus=Voided
  - given: paymentStatus=Authorized, orderStatusSnapshot=Shipped; applies: false
```

```contract
case FailInitiatedPayment
entity:
  Payment
operation:
  fail
requires:
  payment.paymentStatus == Initiated
  payment.authorizationCodePresent == false
ensures:
  result.paymentStatus == Failed
intent:
  An initiated payment with no authorization code fails.
examples:
  - given: paymentStatus=Initiated, authorizationCodePresent=false; then: paymentStatus=Failed
  - given: paymentStatus=Initiated, authorizationCodePresent=true; applies: false
```

```contract
case RefundCapturedPayment
entity:
  Payment
operation:
  refund
requires:
  payment.paymentStatus == Captured
  payment.orderStatusSnapshot == Refunding
  payment.refundableAmountCents > 0
ensures:
  result.paymentStatus == Refunded
  result.refundableAmountCents == 0
preserves:
  payment.amountCents
intent:
  A captured payment is refunded only after the delivered order enters refunding state.
examples:
  - given: paymentStatus=Captured, orderStatusSnapshot=Refunding, refundableAmountCents=12999; then: paymentStatus=Refunded
  - given: paymentStatus=Captured, orderStatusSnapshot=Delivered, refundableAmountCents=12999; applies: false
```

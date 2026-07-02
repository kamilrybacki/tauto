```glossary
entity CustomerAccount
aka: customer
describes: A buyer account whose verification and chargeback posture gates order submission.
states:
  accountStatus: enum(Prospect, Active, Suspended)
fields:
  id: string
  accountStatus: enum(Prospect, Active, Suspended)
  verified: bool
  consentToTerms: bool
  riskScore: int
  chargebackCount: int
operations:
  verify
  suspend
  restore
  grantPreferred
  placeOrder
```

```glossary
entity Order
aka: order
describes: A customer purchase that moves from draft through approval, fulfillment, delivery, and refund request.
states:
  orderStatus: enum(Draft, Submitted, Approved, ReadyToFulfill, Shipped, Delivered, Refunding, Cancelled)
fields:
  id: string
  orderStatus: enum(Draft, Submitted, Approved, ReadyToFulfill, Shipped, Delivered, Refunding, Cancelled)
  customerStatusSnapshot: enum(Prospect, Active, Suspended)
  paymentStatusSnapshot: enum(Initiated, Authorized, Captured, Voided, Refunded, Failed, Settled)
  reservationStatusSnapshot: enum(Requested, Held, Consumed, Released)
  shipmentStatusSnapshot: enum(Created, LabelPrinted, Picked, InTransit, Delivered)
  totalCents: int
  fraudHold: bool
  expedited: bool
  requiresSignature: bool
operations:
  submit
  approve
  markReady
  ship
  markDelivered
  requestReturn
  cancel
```

```glossary
entity Payment
aka: payment
describes: A payment authorization and capture lifecycle tied to order approval and refund state.
states:
  paymentStatus: enum(Initiated, Authorized, Captured, Voided, Refunded, Failed, Settled)
fields:
  id: string
  paymentStatus: enum(Initiated, Authorized, Captured, Voided, Refunded, Failed, Settled)
  orderStatusSnapshot: enum(Draft, Submitted, Approved, ReadyToFulfill, Shipped, Delivered, Refunding, Cancelled)
  customerStatusSnapshot: enum(Prospect, Active, Suspended)
  amountCents: int
  capturedAmountCents: int
  refundableAmountCents: int
  authorizationCodePresent: bool
  threeDSecure: bool
operations:
  authorize
  capture
  cancel
  fail
  refund
```

```glossary
entity InventoryReservation
aka: reservation
describes: A stock hold for an approved order that must exist before fulfillment can proceed.
states:
  reservationStatus: enum(Requested, Held, Consumed, Released)
fields:
  id: string
  reservationStatus: enum(Requested, Held, Consumed, Released)
  orderStatusSnapshot: enum(Draft, Submitted, Approved, ReadyToFulfill, Shipped, Delivered, Refunding, Cancelled)
  paymentStatusSnapshot: enum(Initiated, Authorized, Captured, Voided, Refunded, Failed, Settled)
  availableUnits: int
  fraudHold: bool
  sku: string
operations:
  hold
  consume
  release
```

```glossary
entity Shipment
aka: shipment
describes: A physical shipment whose carrier lifecycle feeds back into the order lifecycle.
states:
  shipmentStatus: enum(Created, LabelPrinted, Picked, InTransit, Delivered)
fields:
  id: string
  shipmentStatus: enum(Created, LabelPrinted, Picked, InTransit, Delivered)
  orderStatusSnapshot: enum(Draft, Submitted, Approved, ReadyToFulfill, Shipped, Delivered, Refunding, Cancelled)
  reservationStatusSnapshot: enum(Requested, Held, Consumed, Released)
  addressVerified: bool
  carrierAccepted: bool
  requiresSignature: bool
  signedByRecipient: bool
  packageWeightGrams: int
operations:
  printLabel
  pick
  handOff
  deliver
```

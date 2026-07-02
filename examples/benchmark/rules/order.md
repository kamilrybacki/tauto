```contract
case SubmitDraftOrder
entity:
  Order
operation:
  submit
requires:
  order.orderStatus == Draft
  order.customerStatusSnapshot == Active
  order.totalCents > 0
  order.fraudHold == false
ensures:
  result.orderStatus == Submitted
intent:
  Only active customers with positive-value, non-held drafts may submit an order.
examples:
  - given: orderStatus=Draft, customerStatusSnapshot=Active, totalCents=12999, fraudHold=false; then: orderStatus=Submitted
  - given: orderStatus=Draft, customerStatusSnapshot=Suspended, totalCents=12999, fraudHold=false; applies: false
```

```contract
case ApproveSubmittedOrder
entity:
  Order
operation:
  approve
requires:
  order.orderStatus == Submitted
  order.fraudHold == false
ensures:
  result.orderStatus == Approved
intent:
  A submitted order without fraud hold is approved.
examples:
  - given: orderStatus=Submitted, fraudHold=false; then: orderStatus=Approved
```

```contract
case ApproveSubmittedOrderConflict
entity:
  Order
operation:
  approve
requires:
  order.orderStatus == Submitted
  order.fraudHold == false
ensures:
  result.orderStatus == Cancelled
intent:
  Intentional benchmark conflict: same approval guard wrongly cancels the order.
examples:
  - given: orderStatus=Submitted, fraudHold=false; then: orderStatus=Cancelled
```

```contract
case MarkReadyForFulfillment
entity:
  Order
operation:
  markReady
requires:
  order.orderStatus == Approved
  order.paymentStatusSnapshot == Captured
  order.reservationStatusSnapshot == Held
ensures:
  result.orderStatus == ReadyToFulfill
intent:
  An approved order becomes fulfillment-ready only after payment capture and inventory hold.
examples:
  - given: orderStatus=Approved, paymentStatusSnapshot=Captured; then: orderStatus=ReadyToFulfill
```

```contract
case ShipReadyOrder
entity:
  Order
operation:
  ship
requires:
  order.orderStatus == ReadyToFulfill
  order.paymentStatusSnapshot == Captured
  order.reservationStatusSnapshot == Consumed
  order.shipmentStatusSnapshot == InTransit
ensures:
  result.orderStatus == Shipped
forbidden:
  cancel(order.id)
intent:
  A ready order ships only after payment captured, inventory consumed, and carrier accepted.
examples:
  - given: orderStatus=ReadyToFulfill, paymentStatusSnapshot=Captured, reservationStatusSnapshot=Consumed, shipmentStatusSnapshot=InTransit; then: orderStatus=Shipped
  - given: orderStatus=ReadyToFulfill, paymentStatusSnapshot=Authorized, reservationStatusSnapshot=Consumed, shipmentStatusSnapshot=InTransit; applies: false
```

```contract
case ShipWithCrossEntityPaymentTrap
entity:
  Order
operation:
  ship
requires:
  order.orderStatus == ReadyToFulfill
  payment.paymentStatus == Captured
ensures:
  result.orderStatus == Shipped
intent:
  Intentional trap: should have used order.paymentStatusSnapshot, not payment.paymentStatus.
examples:
  - given: orderStatus=ReadyToFulfill, paymentStatus=Captured; then: orderStatus=Shipped
```

```contract
case UnknownPriorityExpedite
entity:
  Order
operation:
  ship
requires:
  order.orderStatus == ReadyToFulfill
  order.priority == Platinum
ensures:
  result.orderStatus == Shipped
intent:
  Intentional trap: priority and Platinum are not declared.
examples:
  - given: orderStatus=ReadyToFulfill, priority=Platinum; then: orderStatus=Shipped
```

```contract
case MarkDeliveredOrder
entity:
  Order
operation:
  markDelivered
requires:
  order.orderStatus == Shipped
  order.shipmentStatusSnapshot == Delivered
ensures:
  result.orderStatus == Delivered
intent:
  A shipped order is delivered only after the shipment reports delivery.
examples:
  - given: orderStatus=Shipped, shipmentStatusSnapshot=Delivered; then: orderStatus=Delivered
  - given: orderStatus=Shipped, shipmentStatusSnapshot=InTransit; applies: false
```

```contract
case RequestReturnDeliveredOrder
entity:
  Order
operation:
  requestReturn
requires:
  order.orderStatus == Delivered
  order.totalCents > 0
ensures:
  result.orderStatus == Refunding
preserves:
  order.totalCents
intent:
  A delivered paid order opens the refund workflow without mutating the total.
examples:
  - given: orderStatus=Delivered, totalCents=12999; then: orderStatus=Refunding
  - given: orderStatus=Shipped, totalCents=12999; applies: false
```

```contract
case HoldRequestedReservation
entity:
  InventoryReservation
operation:
  hold
requires:
  reservation.reservationStatus == Requested
  reservation.orderStatusSnapshot == Approved
  reservation.availableUnits > 0
  reservation.fraudHold == false
ensures:
  result.reservationStatus == Held
intent:
  Inventory is held only for approved, non-fraud-held orders with available stock.
examples:
  - given: reservationStatus=Requested, orderStatusSnapshot=Approved, availableUnits=4, fraudHold=false; then: reservationStatus=Held
  - given: reservationStatus=Requested, orderStatusSnapshot=Approved, availableUnits=0, fraudHold=false; applies: false
```

```contract
case ConsumeHeldReservation
entity:
  InventoryReservation
operation:
  consume
requires:
  reservation.reservationStatus == Held
  reservation.orderStatusSnapshot == ReadyToFulfill
ensures:
  result.reservationStatus == Consumed
intent:
  A held reservation is consumed when the order is ready to leave the warehouse.
examples:
  - given: reservationStatus=Held, orderStatusSnapshot=ReadyToFulfill; then: reservationStatus=Consumed
  - given: reservationStatus=Held, orderStatusSnapshot=Approved; applies: false
```

```contract
case ReleaseHeldReservation
entity:
  InventoryReservation
operation:
  release
requires:
  reservation.reservationStatus == Held
  reservation.paymentStatusSnapshot == Voided
ensures:
  result.reservationStatus == Released
preserves:
  reservation.sku
intent:
  Held stock is released if the associated payment is voided before fulfillment.
examples:
  - given: reservationStatus=Held, paymentStatusSnapshot=Voided; then: reservationStatus=Released
  - given: reservationStatus=Held, paymentStatusSnapshot=Captured; applies: false
```

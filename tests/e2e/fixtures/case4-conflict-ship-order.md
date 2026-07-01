# Case 4 — Reject Ship When Paid (conflicts with Case 1)
#
# Same entity/operation/requires as ShipPaidOrder but contradictory ensures:
# Case 1 says result.status == Shipped; this says result.status == Rejected.
# The upload endpoint must return 409 and roll this file back.

```contract
case RejectShipWhenPaid
entity:
  Order
operation:
  shipOrder
requires:
  order.status == Paid
ensures:
  result.status == Rejected
```

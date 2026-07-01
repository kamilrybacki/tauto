# Case 3 — Cancel Pending Order

```contract
case CancelPendingOrder
entity:
  Order
operation:
  cancelOrder
requires:
  order.status == Pending
ensures:
  result.status == Cancelled
```

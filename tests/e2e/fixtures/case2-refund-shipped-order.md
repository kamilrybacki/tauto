# Case 2 — Refund Shipped Order

```contract
case RefundShippedOrder
entity:
  Order
operation:
  refundOrder
requires:
  order.status == Shipped
ensures:
  result.status == Refunded
```

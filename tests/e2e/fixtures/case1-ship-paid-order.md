# Case 1 — Ship Paid Order

```contract
case ShipPaidOrder
entity:
  Order
operation:
  shipOrder
requires:
  order.status == Paid
ensures:
  result.status == Shipped
```

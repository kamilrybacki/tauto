```contract
case CancelPaidOrder
entity:
  Order
operation:
  cancelOrder
requires:
  order.status == Paid
ensures:
  result.status == Cancelled
```

```contract
case ShipApprovedOrder
entity:
  Order
operation:
  shipOrder
requires:
  order.status == Approved
ensures:
  result.status == Shipped
```

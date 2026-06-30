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

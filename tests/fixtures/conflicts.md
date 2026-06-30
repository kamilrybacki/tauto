# Conflict test

```contract
case CancelA
entity:
  Order
operation:
  cancelOrder
ensures:
  result.status == Cancelled
```

```contract
case CancelB
entity:
  Order
operation:
  cancelOrder
ensures:
  result.status == Active
```

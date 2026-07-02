```contract
case PrintLabelForReadyOrder
entity:
  Shipment
operation:
  printLabel
requires:
  shipment.shipmentStatus == Created
  shipment.orderStatusSnapshot == ReadyToFulfill
  shipment.addressVerified == true
  shipment.packageWeightGrams > 0
ensures:
  result.shipmentStatus == LabelPrinted
intent:
  A label is printed only for a fulfillment-ready order with a verified address and positive weight.
examples:
  - given: shipmentStatus=Created, orderStatusSnapshot=ReadyToFulfill, addressVerified=true, packageWeightGrams=850; then: shipmentStatus=LabelPrinted
  - given: shipmentStatus=Created, orderStatusSnapshot=Approved, addressVerified=true, packageWeightGrams=850; applies: false
```

```contract
case PickLabelledShipment
entity:
  Shipment
operation:
  pick
requires:
  shipment.shipmentStatus == LabelPrinted
  shipment.reservationStatusSnapshot == Held
ensures:
  result.shipmentStatus == Picked
intent:
  A labelled shipment is picked only while the inventory reservation is still held.
examples:
  - given: shipmentStatus=LabelPrinted, reservationStatusSnapshot=Held; then: shipmentStatus=Picked
  - given: shipmentStatus=LabelPrinted, reservationStatusSnapshot=Released; applies: false
```

```contract
case HandOffPickedShipment
entity:
  Shipment
operation:
  handOff
requires:
  shipment.shipmentStatus == Picked
  shipment.carrierAccepted == true
ensures:
  result.shipmentStatus == InTransit
intent:
  A picked shipment enters transit only after the carrier accepts it.
examples:
  - given: shipmentStatus=Picked, carrierAccepted=true; then: shipmentStatus=InTransit
  - given: shipmentStatus=Picked, carrierAccepted=false; applies: false
```

```contract
case DeliverSignedShipment
entity:
  Shipment
operation:
  deliver
requires:
  shipment.shipmentStatus == InTransit
  shipment.requiresSignature == true
  shipment.signedByRecipient == true
ensures:
  result.shipmentStatus == Delivered
intent:
  A signature-required shipment is delivered only when the recipient signature is present.
examples:
  - given: shipmentStatus=InTransit, requiresSignature=true, signedByRecipient=true; then: shipmentStatus=InTransit
  - given: shipmentStatus=InTransit, requiresSignature=true, signedByRecipient=false; applies: false
```

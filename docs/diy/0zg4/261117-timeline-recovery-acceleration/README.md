---
prerequisites:
  - 0zg3/261024-datashed-backup-recovery
---

[analogue]: <../../../glossary/analogue.md>
[bulk-order]: <../../../glossary/bulk-order.md>
[datashed]: <../../../glossary/datashed.md>
[diy/0zg4/261117-timeline-recovery-acceleration/0zg5/261117-creating-bulk-orders]: <0zg5/261117-creating-bulk-orders/README.md>
[diy/0zg4/261117-timeline-recovery-acceleration/0zg6/261117-recovering-from-bulk-orders]: <0zg6/261117-recovering-from-bulk-orders/README.md>
[flow]: <../../../glossary/flow.md>
[timeline]: <../../../glossary/timeline.md>

# Timeline recovery acceleration

Recovering *Timeline*[~][timeline] history one object per event becomes expensive
when *Flow*[~][flow] commit freshness is controlled externally. Introduce an
*Analogue*[~][analogue] of *BulkOrders*[~][bulk-order]; use it for the
*Datashed*[~][datashed] first.

Begin with *Creating BulkOrders*[~][diy/0zg4/261117-timeline-recovery-acceleration/0zg5/261117-creating-bulk-orders],
then *Recovering from BulkOrders*[~][diy/0zg4/261117-timeline-recovery-acceleration/0zg6/261117-recovering-from-bulk-orders].

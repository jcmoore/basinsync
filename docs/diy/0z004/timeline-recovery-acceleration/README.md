---
prerequisites:
  - 0z003/datashed-backup-recovery
---

[analogue]: <../../../glossary/analogue.md>
[bulk-order]: <../../../glossary/bulk-order.md>
[datashed]: <../../../glossary/datashed.md>
[diy/0z004/timeline-recovery-acceleration/0z005/creating-bulk-orders]: <0z005/creating-bulk-orders/README.md>
[diy/0z004/timeline-recovery-acceleration/0z006/recovering-from-bulk-orders]: <0z006/recovering-from-bulk-orders/README.md>
[flow]: <../../../glossary/flow.md>
[timeline]: <../../../glossary/timeline.md>

# Timeline recovery acceleration

Recovering *Timeline*[~][timeline] history one object per event becomes expensive
when *Flow*[~][flow] commit freshness is controlled externally. Introduce an
*Analogue*[~][analogue] of *BulkOrders*[~][bulk-order]; use it for the
*Datashed*[~][datashed] first.

Begin with *Creating BulkOrders*[~][diy/0z004/timeline-recovery-acceleration/0z005/creating-bulk-orders],
then *Recovering from BulkOrders*[~][diy/0z004/timeline-recovery-acceleration/0z006/recovering-from-bulk-orders].

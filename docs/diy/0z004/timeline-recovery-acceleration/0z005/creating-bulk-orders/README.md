---
prerequisites:
  - 0z004/timeline-recovery-acceleration
---

[analogue]: <../../../../../glossary/analogue.md>
[benchmark]: <benchmark/benchmark.md>
[bulk-manifest]: <../../../../../glossary/bulk-manifest.md>
[bulk-order]: <../../../../../glossary/bulk-order.md>
[conditional-write]: <../../../../../glossary/conditional-write.md>
[datashed]: <../../../../../glossary/datashed.md>
[development]: <../../../../../development.md>
[differential-event]: <../../../../../glossary/differential-event.md>
[envbuf]: <../../../../../glossary/envbuf.md>
[margin]: <../../../../../glossary/margin.md>
[milestone]: <../../../../../glossary/milestone.md>
[object-keys]: <../../../../../object-keys.md>
[reservoir]: <../../../../../glossary/reservoir.md>
[timeline]: <../../../../../glossary/timeline.md>

# Creating BulkOrders

Give each *Reservoir*[~][reservoir] an *Analogue*[~][analogue]. Its *BulkOrders*[~][bulk-order]
concatenate complete *DifferentialEvents*[~][differential-event] from the
*Timeline*[~][timeline], retaining each *event's*[~][differential-event] nested *EnvBuf*[~][envbuf]. Follow
the *key layout*[~][object-keys]: complemented lower bounds sort descending,
then complemented exclusive upper bounds sort descending.

For the *Datashed*[~][datashed], hand durably published bytes to a background
worker through a bounded, nonblocking queue. Include the bytes actually accepted
after any *margin retry*[~][margin]. On overflow or restart, fetch missing events
from the durable *Timeline*[~][timeline]; resume beyond contiguous existing orders.

Accumulate in memory. Upload after reaching at least 5 MiB of whole events or
30 seconds from the first buffered event; flush a partial order during shutdown.
Append the *BulkManifest*[~][bulk-manifest] and its own *counter margin*[~][margin],
record the exclusive event end in the key and the footer range in metadata, and use one *conditional PUT*[~][conditional-write].
Concurrent workers may create overlapping intervals; adopt an existing identical
event sequence at the same key, allowing differing trailing event margins.
Worker failures retry independently of client acknowledgments.

`BASINSYNC_BULK_TARGET_BYTES` defaults to `5242880`;
`BASINSYNC_BULK_MAX_AGE_MS` defaults to `30000`, with zero disabling creation.
Disk-backed accumulation and multipart uploads remain for later work.

Test the two independent codecs, interval/offset boundaries, exact archived bytes,
byte/age/shutdown flushing, queue overflow, retry margins, competing workers,
and restart. Run the *development checks*[~][development].

See the *measurements*[~][benchmark].

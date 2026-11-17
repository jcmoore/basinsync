---
prerequisites:
  - 0z004/timeline-recovery-acceleration/0z005/creating-bulk-orders
---

[analogue]: <../../../../../glossary/analogue.md>
[benchmark]: <benchmark/benchmark.md>
[bulk-order]: <../../../../../glossary/bulk-order.md>
[commit-marker]: <../../../../../glossary/commit-marker.md>
[conditional-read]: <../../../../../glossary/conditional-read.md>
[datashed-backup]: <../../../../../datashed-backup.md>
[differential-event]: <../../../../../glossary/differential-event.md>
[lexidecimal]: <../../../../../glossary/lexidecimal.md>
[milestone]: <../../../../../glossary/milestone.md>
[timeline]: <../../../../../glossary/timeline.md>

# Recovering from BulkOrders

Load the latest *CommitMarker*[~][commit-marker] through the existing
*snapshot recovery*[~][datashed-backup], then apply subsequent
*DifferentialEvents*[~][differential-event] in *Milestone*[~][milestone] order.

Fetch the next *Timeline*[~][timeline] *event*[~][differential-event] immediately while a separate worker
LISTs eligible *BulkOrders*[~][bulk-order]. For next position `c`, seek after
`analogue/${lex(~c)}/`; retain intervals satisfying `lowest <= c < highest`.
Prefer the greatest exclusive upper bound, retaining weaker candidates as fallbacks.
Let discovery search ahead of ingestion; LIST newly eligible lower-bound bands
and stop at previously examined bands. Keep useful orders ending behind that
search frontier. A min-upper-bound queue identifies intervals contained by later
entries in each descending-lower-bound scan; retain this information without deleting objects.

Fetch either the entire order or its footer plus the useful event range. Verify
the key/index bounds, offsets, framing, and each *event's*[~][differential-event] existing ingestion and
predecessor checks. Only validated, contiguous application advances the materialized
cursor; advertised candidate endpoints never do. A failed or invalid order falls
back to the *Timeline*[~][timeline]. Discovery must not hold up direct progress.
Use *conditional reads*[~][conditional-read].

`BASINSYNC_RECOVERY_MODE` defaults to `bulk-parallel`; alternatives are `sequential`,
`parallel`, `bulk`, `range`, `hedged`, and `range-parallel`.
Parallel mode prefetches individual events; Bulk mode overlaps
LIST discovery with retrieval; range mode selects useful suffixes; hedged mode
keeps one individual GET progressing beside a BulkOrder GET. Apply all results in
order. `BASINSYNC_RECOVERY_CONCURRENCY` defaults to four;
`BASINSYNC_RECOVERY_MAX_BYTES` bounds each data response, defaulting to 128 MiB.

`bulk-parallel` and `range-parallel` modes prefetch beyond unfinished
orders using their advertised endpoints, while still ingesting strictly in order.
`BASINSYNC_RECOVERY_BULK_CONCURRENCY` defaults to two; completed future orders
retain their slots. `BASINSYNC_RECOVERY_BULK_BUFFER_BYTES` defaults to 32 MiB,
covering buffered archive payloads plus in-flight reservations using listed object
sizes, including for Range reads. Individual fallback responses, transient buffers,
and database memory are separate. Oversized orders fall back to individual reads.

Compare exact recovered database images across all strategies, including overlapping
and contained intervals, paginated listings, snapshot suffixes, corruption,
unavailable orders, and blocked discovery. Benchmark latency, transferred bytes,
request counts, and queued payload memory against one-by-one retrieval.

See the *measurements*[~][benchmark].

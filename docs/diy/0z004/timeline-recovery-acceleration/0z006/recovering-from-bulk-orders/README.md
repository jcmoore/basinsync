---
prerequisites:
  - 0z004/timeline-recovery-acceleration/0z005/creating-bulk-orders
---

[analogue]: <../../../../../glossary/analogue.md>
[benchmark]: <benchmark/benchmark.md>
[bidirectional]: <benchmark/bidirectional.md>
[bulk-manifest]: <../../../../../glossary/bulk-manifest.md>
[bulk-order]: <../../../../../glossary/bulk-order.md>
[byte-coordinate]: <../../../../../glossary/byte-coordinate.md>
[commit-marker]: <../../../../../glossary/commit-marker.md>
[conditional-read]: <../../../../../glossary/conditional-read.md>
[datashed]: <../../../../../glossary/datashed.md>
[datashed-backup]: <../../../../../datashed-backup.md>
[differential-event]: <../../../../../glossary/differential-event.md>
[jet]: <../../../../../glossary/jet.md>
[lexidecimal]: <../../../../../glossary/lexidecimal.md>
[margin]: <../../../../../glossary/margin.md>
[milestone]: <../../../../../glossary/milestone.md>
[sha384]: <../../../../../glossary/sha384.md>
[timeline]: <../../../../../glossary/timeline.md>

# Recovering from BulkOrders

Load the latest *CommitMarker*[~][commit-marker] through the existing
*snapshot recovery*[~][datashed-backup], then apply subsequent
*DifferentialEvents*[~][differential-event] in *Milestone*[~][milestone] order.

Fetch the next *Timeline*[~][timeline] *event*[~][differential-event] immediately while a separate worker
LISTs eligible *BulkOrders*[~][bulk-order]. For next position `c`, seek after
`kernelspace/datashed/analogue/${lex(~c)}/`; retain intervals satisfying `lowest <= c < highest`.
Prefer the greatest exclusive upper bound, retaining weaker candidates as fallbacks.
Let discovery search ahead of ingestion; LIST newly eligible lower-bound bands
and stop at previously examined bands. Keep useful orders ending behind that
search frontier. A min-upper-bound queue identifies intervals contained by later
entries in each descending-lower-bound scan; retain this information without deleting objects.

Fetch either the entire order or its *BulkManifest*[~][bulk-manifest] plus the useful event range.
The key supplies the exclusive event end; fetch `bytes=<bulkmanifest-start>-` and parse its
first envelope as the footer, skipping trailing *margins*[~][margin] without HEAD. Verify
the key/index bounds, offsets, framing, and each *event's*[~][differential-event] existing ingestion and
predecessor checks. Only validated, contiguous application advances the materialized
cursor; advertised candidate endpoints never do. A failed or invalid order falls
back to the *Timeline*[~][timeline]. Discovery must not hold up direct progress.
Use *conditional reads*[~][conditional-read].

Keep `bulk-parallel` for *Datasheds*[~][datashed]; plan to use `bidirectional`
for *Jets*[~][jet] when introduced.

`BASINSYNC_RECOVERY_MODE` defaults to `bulk-parallel`; alternatives are `sequential`,
`parallel`, `bulk`, `range`, `hedged`, `range-parallel`, `range-prefetch`, and `bidirectional`.
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

`range-prefetch` separates footer probes from data downloads: cache/probe up to
`BASINSYNC_RECOVERY_MANIFEST_CONCURRENCY` manifests (default eight, each fetched tail capped
at 1 MiB), while retaining the two-slot data window. Probes advance discovery
using advertised endpoints; only ingestion advances state. Its data-byte budget
reserves the selected ranges rather than whole-object lengths.

`bidirectional` fetches individual *Timeline*[~][timeline] *events*[~][differential-event]
descending from a fixed tail while fetching *BulkOrders*[~][bulk-order] ascending
with two data slots. Replay only ascending. Trimmed event bytes share logical
*byte coordinates*[~][byte-coordinate] with physical offsets, so known boundaries
translate directly into Ranges without a *BulkManifest*[~][bulk-manifest] GET.
Trim retained prefixes and suffixes; hints never advance the applied cursor.

Cache descending results in a rolling list of roughly size-capped disk files;
checksum each record's *Milestone*[~][milestone], length, and complete bytes with
*SHA-384*[~][sha384] before writing, verifying before replay. This disposable cache needs no
fsync. Evict old segments or bypass saturated/failed writes rather than waiting
for cache space. Only retained coverage counts toward meeting; refetch evicted
or damaged events. After descending reaches the applied cursor, fill remaining
holes forward even if archive reads remain pending.

`BASINSYNC_RECOVERY_DISK_BYTES` defaults to 256 MiB (zero disables caching);
`BASINSYNC_RECOVERY_DISK_SEGMENT_BYTES` defaults to 8 MiB. An oversized event
gets a dedicated segment if it fits the total budget. `BASINSYNC_RECOVERY_DISK_DIRECTORY`
optionally selects the temporary-file parent. Two queued writes and one active
write have separate per-response memory limits. The directory is disposable
after restart. See the *bidirectional measurements*[~][bidirectional].

Compare exact recovered database images across all strategies, including overlapping
and contained intervals, paginated listings, snapshot suffixes, corruption,
unavailable orders, and blocked discovery. Benchmark latency, transferred bytes,
request counts, and queued payload memory against one-by-one retrieval.

See the *measurements*[~][benchmark].

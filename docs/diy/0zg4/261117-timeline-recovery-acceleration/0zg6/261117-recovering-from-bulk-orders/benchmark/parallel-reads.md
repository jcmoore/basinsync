[benchmark]: <benchmark.md>
[bulk-order]: <../../../../../../glossary/bulk-order.md>
[manifest-probes]: <manifest-probes.md>
[milestone]: <../../../../../../glossary/milestone.md>
[parallel]: <parallel.json>
[parallel-overlap]: <parallel-overlap.json>

# Parallel BulkOrder reads

These measurements predate footer ranges in keys; see the
*no-HEAD and separate-probe comparison*[~][manifest-probes].

Repeat the *previous protocol*[~][benchmark]: 96 events (23.04 MiB), five adjacent
*BulkOrders*[~][bulk-order], two rotated rounds per strategy, from genesis and
from the snapshot at *Milestone*[~][milestone] 72. Same release build, Apple M2 and VersityGW.
All recovered images match the sequential baseline. Parallel modes use a 32 MiB
archive-payload budget with two- and four-slot windows. Times are median milliseconds
from genesis, including bootstrap and discovery; these are small simulated runs.

| Profile | Individual ×4 | Bulk ×1 | Bulk ×2 | Bulk ×4 | Range ×1 | Range ×2 | Range ×4 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| local | 154 | 142 | 121 | 131 | 153 | 132 | 123 |
| 25ms | 998 | 554 | 458 | 461 | 904 | 615 | 518 |
| 100ms | 3554 | 1724 | 1505 | 1498 | 2853 | 2053 | 1708 |
| 25ms-5MiBps | 4909 | 5414 | 5055 | 5120 | 5700 | 5292 | 5133 |

At 100 ms, two-slot Bulk reduces elapsed time by 13%; four gives essentially no
further gain. Actual whole-order concurrency peaks at two in both configurations:
new lower-bound bands still need successive LISTs. Range ×4 reaches four concurrent
order reads and reduces time by 40% versus Range ×1, overlapping each order's
HEAD/footer/data chain.

Transferred bytes remain about 23.63 MiB. The first 100 ms round reserves at most
10.40 MiB with Bulk ×2 and 20.68 MiB with Range ×4, versus 5.20 MiB serially.
Reservations include in-flight listed object sizes and retained future bodies;
they exclude the individual fallback, transient allocations and database memory.
Completed future bodies keep their slots, bounding lookahead when an earlier read stalls.

With the snapshot, 100 ms times are 2389/2370/2367 ms for Bulk ×1/×2/×4:
bootstrap and tail probes dominate. Under the shared 5 MiB/s link, individual
prefetch still wins; Bulk ×2 narrows the gap to 5055 versus 4909 ms.
Use `bulk-parallel` with two slots and a 32 MiB archive budget by default.
Four slots are more promising for Range reads.
See *raw measurements*[~][parallel].

For six staggered orders (24 events each, 16-event stride), parallel whole-order
GETs still transfer overlapping prefixes. At 100 ms, Bulk ×1/×2 takes
1848/1612 ms; Range ×1/×2 takes 3212/2143 ms. On the shared link, Range ×2
finishes in 5333 ms versus 7032 ms for Bulk ×2, transferring 23.64 versus
33.09 MiB. These two-slot results retain the same ordering and budget rules.
See the *overlapping-order samples*[~][parallel-overlap].

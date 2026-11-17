[bulk-order]: <../../../../../../glossary/bulk-order.md>
[measurements]: <measurements.json>
[milestone]: <../../../../../../glossary/milestone.md>
[overlap]: <overlap.json>
[parallel-reads]: <parallel-reads.md>

# Recovery measurements

Apple M2, 24 GiB RAM, macOS, release build, VersityGW 1.11.0 POSIX backend;
warm filesystem. Generate 96 genuine events with 32 KiB marker messages and
physical WAL deltas: 23.04 MiB total, packed into five adjacent *BulkOrders*[~][bulk-order]
at whole-event boundaries of at least 5 MiB. Run each strategy twice in rotated
order, both from genesis and from a snapshot at *Milestone*[~][milestone] 72.
Every run must reconstruct exactly the baseline database image.

Times below are median milliseconds from genesis, including bootstrap and discovery.
Profiles add request latency; the last also imposes a shared 5 MiB/s response-body
budget. This is a coarse proxy simulation, not a production network measurement.

| Profile | Sequential | Parallel (4) | Bulk | Range | Hedged |
| --- | ---: | ---: | ---: | ---: | ---: |
| local | 259 | 135 | 144 | 152 | 139 |
| 25ms | 3770 | 1020 | 568 | 908 | 550 |
| 100ms | 11785 | 3587 | 1725 | 2828 | 1720 |
| 25ms-5MiBps | 8518 | 4901 | 5362 | 5637 | 5515 |

At 100 ms, Bulk is 6.8× faster than sequential and 2.1× faster than parallel.
Its first round uses one individual GET, five archive GETs and six archive LISTs,
versus 96 individual GETs. It transfers 23.63 MiB versus 23.04 MiB and queues at
most 5.20 MiB of unique payload, versus 0.58 MiB sequential. That memory statistic
excludes in-flight responses, parser/database allocations, and process RSS.

Parallel wins locally and on the constrained link. After the snapshot, the
constrained-link times are 2705/1796/2414/2034/2436 ms respectively: Range avoids
most already-applied bytes but adds HEAD/footer requests. Hedging spends extra
bandwidth for little gain here. Keep Bulk as the latency-oriented default;
choose parallel or range when bandwidth dominates. See *raw measurements*[~][measurements].

With a competing order covering the entire history, the same snapshot/link test
takes 5477 ms with Bulk versus 1979 ms with Range: 23.85 versus 6.25 MiB transferred.
Choosing the furthest endpoint favors fewer requests, but can over-fetch badly.
See the *overlap measurements*[~][overlap].

Continue with the *parallel-order comparison*[~][parallel-reads].

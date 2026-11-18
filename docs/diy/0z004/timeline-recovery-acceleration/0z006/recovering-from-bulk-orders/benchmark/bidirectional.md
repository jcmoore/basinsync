[bidirectional-full]: <bidirectional-full.json>
[bidirectional-tail]: <bidirectional-tail.json>
[bulk-order]: <../../../../../../glossary/bulk-order.md>
[byte-coordinate]: <../../../../../../glossary/byte-coordinate.md>
[crc32c]: <../../../../../../glossary/crc32c.md>
[differential-event]: <../../../../../../glossary/differential-event.md>
[external-margins-full]: <external-margins-full.json>
[external-margins-tail]: <external-margins-tail.json>
[margin]: <../../../../../../glossary/margin.md>
[margin-free]: <margin-free.json>
[milestone]: <../../../../../../glossary/milestone.md>
[timeline]: <../../../../../../glossary/timeline.md>
[trimmed-events]: <trimmed-events.json>

# Bidirectional recovery

These initial measurements predate trimming event retry margins and plain
exclusive ends. The current trimmed format again permits direct coordinate-to-Range
translation; the paired-end experiment below was an intermediate protocol.

Apple M2, release build, VersityGW; 96 events with 32 KiB markers and real WAL
frames, totaling 23.05 MiB. Two rotated rounds per strategy, recovering from
genesis; require identical database images. Compare fully archived history
(five roughly 5 MiB orders) with only events 1–80 archived (four orders).

Median milliseconds, including bootstrap and discovery:

| Archive coverage | Network | Individual ×4 | Bulk ×2 | Manifest probes ×8/data ×2 | Bidirectional |
| --- | --- | ---: | ---: | ---: | ---: |
| Complete | +100 ms | 3386 | 1471 | 1578 | 1453 |
| Complete | +25 ms, shared 5 MiB/s | 4888 | 5012 | 5029 | 5076 |
| First 80 events | +100 ms | 3594 | 3205 | 3345 | 2045 |
| First 80 events | +25 ms, shared 5 MiB/s | 4910 | 5821 | 5619 | 5036 |

Bidirectional combines two ascending *BulkOrder*[~][bulk-order] slots with one
descending *Timeline*[~][timeline] reader and the default disk ring. *Byte coordinates*[~][byte-coordinate]
locate data ranges without manifest GETs. With 16 unarchived *events*[~][differential-event],
it reduces the +100 ms Bulk time by 36%; peak cached disk bytes are 2.36 MiB.
No cache evictions or bypasses occurred in these benchmark samples.

With complete archives it offers little latency benefit and transfers 24.35 MiB
versus Bulk's 23.64 MiB. The first +100 ms round records 28 versus 20 total S3
requests, including bootstrap and LISTs. Speculative Timeline reads can duplicate
in-flight archive ranges; coordinates cannot retract bytes already requested.
On the constrained link, individual ×4 remains fastest in both scenarios.
Keep Bulk ×2 as the default; use bidirectional when archive lag warrants it.
Two simulated rounds establish a useful comparison, not a stable ranking.

Raw samples: *complete archives*[~][bidirectional-full] and *unarchived tail*[~][bidirectional-tail].

Repeating the 16-event-tail, +100 ms comparison with margin-free coordinates
and paired exclusive ends gives 3249 ms Bulk ×2 versus 2123 ms bidirectional
(two rotated rounds, identical recovered images). Bidirectional now performs
four manifest GETs plus four data Ranges; those probes overlap descending reads.
This retains a 35% latency reduction in that scenario. See the *updated samples*[~][margin-free].

After trimming event retry margins and restoring plain exclusive ends, the same
two-round comparison gives 3133 ms Bulk ×2 versus 2007 ms bidirectional, with
identical recovered images. Bidirectional uses four data Ranges and no manifest
GETs, preserving the 36% latency reduction. See the *trimmed-event samples*[~][trimmed-events].

## External event margins and CRC32C

Repeat on `7b83f0e1`, with external *retry margins*[~][margin] and cumulative
*CRC32C*[~][crc32c]: four alternating rounds per strategy, same machine,
backend, 96-event workload, and genesis recovery. All 32 measured recoveries
match the sequential reference database image and agree on final
*Milestone*[~][milestone], *coordinate*[~][byte-coordinate], and checksum.

Median milliseconds:

| Archive coverage | Network | Bulk ×2 | Bidirectional |
| --- | --- | ---: | ---: |
| Complete | +100 ms | 1511 | 1506 |
| Complete | +25 ms, shared 5 MiB/s | 5156 | 5030 |
| First 80 events | +100 ms | 3169 | 1999 |
| First 80 events | +25 ms, shared 5 MiB/s | 5654 | 5040 |

With complete archives, bidirectional uses 28–29 total requests versus 20,
and transfers 24.00–24.17 MiB versus 23.63 MiB. With the unarchived tail,
it uses 36–37 requests versus 35 and 23.87 MiB versus 23.63 MiB,
reducing recovery time by 37% and 11% respectively. No manifest GETs are needed.
Keep Bulk ×2 as the default; bidirectional remains useful when archives lag.
These small, simulated runs support that tradeoff without establishing a production ranking.

Raw samples: *complete archives*[~][external-margins-full] and
*unarchived tail*[~][external-margins-tail].

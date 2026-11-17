[bulk-manifest]: <../../../../../../glossary/bulk-manifest.md>
[manifest-overlap]: <manifest-overlap.json>
[manifest-prefetch]: <manifest-prefetch.json>
[milestone]: <../../../../../../glossary/milestone.md>

# Key-located manifests and separate probes

Repeat the 96-event, five-order test on the same Apple M2, release build and
VersityGW backend, with two rotated rounds and exact recovered-image checks.
All Range readers now use the key's HTTP range to fetch the *BulkManifest*[~][bulk-manifest]
without HEAD. Compare complete per-order Range pipelines (two/four slots) with
separate manifest windows (two/four/eight slots), each keeping two data slots.
The latter reserve selected data bytes against the 32 MiB budget; manifests have
a separate bounded cache/probe window and 1 MiB per-manifest limit.

Median milliseconds from genesis, including bootstrap and discovery:

| Profile | Individual ×4 | Bulk ×2 | Range ×2 | Range ×4 | Probes ×2/data ×2 | Probes ×4/data ×2 | Probes ×8/data ×2 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| local | 151 | 123 | 122 | 119 | 123 | 122 | 122 |
| 25ms | 1034 | 465 | 528 | 493 | 472 | 470 | 471 |
| 100ms | 3521 | 1502 | 1703 | 1597 | 1593 | 1603 | 1579 |
| 25ms-5MiBps | 4907 | 5108 | 5177 | 5063 | 5063 | 5018 | 5065 |

At 100 ms, removing HEAD brings Range ×2 to 1703 ms (2053 ms in the earlier
three-request protocol). Separate probes reach 1579 ms with two data slots,
about 7% below the current coupled Range ×2, but above whole-order Bulk's 1502 ms.
Increasing the manifest window beyond two shows little consistent advantage in
this five-order history. With the first data response blocked, integration tests
separately prove that later manifests are fetched beyond the two occupied data slots.

Individual Timeline GETs remain fastest on the shared 5 MiB/s link: 4907 ms
versus 5018–5177 ms for the archive strategies. With a snapshot at
*Milestone*[~][milestone] 72, that link gives 1866 ms individual, 2303 ms Bulk ×2,
and 1903 ms separate probes ×8/data ×2. Small differences across two simulated
rounds should not be treated as stable rankings.

Keep Bulk ×2 as the default; expose `range-prefetch` for overlap/bandwidth tuning.
See *raw measurements*[~][manifest-prefetch].

For six staggered overlapping orders, the 100 ms times are 1615 ms whole-order
Bulk ×2, 1729 ms coupled Range ×4, and 1709 ms probes ×8/data ×2. On the shared
link these become 7103/5061/5054 ms: whole-order reads transfer 33.09 MiB versus
23.64 MiB for the ranged strategies. Individual Timeline GETs take 4874 ms at
23.04 MiB. A speculative initial Timeline GET can overlap the first archive range,
explaining some remaining duplicate traffic even in the ranged modes.
See *overlapping-order samples*[~][manifest-overlap].

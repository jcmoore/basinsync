[measurements]: <measurements.json>

# Background creation measurements

Apple M2, 24 GiB RAM, macOS, release build, VersityGW 1.11.0 POSIX backend.
Add 25 ms to each S3 request. Four concurrent clients each commit 32 markers of
64 KiB; batches permit 32 requests with a 2 ms wait. Rotate the three configurations
over three rounds. Time acknowledgments; exclude startup and final archive drain.

| Archive target | Requests/s | p50 ms | p95 ms | Archive PUTs per round |
| --- | ---: | ---: | ---: | --- |
| Disabled | 21.0 | 190.8 | 198.0 | 0, 0, 0 |
| 5 MiB | 22.1 | 183.3 | 200.3 | 2, 2, 2 |
| 512 KiB | 22.3 | 176.8 | 193.0 | 16, 16, 17 |

Rates and latencies are medians across rounds. Variation exceeds the differences:
there is no demonstrated foreground throughput gain or penalty. All archive-enabled
runs made zero source-event GETs: the bounded handoff supplied their bytes. Holding
an archive PUT in integration tests separately verifies overflow catch-up from storage.
The 5 MiB/30-second defaults balance request count and freshness; deployments should
measure their own workload. See *raw measurements*[~][measurements].

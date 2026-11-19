[metric]: <glossary/metric.md>
[tracing]: <glossary/tracing.md>

# Observability

Use `RUST_LOG` to filter *tracing*[~][tracing] output; the default is
`basinsync=info`. `BASINSYNC_LOG_FORMAT=text|json` selects the representation,
defaulting to `text`. API responses include a generated `x-request-id`.
Correlate it with `request_id`, `batch_id`, and the recorded positions.

`GET /metrics` on the server listener exposes process-local *Prometheus
metrics*[~][metric]. Counters reset at restart; durable queue counts are
resampled from recovered state, approximately once per second.

| Metric | Meaning |
| --- | --- |
| `basinsync_http_total`, `basinsync_http_seconds` | HTTP outcomes and handler latency, by fixed operation and status |
| `basinsync_stage_seconds`, `basinsync_stage_total` | Processing-stage latency and completion outcomes |
| `basinsync_http_inflight`, `basinsync_stage_inflight` | Active handlers and stages |
| `basinsync_requests_queued`, `basinsync_submissions_inflight` | Requests awaiting preparation or a durable result |
| `basinsync_submissions_total` | `ok`, `rejected`, `failed`, or `abandoned` durable-result handling |
| `basinsync_batches_queued`, `basinsync_batches_publishing`, `basinsync_batches_total` | Prepared-batch pressure and outcomes |
| `basinsync_background_pending`, `basinsync_background_oldest_age_seconds` | Durable sweeping, marking, freed-node, and promotion queues by space |
| `basinsync_background_sample_timestamp_seconds` | Freshness of the last successful queue sample |
| `basinsync_s3_copy_total` | COPY attempts by HTTP status; destination verification determines promotion success |
| `basinsync_analogue_buffer_bytes` | Archive buffering |
| `basinsync_last_success_timestamp_seconds` | Last local Timeline, backup, or archive publication by operation |

Histograms expose `_bucket`, `_sum`, and `_count`. Queue wait runs from submission
to preparation; acknowledgement latency runs from submission to the durable
result, excluding body parsing. HTTP latency includes the handler's earlier work.
HTTP cancellation and durable completion are independent: disconnected clients'
accepted work can still succeed. Dropped observations record abandonment.

---
prerequisites:
  - 0zg7/261118-shallow-datashed-backup-nodes
---

[access-token]: <../../../glossary/access-token.md>
[datashed]: <../../../glossary/datashed.md>
[development]: <../../../development.md>
[flow]: <../../../glossary/flow.md>
[metric]: <../../../glossary/metric.md>
[milestone]: <../../../glossary/milestone.md>
[observability]: <../../../observability.md>
[timeline]: <../../../glossary/timeline.md>
[tracing]: <../../../glossary/tracing.md>
[viewpoint]: <../../../glossary/viewpoint.md>

# Observability

Build on *tracing*[~][tracing]. Assign request ids and batch ids; link every
request to its batch, carrying the request span through queues and blocking
preparation. Retain that context until durable completion, even after HTTP
cancellation. Record *Flow*[~][flow], *Viewpoint*[~][viewpoint], and
*Milestone*[~][milestone] as trace fields, with completion outcomes.

Measure admission wait, queue wait, preparation, S3 publication, and durable
acknowledgement separately. Add *metrics*[~][metric] for in-flight work,
failures, recovery, backups, cleanup, promotion, and archive publication.
Sample durable queue counts and oldest ages from pinned database views outside
the request handler. Scraping must require neither SQL nor S3 and must not
advance the *Datashed*[~][datashed] *Timeline*[~][timeline].

Keep *metric labels*[~][metric] bounded. Exclude *access tokens*[~][access-token],
credentials, signed URLs, payloads, and client context values from telemetry.
Default to JSON logs, retain optional text logging, and offer Prometheus exposition through the
*operational interface*[~][observability].

Gate publication to prove successful acknowledgements are counted only after
durability. Test shared-batch correlation, disconnected clients, failed PUTs,
restart, pending-work age, and sensitive-data exclusion. Run the
*development checks*[~][development].

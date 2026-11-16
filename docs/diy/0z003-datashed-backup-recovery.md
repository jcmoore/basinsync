---
prerequisites:
  - 0z002-turso-reservoir-vfs.md
---

[0004-backup-overhead]: <../benchmarks/0004-backup-overhead.md>
[0z000-shallow-epilogue-commits]: <0z000-shallow-epilogue-commits.md>
[0z002-turso-reservoir-vfs]: <0z002-turso-reservoir-vfs.md>
[commit-marker]: <../glossary/commit-marker.md>
[conditional-write]: <../glossary/conditional-write.md>
[datashed]: <../glossary/datashed.md>
[datashed-backup]: <../datashed-backup.md>
[datashed-timeline]: <../datashed-timeline.md>
[development]: <../development.md>
[differential-event]: <../glossary/differential-event.md>
[envbuf]: <../glossary/envbuf.md>
[epilogue]: <../glossary/epilogue.md>
[epilogue-compaction]: <../glossary/epilogue-compaction.md>
[flow]: <../glossary/flow.md>
[jet]: <../glossary/jet.md>
[milestone]: <../glossary/milestone.md>
[prologue]: <../glossary/prologue.md>
[prologue-vacuuming]: <../glossary/prologue-vacuuming.md>
[range-get]: <../glossary/range-get.md>
[reservoir]: <../glossary/reservoir.md>
[sha384]: <../glossary/sha384.md>
[timeline]: <../glossary/timeline.md>
[timeline-eviction]: <../glossary/timeline-eviction.md>
[viewpoint]: <../glossary/viewpoint.md>
[virtual-file-system]: <../glossary/virtual-file-system.md>

# Datashed backup and recovery

*Datasheds*[^][datashed] and *Flows*[^][flow] are *Reservoirs*[^][reservoir], each with
an *Epilogue*[^][epilogue], *Timeline (Chronologue)*[^][timeline], and
*Prologue*[^][prologue]. Give the *Datashed*[^][datashed] its own
*Epilogue*[^][epilogue]. Revisit *Prologues*[^][prologue] and the third
*Reservoir*[^][reservoir] type, the *Jet*[^][jet], later.

Promote *Turso Reservoir VFS's*[^][0z002-turso-reservoir-vfs] file-backed copy-on-write
*VFS*[^][virtual-file-system] into the server. Pin a consistent, checkpointed
*Milestone*[^][milestone] through backup publication while request preparation
continues. Use the entire `default.turso` database file as the raw
*EnvBuf message*[^][envbuf] of the *Datashed*[^][datashed]
*CommitMarker*[^][commit-marker]; follow the
*key and metadata contract*[^][datashed-backup].

A separate worker first copies every *Flow*[^][flow] *CommitMarker*[^][commit-marker]
through that durable *Milestone*[^][milestone] into its
*Epilogue*[^][epilogue], as in *Shallow epilogue commits*[^][0z000-shallow-epilogue-commits]. Only then
publish the pinned backup at the next *Datashed*[^][datashed]
*Viewpoint*[^][viewpoint] using a single streamed *conditional PUT*[^][conditional-write]
with the complete enveloped object's content length. The raw database remains
one *message*[^][envbuf], bounded by its `u32` message-length field.
Keep one active backup and coalesce pending targets. Bound independent
*Flow*[^][flow] copies to eight concurrent workers, preserving each
*Flow*[^][flow] sequence and completing each *Milestone*[^][milestone] before
copying the next. Client success still waits only for durable *Timeline*[^][timeline]
publication. For status `latest`, HEAD its known *Flow*[^][flow]
*Epilogue*[^][epilogue] key and verify length and *digest*[^][sha384] before using its
message-range URL; otherwise retain the *Timeline*[^][timeline]
*Range GET*[^][range-get]. Test URL selection and copied contents directly.

Before writing the *Timeline*[^][timeline], read the latest
*Datashed*[^][datashed] *Viewpoint*[^][viewpoint] and corresponding
*Milestone*[^][milestone]. Load its snapshot when needed and successfully ingest
all subsequent *DifferentialEvents*[^][differential-event]. Use that
*Milestone*[^][milestone] as the exponential-search seed. With no backup, recover
from the beginning. Preserve ordered publication across the two-batch pipeline.
For status, honor the *probe freshness window*[^][datashed-timeline]; when due,
probe the next *Milestone*[^][milestone] first. Discover a newer
*Datashed*[^][datashed] backup and refresh only when that successor exists.

Test pinned-image equality against full frame replay, snapshot-plus-suffix
recovery, large and delayed uploads, concurrent backup workers, interrupted copies, lost
replies, and corrupt backups. Run the *development checks*[^][development] and
compare *background backup overhead*[^][0004-backup-overhead].

`BASINSYNC_DATA_DIR` defaults to `.local/state`; each writer uses its own
reconstructed on-disk cache. `BASINSYNC_BACKUP_INTERVAL_MS` defaults to 1000;
zero disables background backups for comparisons. Export with bounded buffers
and stream uploads. *Epilogue compaction*[^][epilogue-compaction],
*Timeline eviction*[^][timeline-eviction], and
*Prologue vacuuming*[^][prologue-vacuuming] remain for later tutorials.

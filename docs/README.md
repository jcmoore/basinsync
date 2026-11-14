[0001-shallow-epilogue-commits]: <diy/0001-shallow-epilogue-commits.md>
[0002-datashed-timeline-race]: <diy/0002-datashed-timeline-race.md>
[0003-turso-reservoir-vfs]: <diy/0003-turso-reservoir-vfs.md>
[0004-datashed-backup-recovery]: <diy/0004-datashed-backup-recovery.md>
[0005-shallow-userheap-management]: <diy/0005-shallow-userheap-management.md>
[0006-async-shallow-cleanup]: <diy/0006-async-shallow-cleanup.md>
[0007-deep-userheap-support]: <diy/0007-deep-userheap-support.md>
[0008-commit-marker-edges-with-node-promotion]: <diy/0008-commit-marker-edges-with-node-promotion.md>

# basinsync

A Rust server for assembling, retiring, and garbage collecting immutable,
first-writer-wins directed acyclic graphs of data nodes backed by S3-compatible
object storage.

Build it incrementally, recording milestones as concise tutorials with linked
reference and glossary entries. Begin with
*0001 — Shallow epilogue commits*[^][0001-shallow-epilogue-commits].

Coordinate writes and locate payloads with
*0002 — Datashed Timeline race*[^][0002-datashed-timeline-race].

Then materialize *0003 — Turso Reservoir VFS*[^][0003-turso-reservoir-vfs].

Recover from snapshots with *0004 — Datashed backup and recovery*[^][0004-datashed-backup-recovery].

Stage uploads with *0005 — Shallow UserHeap management*[^][0005-shallow-userheap-management].


Reclaim staged objects with *0006 — Async shallow cleanup*[^][0006-async-shallow-cleanup].

Build deeper graphs with *0007 — Deep UserHeap support*[^][0007-deep-userheap-support].

Retain and promote committed graphs with
*0008 — CommitMarker edges with node promotion*[^][0008-commit-marker-edges-with-node-promotion].

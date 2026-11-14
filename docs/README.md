[0z000-shallow-epilogue-commits]: <diy/0z000-shallow-epilogue-commits.md>
[0z001-datashed-timeline-race]: <diy/0z001-datashed-timeline-race.md>
[0z002-turso-reservoir-vfs]: <diy/0z002-turso-reservoir-vfs.md>
[0z003-datashed-backup-recovery]: <diy/0z003-datashed-backup-recovery.md>
[0z003-shallow-userheap-management]: <diy/0z003-shallow-userheap-management.md>
[0z004-async-shallow-cleanup]: <diy/0z004-async-shallow-cleanup.md>
[0z005-deep-userheap-support]: <diy/0z005-deep-userheap-support.md>
[0z006-commit-marker-edges-with-node-promotion]: <diy/0z006-commit-marker-edges-with-node-promotion.md>
[lexidecimal]: <glossary/lexidecimal.md>

# basinsync

A Rust server for assembling, retiring, and garbage collecting immutable,
first-writer-wins directed acyclic graphs of data nodes backed by S3-compatible
object storage.

Build it incrementally, recording milestones as concise tutorials with linked
reference and glossary entries. Filename prefixes encode dependency depth in
*Lexidecimal*[^][lexidecimal]; frontmatter lists prerequisites. Equal prefixes
identify parallel branches, not a reading sequence. Begin with
*Shallow epilogue commits*[^][0z000-shallow-epilogue-commits].

Coordinate writes and locate payloads with
*Datashed Timeline race*[^][0z001-datashed-timeline-race].

Then materialize *Turso Reservoir VFS*[^][0z002-turso-reservoir-vfs].

Recover from snapshots with *Datashed backup and recovery*[^][0z003-datashed-backup-recovery].

Independently of backup support, stage uploads with *Shallow UserHeap management*[^][0z003-shallow-userheap-management].

Reclaim staged objects with *Async shallow cleanup*[^][0z004-async-shallow-cleanup].

Build deeper graphs with *Deep UserHeap support*[^][0z005-deep-userheap-support].

Retain and promote committed graphs with
*CommitMarker edges with node promotion*[^][0z006-commit-marker-edges-with-node-promotion].

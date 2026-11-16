[async-shallow-cleanup]: <diy/0z004/async-shallow-cleanup.md>
[commit-marker-edges-with-node-promotion]: <diy/0z006/commit-marker-edges-with-node-promotion.md>
[datashed-backup-recovery]: <diy/0z003/datashed-backup-recovery.md>
[datashed-timeline-race]: <diy/0z001/datashed-timeline-race.md>
[deep-userheap-support]: <diy/0z005/deep-userheap-support.md>
[lexidecimal]: <glossary/lexidecimal.md>
[object-keys]: <object-keys.md>
[shallow-epilogue-commits]: <diy/0z000/shallow-epilogue-commits.md>
[shallow-userheap-management]: <diy/0z003/shallow-userheap-management.md>
[turso-reservoir-vfs]: <diy/0z002/turso-reservoir-vfs.md>

# basinsync

A Rust server for assembling, retiring, and garbage collecting immutable,
first-writer-wins directed acyclic graphs of data nodes backed by S3-compatible
object storage.

Build it incrementally, recording milestones as concise tutorials with linked
reference and glossary entries. Folder names encode dependency height in
*Lexidecimal*[^][lexidecimal]; frontmatter lists prerequisite paths relative to `diy/`.
Tutorials in the same folder are parallel branches, not a reading sequence. Begin with
*Shallow epilogue commits*[^][shallow-epilogue-commits].

Coordinate writes and locate payloads with
*Datashed Timeline race*[^][datashed-timeline-race].

Then materialize *Turso Reservoir VFS*[^][turso-reservoir-vfs].

Recover from snapshots with *Datashed backup and recovery*[^][datashed-backup-recovery].

Independently of backup support, stage uploads with *Shallow UserHeap management*[^][shallow-userheap-management].

Reclaim staged objects with *Async shallow cleanup*[^][async-shallow-cleanup].

Build deeper graphs with *Deep UserHeap support*[^][deep-userheap-support].

Retain and promote committed graphs with
*CommitMarker edges with node promotion*[^][commit-marker-edges-with-node-promotion].

Audit storage layouts in the *object-key reference*[^][object-keys].

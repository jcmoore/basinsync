[diy/0z000/shallow-epilogue-commits]: <diy/0z000/shallow-epilogue-commits/README.md>
[diy/0z001/datashed-timeline-race]: <diy/0z001/datashed-timeline-race/README.md>
[diy/0z002/turso-reservoir-vfs]: <diy/0z002/turso-reservoir-vfs/README.md>
[diy/0z003/datashed-backup-recovery]: <diy/0z003/datashed-backup-recovery/README.md>
[diy/0z003/shallow-heap-management]: <diy/0z003/shallow-heap-management/README.md>
[diy/0z004/async-shallow-cleanup]: <diy/0z004/async-shallow-cleanup/README.md>
[diy/0z004/timeline-recovery-acceleration]: <diy/0z004/timeline-recovery-acceleration/README.md>
[diy/0z005/deep-heap-support]: <diy/0z005/deep-heap-support/README.md>
[diy/0z006/commit-marker-edges-with-node-promotion]: <diy/0z006/commit-marker-edges-with-node-promotion/README.md>
[diy/0z007/shallow-datashed-backup-nodes]: <diy/0z007/shallow-datashed-backup-nodes/README.md>
[diy/0z008/observability]: <diy/0z008/observability/README.md>
[lexidecimal]: <glossary/lexidecimal.md>
[object-keys]: <object-keys.md>

# basinsync

A Rust server for assembling, retiring, and garbage collecting immutable,
first-writer-wins directed acyclic graphs of data nodes backed by S3-compatible
object storage.

Build it incrementally, recording milestones as concise tutorials with linked
reference and glossary entries. Height folders encode dependency height in
*Lexidecimal*[~][lexidecimal]; frontmatter lists prerequisite folders relative to `diy/`.
Tutorial folders may nest by repeating height/name pairs; tutorial slugs must be
unique throughout `diy/`. Tutorials at the same height are parallel branches, not a reading sequence. Begin with
*Shallow epilogue commits*[~][diy/0z000/shallow-epilogue-commits].

Coordinate writes and locate payloads with
*Datashed Timeline race*[~][diy/0z001/datashed-timeline-race].

Then materialize *Turso Reservoir VFS*[~][diy/0z002/turso-reservoir-vfs].

Recover from snapshots with *Datashed backup and recovery*[~][diy/0z003/datashed-backup-recovery].

Independently of backup support, stage uploads with *Shallow Heap management*[~][diy/0z003/shallow-heap-management].

Reclaim staged objects with *Async shallow cleanup*[~][diy/0z004/async-shallow-cleanup].

Build deeper graphs with *Deep Heap support*[~][diy/0z005/deep-heap-support].

Retain and promote committed graphs with
*CommitMarker edges with node promotion*[~][diy/0z006/commit-marker-edges-with-node-promotion].

Coalesce and retrieve history with *Timeline recovery acceleration*[~][diy/0z004/timeline-recovery-acceleration].

Back up paired database graphs with
*Shallow Datashed backup nodes*[~][diy/0z007/shallow-datashed-backup-nodes].

Inspect request publication and background progress with
*Observability*[~][diy/0z008/observability].

Audit storage layouts in the *object-key reference*[~][object-keys].

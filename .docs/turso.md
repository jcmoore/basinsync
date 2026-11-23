[durable-storage]: <../docs/glossary/durable-storage.md>
[logical-frame]: <../docs/glossary/logical-frame.md>
[physical-frame]: <../docs/glossary/physical-frame.md>
[turso]: <../docs/glossary/turso.md>
[turso-state]: <../docs/turso-state.md>
[virtual-file-system]: <../docs/glossary/virtual-file-system.md>
[wal]: <../docs/glossary/wal.md>

# Turso materialization

The documentation pins *Turso*[~][turso] and its configuration in the
*delta reference*[~][turso-state]. This repository vendors that revision as the
`vendor/turso` submodule; clean rooms get a read-only copy of the same source.
`turso_core` 0.9.0-pre.6 on crates.io breaks five of the APIs used here.

- Implement *DurableStorage*[~][durable-storage] by delegating to the engine and
  observing `log_tx`'s serialization callback for
  *logical frames*[~][logical-frame].
- Disable automatic checkpointing; intercept *physical WAL*[~][wal] writes
  before each explicit checkpoint truncates them, and publish them as
  `sqlite-wal/1` segments of *page frames*[~][physical-frame].
- Pin snapshots with a file-backed copy-on-write
  *VFS*[~][virtual-file-system]: pin a checkpointed generation before the next
  transaction, retain overwritten pages on disk, and seal the next generation
  after its checkpoint.

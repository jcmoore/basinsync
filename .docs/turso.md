[durable-storage]: <../docs/glossary/durable-storage.md>
[logical-frame]: <../docs/glossary/logical-frame.md>
[physical-frame]: <../docs/glossary/physical-frame.md>
[turso]: <../docs/glossary/turso.md>
[turso-state]: <../docs/turso-state.md>
[virtual-file-system]: <../docs/glossary/virtual-file-system.md>
[wal]: <../docs/glossary/wal.md>

# Turso materialization

This repository materializes the Basin in *Turso*[~][turso]. The documentation
treats the engine as an implementation choice and specifies only the tables
and the *delta contract*[~][turso-state]; these are this implementation's
engine notes.

- Pin the vendored build: `0.9.0-pre.1+032f5bb2c7e74107f7ef9b66d576312379000a88`,
  which deltas carry as their `version`. Use crate features `fs` and
  `conn_raw_api`.
- Open databases with `PRAGMA page_size = 16384` and
  `PRAGMA journal_mode = mvcc`, and call
  `connection.set_portable_logical_changes_enabled(true)`.
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

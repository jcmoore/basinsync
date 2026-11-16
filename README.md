[diy/0z000/shallow-epilogue-commits]: <docs/diy/0z000/shallow-epilogue-commits/README.md>
[diy/0z006/commit-marker-edges-with-node-promotion]: <docs/diy/0z006/commit-marker-edges-with-node-promotion/README.md>

# basinsync

A Rust server growing toward immutable data DAGs backed by S3-compatible object
storage. Start with *Shallow epilogue commits*[~][diy/0z000/shallow-epilogue-commits]:
a first-writer-wins Epilogue, a server, and TypeScript stress clients.

[Run and verify](docs/development.md) · [Protocol and invariant](docs/epilogue-protocol.md)

The current milestone is *CommitMarker edges with node promotion*[~][diy/0z006/commit-marker-edges-with-node-promotion]:
authenticated marker edges, bounded depth-first marking, and restartable promotion
of retained graphs from young to old storage outside the request path, with
PromotionSigns certifying completion.

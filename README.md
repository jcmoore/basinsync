[diy/0z000/shallow-epilogue-commits]: <docs/diy/0z000/shallow-epilogue-commits/README.md>
[diy/0z008/observability]: <docs/diy/0z008/observability/README.md>

# basinsync

A Rust server growing toward immutable data DAGs backed by S3-compatible object
storage. Start with *Shallow epilogue commits*[~][diy/0z000/shallow-epilogue-commits]:
a first-writer-wins Epilogue, a server, and TypeScript stress clients.

[Run and verify](docs/development.md) · [Protocol and invariant](docs/epilogue-protocol.md)

The current milestone is *Observability*[~][diy/0z008/observability]:
correlated request and batch traces, durable-publication latency, and Prometheus
metrics for foreground pressure and background progress.

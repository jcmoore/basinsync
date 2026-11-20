[diy/0zg0/261022-shallow-epilogue-commits]: <docs/diy/0zg0/261022-shallow-epilogue-commits/README.md>
[diy/0zg8/261119-observability]: <docs/diy/0zg8/261119-observability/README.md>

# basinsync

A Rust server growing toward immutable data DAGs backed by S3-compatible object
storage. Start with *Shallow epilogue commits*[~][diy/0zg0/261022-shallow-epilogue-commits]:
a first-writer-wins Epilogue, a server, and TypeScript stress clients.

[Run and verify](docs/development.md) · [Protocol and invariant](docs/epilogue-protocol.md)

[Prompt requests](docs/prompt-requests.md) describes the experimental workflow for
proposing changes that maintainers implement with their own coding agents.

The current milestone is *Observability*[~][diy/0zg8/261119-observability]:
correlated request and batch traces, durable-publication latency, and Prometheus
metrics for foreground pressure and background progress.

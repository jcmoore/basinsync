#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
node docs/skills/basinsync-tutorial-dag/scripts/check-dag.mjs
node --test docs/skills/basinsync-tutorial-dag/scripts/check-dag.test.mjs
mkdir -p .local/bin
CGO_ENABLED=0 GOBIN="$PWD/.local/bin" go install github.com/versity/versitygw/cmd/versitygw@v1.11.0
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --locked --examples
cargo build --locked
bun install --frozen-lockfile
bun run check
bun run test
bun run test:integration

[development]: <development.md>
[docs]: <../docs/README.md>
[srt]: <https://github.com/anthropic-experimental/sandbox-runtime>

# Clean-room rebuilds

A clean-room rebuild gives an agent the *documentation*[~][docs] alone and asks
it to rebuild basinsync: Quint specifications for the protocols, a Rust core,
`quint-connect` model-based tests, and Kani proofs for the codecs. Its gap log
measures how complete and correct `docs/` is; comparing its build with this
repository's measures the rest. Rebuilds are meant to be repeated, by different
agents and against later documentation.

```sh
scripts/cleanroom/setup.sh [--ref REF] NAME     # docs/ + brief at REF, fresh history
scripts/cleanroom/run.sh --check NAME           # probe the sandbox only
CLAUDE_CODE_OAUTH_TOKEN=... scripts/cleanroom/run.sh NAME
scripts/cleanroom/run.sh --continue NAME        # resume an unfinished claude run
scripts/cleanroom/run.sh --agent codex NAME     # with OPENAI_API_KEY
scripts/cleanroom/run.sh --command 'CMD' NAME   # any other agent
scripts/cleanroom/collect.sh NAME               # fetch as orphan branch cleanroom/NAME
```

Runs live in `~/cleanroom/NAME` (override with `CLEANROOM_ROOT`). The brief is
`scripts/cleanroom/brief.md`, copied into each workspace as `AGENTS.md`, with a
`CLAUDE.md` that points to it.

## Isolation

`run.sh` wraps the agent in *srt*[~][srt] (bubblewrap on Linux, Seatbelt on
macOS) and refuses to start unless a probe confirms the sandbox:

- reads: the workspace, the agent's own config home, its TMPDIR and caches,
  and read-only toolchains; all of `/home`, `/root`, `/run/user`, and this
  user's agent scratch space under `/tmp` are hidden;
- writes: the workspace, the agent home, caches, and the Quint cache;
- network: the agent's API, crates.io, npm, the Go module proxy, and tool
  documentation; GitHub is unreachable, since basinsync's repositories are
  public;
- environment: cleared, then a curated set exported, so no host variables or
  git identity leak in.

Never run a clean room as an in-process subagent: the agent's file tools would
bypass a sandbox around its shell. Never copy your own agent login into the
sandbox; pass a dedicated token (`claude setup-token`) or API key instead.
On Ubuntu, AppArmor refuses srt's nested user namespace for Unix-socket
filtering, so runs allow Unix sockets and hide `/run/user` instead.

Prerequisites beyond the *development tools*[~][development]: `srt`,
`bubblewrap`, `socat`, `ripgrep`, Quint (with Apalache cached in `~/.quint`
by one `quint verify` outside the sandbox), Java, and Kani.

## Oracles

Agents never see these; apply them to a collected branch:

1. its `GAPS.md`, reviewed against `docs/`;
2. the codec vectors in `tests/vectors/`;
3. the TypeScript integration suites, pointed at the clean-room server;
4. a `quint-connect` driver for this implementation, replaying the clean-room
   specifications' traces.

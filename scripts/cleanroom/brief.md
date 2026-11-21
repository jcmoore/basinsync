# Clean-room rebuild of basinsync

This repository holds only the basinsync documentation in `docs/`. Rebuild
basinsync from that documentation alone, so we can learn whether it is complete
and correct. Start with `docs/README.md`.

You work unattended: nobody will answer questions. When the documentation is
ambiguous, silent, or contradictory, make the most reasonable choice, record it
as a gap, and keep going.

## Rules

- `docs/` is the only source of requirements. Do not look for, fetch, or read
  any other basinsync code, repository, issue, or discussion; that would
  invalidate the experiment. Package registries and documentation for your
  tools (Rust, Quint, Kani, S3) are fine.
- Do not edit `docs/`. Everything you write lives outside it.
- Your build will later be checked against independent tests you cannot see.
  Follow the documentation exactly; do not invent behavior to make things
  "work" without logging the gap that forced it.
- Commit to git often with clear messages. There is no remote; do not add one.

## Deliverables

1. **`GAPS.md`, the primary deliverable.** One entry per gap: an id, the
   documentation file and line(s), the kind (omission, ambiguity,
   contradiction, or error), what is missing or unclear, the assumption you
   made, and what it affects. Also note documentation you found sufficient
   for tricky parts. Keep it current as you work.
2. **Quint specifications** in `specs/`, written from the documentation:
   - Epilogue commits with conditional, first-writer-wins writes;
   - the Basin Timeline: racing writers, ordering, and recovery;
   - the Heap: reservations, staging, sweeping the young generation after
     CommitMarkers publish, and reclamation safety (nothing deleted while
     still retained).
   State invariants and witnesses (reachability checks). Check them with
   `quint typecheck`, `quint run --invariant=...`, and, where the state space
   allows, `quint verify` (Apalache, bounded).
3. **A Rust implementation** of the core in a Cargo workspace: the codecs the
   documentation defines (Lexidecimal, Diffuse16, EnvBuf, FlexBuffers
   extensions, JSON0, CRC32C and SHA-384 handling), Epilogue commits, the
   Basin Timeline, and the Heap, with the documented HTTP API and S3 object
   layout, runnable against an S3-compatible endpoint such as VersityGW as
   `docs/development.md` describes. Prefer finishing these well over covering
   later tutorials; list what you skipped.
4. **Model-based tests** with the `quint-connect` crate, replaying Quint traces
   against the Rust core and comparing state at every step.
5. **Kani proofs** for the codecs (`cargo kani`): no panics on arbitrary input
   up to a bound; round-trips (decode of encode returns the value, and encode
   of a decoded canonical string returns that string); rejection of
   noncanonical encodings. Pick bounds Kani can finish; record them.
6. **`README.md`** explaining how to build, run, and test everything, and what
   is incomplete.

## Status and blockers

Keep `STATUS.md` current and commit it with your work: the phase you are in,
what is done, what is next, and anything blocking you. Start each blocking
problem's line with `BLOCKED:` and say what you tried; the people running the
rebuild watch for those lines. If you cannot make further progress, write the
blockers, commit, and stop rather than looping.

## Resources

Everything you start shares one memory cap, `$CLEANROOM_MEMORY_MAX`; exceeding
it kills the largest process. Model checkers can consume all of it:

- run one Kani or Apalache job at a time, never several in parallel or in
  background helpers;
- wrap each in `timeout` (for example `timeout 900 cargo kani --harness NAME`),
  start with small bounds (`--default-unwind`, `--max-steps`), and grow them
  only while runs stay well under the cap;
- treat an out-of-memory kill or timeout as a reason to shrink the bound or
  split the harness, and record the bound you settled on.

## Environment

Rust stable with Cargo, Kani (`cargo kani`), the Quint CLI (`quint`, with
Apalache cached), Java, Node.js, Bun, and Go are on `PATH`. You run inside a
sandbox: you can read and write this repository and your tool caches, nothing
else on the machine. Network access is limited to the model API, package
registries (crates.io, npm, the Go module proxy), and tool documentation. If
something you need is blocked, record it in `GAPS.md` and work around it.

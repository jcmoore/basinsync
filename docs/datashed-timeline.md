[causal-consistency]: <glossary/causal-consistency.md>
[commit-marker]: <glossary/commit-marker.md>
[conditional-read]: <glossary/conditional-read.md>
[conditional-write]: <glossary/conditional-write.md>
[context]: <glossary/context.md>
[datashed]: <glossary/datashed.md>
[datashed-backup]: <datashed-backup.md>
[differential-event]: <glossary/differential-event.md>
[empty-sentinel]: <glossary/empty-sentinel.md>
[envbuf]: <glossary/envbuf.md>
[epilogue-protocol]: <epilogue-protocol.md>
[flexbuffers]: <glossary/flexbuffers.md>
[flow]: <glossary/flow.md>
[lexidecimal]: <glossary/lexidecimal.md>
[margin]: <glossary/margin.md>
[milestone]: <glossary/milestone.md>
[node]: <glossary/node.md>
[object-keys]: <object-keys.md>
[physical-frame]: <glossary/physical-frame.md>
[range-manifest]: <glossary/range-manifest.md>
[sha384]: <glossary/sha384.md>
[timeline]: <glossary/timeline.md>
[tombstone]: <glossary/tombstone.md>
[turso-state]: <turso-state.md>
[viewpoint]: <glossary/viewpoint.md>

# Datashed Timeline

The *Datashed's*[~][datashed] shared *Timeline*[~][timeline] records state changes using the
*Timeline object-key pattern*[~][object-keys]. *Milestones*[~][milestone] start at one and increase
without gaps. Before writing, read the latest *Datashed*[~][datashed] *recovery anchor*[~][datashed-backup]
and ingest every subsequent *DifferentialEvent*[~][differential-event]. Its
recorded *Milestone*[~][milestone] seeds exponential search for the tail.
Encode complemented *Milestones*[~][milestone] as *Lexidecimal*[~][lexidecimal],
so newer entries sort first. Without a *CommitMarker*[~][commit-marker], LIST the
lowest published key, skipping zero-byte *sentinels*[~][empty-sentinel], and
replay from the beginning through that *Milestone*[~][milestone].
Treat trailing *empty sentinels*[~][empty-sentinel] as exclusive upper search
bounds. Leading *tombstones*[~][tombstone] occupy retired positions, not replayable
entries; recovery needs a snapshot covering them. An empty *Timeline*[~][timeline] expects *Milestone*[~][milestone] one.
Use *conditional reads*[~][conditional-read] for GETs and HEADs, including search probes.

Commitflow tuples begin with `Symbol.species`, using the
*FlexBuffers*[~][flexbuffers] extension encoding, and carry a `markers` map
keyed by *Flow*[~][flow] id.
Each entry contains decimal `viewpoint`, hexadecimal *SHA-384*[~][sha384] in
`sha384`, binary `blob`, *context*[~][context] as `Record<string, string>`, and required `edges` (`[]` for an empty graph) containing derived child *node*[~][node] ids. *Conditional creation*[~][conditional-write] of a
*DifferentialEvent*[~][differential-event] at the next *Milestone*[~][milestone]
arbitrates competing writers. Concatenate zero or more *EnvBuf-wrapped*[~][envbuf]
commitflow tuples, one `sts/0/databasin/ingestturso` tuple, then a
*range manifest*[~][range-manifest]. Wrap that command feed in one nested
*EnvBuf*[~][envbuf] (`100`), then append a separate *counter margin*[~][margin]. Each tuple is its own
*FlexBuffers*[~][flexbuffers] document. The ingestion command's
*Turso deltas*[~][turso-state] carry the *physical frames*[~][physical-frame]
that materialize the recorded state; all requests wait for durable publication.

Regardless of in-flight limits, durable publication must follow
*Milestone*[~][milestone] order to preserve *causal consistency*[~][causal-consistency].
Complete one *conditional PUT*[~][conditional-write] before starting the next;
acknowledge a batch only after it and all predecessors are durable.
Predecessor *digests*[~][sha384] cover the complete event *EnvBuf*[~][envbuf],
including its header, message, alignment padding, and footer. Separate trailing
retry *margins*[~][margin] are outside that identity.

`PUT /api/v0/epilogue` adds a `flow` string to the
*commit details*[~][epilogue-protocol] and validates against that
*Flow's*[~][flow] expected *Viewpoint*[~][viewpoint].
`GET /api/v0/epilogue/status?flow=<id>` returns the *common status response*[~][epilogue-protocol].
The `flow` query parameter is an ordinary URL-encoded string.
Mismatches use the *common response*[~][epilogue-protocol].

`BASINSYNC_STATUS_PROBE_MAX_AGE_MS` defaults to 3000; zero probes on every status
request. Reuse a successful probe or refresh across *Flows*[~][flow] within that
window, without extending it on cache hits. Status still waits for pending local
uploads; writes always refresh independently of this allowance.

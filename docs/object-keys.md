[access-token]: <glossary/access-token.md>
[analogue]: <glossary/analogue.md>
[bulk-manifest]: <glossary/bulk-manifest.md>
[bulk-order]: <glossary/bulk-order.md>
[byte-coordinate]: <glossary/byte-coordinate.md>
[commit-marker]: <glossary/commit-marker.md>
[datashed]: <glossary/datashed.md>
[derived-subkey]: <glossary/derived-subkey.md>
[differential-event]: <glossary/differential-event.md>
[empty-sentinel]: <glossary/empty-sentinel.md>
[envbuf]: <glossary/envbuf.md>
[flow]: <glossary/flow.md>
[heap]: <glossary/heap.md>
[lexidecimal]: <glossary/lexidecimal.md>
[margin]: <glossary/margin.md>
[milestone]: <glossary/milestone.md>
[node]: <glossary/node.md>
[node-height]: <glossary/node-height.md>
[old-generation]: <glossary/old-generation.md>
[promotion-sign]: <glossary/promotion-sign.md>
[retainer]: <glossary/retainer.md>
[revision]: <glossary/revision.md>
[sha384]: <glossary/sha384.md>
[tombstone]: <glossary/tombstone.md>
[userspace]: <glossary/userspace.md>
[viewpoint]: <glossary/viewpoint.md>
[young-generation]: <glossary/young-generation.md>

# Object keys

All implemented S3 key patterns, relative to the configured bucket:

| Object | Key |
| --- | --- |
| Empty-validator reference | `sentinel/empty` |
| *Tombstone*[~][tombstone] probes | `sentinel/tombstone/${length}` |
| *Margin*[~][margin] probes | `sentinel/margin/${counter}` |
| *Datashed*[~][datashed] *DifferentialEvent*[~][differential-event] | `timeline/${lex(~milestone)}` |
| *Datashed*[~][datashed] *Analogue*[~][analogue] *BulkOrder*[~][bulk-order] | `analogue/${lex(~lowest)}/${lex(~highest)}/v0/${bulkmanifest_start}/${coordinate}` |
| *Datashed*[~][datashed] backup *CommitMarker*[~][commit-marker] | `epilogue/${lex(~viewpoint)}` |
| *Datashed*[~][datashed] *PromotionSign*[~][promotion-sign] | `epilogue/${lex(viewpoint)}` |
| *Flow*[~][flow] *CommitMarker*[~][commit-marker] | `flows/${flow}/${flow_revision}/epilogue/${lex(~viewpoint)}` |
| *Flow*[~][flow] *PromotionSign*[~][promotion-sign] | `flows/${flow}/${flow_revision}/epilogue/${lex(viewpoint)}` |
| *UserSpace*[~][userspace] *Heap*[~][heap] *young-generation*[~][young-generation] *node*[~][node] | `userspace/heap/young/${node_id}/${node_revision}` |
| *UserSpace*[~][userspace] *Heap*[~][heap] *old-generation*[~][old-generation] *node*[~][node] | `userspace/heap/old/${node_id}/${node_revision}` |

`lex` is signed *Lexidecimal*[~][lexidecimal]; `~` is arbitrary-precision bitwise
complement. *Viewpoints*[~][viewpoint]
start at zero; *Milestones*[~][milestone] start at one.
*Revisions*[~][revision] are lowercase UUIDv4 strings. A *Flow's*[~][flow]
revision is assigned at its first commit; each *node*[~][node] allocation gets a
new revision, shared across both generations. Object keys must fit S3's
1024-byte limit.

## Flow ids

Accept 1–160 ASCII letters, digits, `.`, `_`, or `-`, excluding `.` and `..`.
Lowercase letters before identifying the *Flow*[~][flow]. Use this canonical id
directly, without hexadecimal or *Lexidecimal*[~][lexidecimal] encoding.
Reject `/` and every other character outside that subset.

## Node ids

```text
n-${lex(~height)}/v.${lex(version)}/t.${lex(token.length)}/d_${sha384(token)}
```

`height` is the *node height*[~][node-height]; `version` is currently zero.
`token.length` counts *access-token*[~][access-token] bytes. The *derived
subkey*[~][derived-subkey] is lowercase hexadecimal *SHA-384*[~][sha384]; never
persist the token itself.

## Database retainers

These are *retainer*[~][retainer] identifiers, not additional S3 objects:

```text
_a_${flow}/v.${lex(version)}/e.${lex(viewpoint)}   # staging area
_m_${flow}/v.${lex(version)}/e.${lex(viewpoint)}   # CommitMarker
```

An outgoing *node*[~][node] edge uses the parent's `node_id` as its
*retainer*[~][retainer]. Require `retainer < node`; `_a_`/`_m_` precede `n-`,
and complemented *heights*[~][node-height] order parents before children.
Reserve `_r_` for future *retainers*[~][retainer].

## Object bodies

Sequence objects reserve zero bytes for *empty sentinels*[~][empty-sentinel].
*DifferentialEvents*[~][differential-event] each wrap a command feed in one nested
*EnvBuf*[~][envbuf] (`100`), followed by a separate *counter margin*[~][margin];
*CommitMarkers*[~][commit-marker], including database backups, wrap their blob in
one raw message. These feeds default to a trailing `u64` *counter margin*[~][margin].
*PromotionSigns*[~][promotion-sign] use a *margin*[~][margin] with a one-byte counter
from 1 through 7. Their framing keeps valid empty messages distinct from
*sentinels*[~][empty-sentinel]. *Heap*[~][heap] objects stay opaque, including valid empty blobs.
The empty reference is always zero bytes. Probe objects are created only if
absent and verified on reuse. *Tombstone*[~][tombstone] probe lengths are decimal
2, 4, …, 14; sixteen-byte candidates are refused. *Margin*[~][margin] probes each contain a single
24-byte *EnvBuf*[~][envbuf], with a one-byte message containing `counter` from 1 through 7.

*BulkOrder*[~][bulk-order] bounds are inclusive `lowest` and exclusive `highest`, with `lowest >= 1`.
*BulkOrders*[~][bulk-order] contain complete event envelopes, their own footer tuple,
and a trailing *counter margin*[~][margin].

`bulkmanifest_start` is the exclusive end of the event bytes and the starting
object offset of the *BulkManifest*[~][bulk-manifest]. `coordinate` is the events'
starting *Timeline byte coordinate*[~][byte-coordinate]; both are canonical decimal.
The manifest's representation and length do not enter the key.

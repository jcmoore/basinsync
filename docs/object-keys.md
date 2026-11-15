[access-token]: <glossary/access-token.md>
[commit-marker]: <glossary/commit-marker.md>
[datashed]: <glossary/datashed.md>
[derived-subkey]: <glossary/derived-subkey.md>
[differential-event]: <glossary/differential-event.md>
[diffuse16]: <glossary/diffuse16.md>
[flow]: <glossary/flow.md>
[lexidecimal]: <glossary/lexidecimal.md>
[milestone]: <glossary/milestone.md>
[node]: <glossary/node.md>
[node-height]: <glossary/node-height.md>
[old-generation]: <glossary/old-generation.md>
[promotion-sign]: <glossary/promotion-sign.md>
[retainer]: <glossary/retainer.md>
[revision]: <glossary/revision.md>
[sha384]: <glossary/sha384.md>
[viewpoint]: <glossary/viewpoint.md>
[young-generation]: <glossary/young-generation.md>

# Object keys

All implemented S3 key patterns, relative to the configured bucket:

| Object | Key |
| --- | --- |
| *Datashed*[^][datashed] *DifferentialEvent*[^][differential-event] | `timeline/${diffuse16(milestone)}` |
| *Datashed*[^][datashed] backup *CommitMarker*[^][commit-marker] | `epilogue/${lex(~viewpoint)}` |
| *Datashed*[^][datashed] *PromotionSign*[^][promotion-sign] | `epilogue/${lex(viewpoint)}` |
| *Flow*[^][flow] *CommitMarker*[^][commit-marker] | `flows/${flow}/${flow_revision}/epilogue/${lex(~viewpoint)}` |
| *Flow*[^][flow] *PromotionSign*[^][promotion-sign] | `flows/${flow}/${flow_revision}/epilogue/${lex(viewpoint)}` |
| *Young-generation*[^][young-generation] *node*[^][node] | `userheap/young/${node_id}/${node_revision}` |
| *Old-generation*[^][old-generation] *node*[^][node] | `userheap/old/${node_id}/${node_revision}` |

`lex` is signed *Lexidecimal*[^][lexidecimal]; `~` is arbitrary-precision bitwise
complement. `diffuse16` is *Diffuse16*[^][diffuse16]. *Viewpoints*[^][viewpoint]
start at zero; *Milestones*[^][milestone] start at one.
*Revisions*[^][revision] are lowercase UUIDv4 strings. A *Flow's*[^][flow]
revision is assigned at its first commit; each *node*[^][node] allocation gets a
new revision, shared across both generations. Object keys must fit S3's
1024-byte limit.

## Flow ids

Accept 1–160 ASCII letters, digits, `.`, `_`, or `-`, excluding `.` and `..`.
Lowercase letters before identifying the *Flow*[^][flow]. Use this canonical id
directly, without hexadecimal or *Lexidecimal*[^][lexidecimal] encoding.
Reject `/` and every other character outside that subset.

## Node ids

```text
n-${lex(~height)}/v.${lex(version)}/t.${lex(token.length)}/d_${sha384(token)}
```

`height` is the *node height*[^][node-height]; `version` is currently zero.
`token.length` counts *access-token*[^][access-token] bytes. The *derived
subkey*[^][derived-subkey] is lowercase hexadecimal *SHA-384*[^][sha384]; never
persist the token itself.

## Database retainers

These are *retainer*[^][retainer] identifiers, not additional S3 objects:

```text
_a_${flow}/v.${lex(version)}/e.${lex(viewpoint)}   # staging area
_m_${flow}/v.${lex(version)}/e.${lex(viewpoint)}   # CommitMarker
```

An outgoing *node*[^][node] edge uses the parent's `node_id` as its
*retainer*[^][retainer]. Require `retainer < node`; `_a_`/`_m_` precede `n-`,
and complemented *heights*[^][node-height] order parents before children.
Reserve `_r_` for future *retainers*[^][retainer].

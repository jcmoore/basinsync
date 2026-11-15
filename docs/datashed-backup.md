[commit-marker]: <glossary/commit-marker.md>
[conditional-write]: <glossary/conditional-write.md>
[context]: <glossary/context.md>
[datashed]: <glossary/datashed.md>
[epilogue]: <glossary/epilogue.md>
[epilogue-compaction]: <glossary/epilogue-compaction.md>
[flow]: <glossary/flow.md>
[lexidecimal]: <glossary/lexidecimal.md>
[milestone]: <glossary/milestone.md>
[object-keys]: <object-keys.md>
[prologue-vacuuming]: <glossary/prologue-vacuuming.md>
[promotion-sign]: <glossary/promotion-sign.md>
[reservoir]: <glossary/reservoir.md>
[sha384]: <glossary/sha384.md>
[timeline]: <glossary/timeline.md>
[timeline-eviction]: <glossary/timeline-eviction.md>
[turso]: <glossary/turso.md>
[viewpoint]: <glossary/viewpoint.md>

# Datashed backup and recovery

Keep *Reservoir*[^][reservoir] keyspaces separate using the shared
*object-key patterns*[^][object-keys].

Use *complemented Lexidecimal*[^][lexidecimal] positions for *markers*[^][commit-marker]
in both *Epilogues*[^][epilogue]; their *PromotionSigns*[^][promotion-sign] use
uncomplemented positions in those same namespaces. The full-file
*Datashed*[^][datashed] *marker's*[^][commit-marker] own graph has no outgoing edges;
confirm its sign after publication, repairing a missing sign on retry. *Datashed*[^][datashed] *Viewpoints*[^][viewpoint] count
backups independently of *Milestones*[^][milestone]. A backup's S3 metadata
binds its image to the recovery prefix:

```text
x-amz-meta-milestone: <canonical decimal>
x-amz-meta-event-sha384: <digest of DifferentialEvent at that Milestone>
x-amz-meta-sha384: <digest of CommitMarker bytes>
x-amz-meta-turso-version: <engine release and revision>
```

Digests are hexadecimal *SHA-384*[^][sha384]. Validate metadata, checksum,
*Turso*[^][turso] version, and the image's recorded *Milestone*[^][milestone].

Before publishing a *Datashed*[^][datashed] *CommitMarker*[^][commit-marker], replicate
all *Flow*[^][flow] *markers*[^][commit-marker] through its
*Milestone*[^][milestone] into their *Epilogues*[^][epilogue]. Use
*conditional creation*[^][conditional-write]; preserve *context*[^][context] metadata and verify existing bytes and context on retry.
Competing backup workers adopt the winning *Viewpoint*[^][viewpoint], keeping
backed-up *Milestones*[^][milestone] increasing.

Plan future *Epilogue compaction*[^][epilogue-compaction] around a contiguous
available prefix of stored *CommitMarker*[^][commit-marker]/*PromotionSign*[^][promotion-sign]
pairs. *Markers*[^][commit-marker] must have reached their *Epilogues*[^][epilogue]
from the *Datashed*[^][datashed] *Timeline*[^][timeline]; a recovery anchor alone
is insufficient. Compact up to, but not through, the retained *Viewpoint*[^][viewpoint].
Tombstone compacted *markers*[^][commit-marker] with zero-length objects to preserve
*conditional-write barriers*[^][conditional-write], and DELETE their
*PromotionSigns*[^][promotion-sign]. A sign LISTer checks the corresponding
*marker*[^][commit-marker] metadata, deleting and skipping stale signs on demand
before selecting the oldest available snapshot; see *compaction*[^][epilogue-compaction].

A stored *Datashed*[^][datashed] recovery anchor permits *Timeline eviction*[^][timeline-eviction]
through its *Milestone*[^][milestone] once the corresponding *Flow*[^][flow]
*markers*[^][commit-marker] are replicated. Serialize cleanup requests through the
*Datashed*[^][datashed] *Timeline*[^][timeline]. *Prologue vacuuming*[^][prologue-vacuuming]
is a separate process.

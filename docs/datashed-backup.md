[commit-marker]: <glossary/commit-marker.md>
[conditional-write]: <glossary/conditional-write.md>
[context]: <glossary/context.md>
[crc32c]: <glossary/crc32c.md>
[datashed]: <glossary/datashed.md>
[envbuf]: <glossary/envbuf.md>
[epilogue]: <glossary/epilogue.md>
[epilogue-compaction]: <glossary/epilogue-compaction.md>
[flexbuffers]: <glossary/flexbuffers.md>
[flow]: <glossary/flow.md>
[kernelspace]: <glossary/kernelspace.md>
[lexidecimal]: <glossary/lexidecimal.md>
[milestone]: <glossary/milestone.md>
[node]: <glossary/node.md>
[object-keys]: <object-keys.md>
[prologue-vacuuming]: <glossary/prologue-vacuuming.md>
[promotion-sign]: <glossary/promotion-sign.md>
[reservoir]: <glossary/reservoir.md>
[sha384]: <glossary/sha384.md>
[timeline]: <glossary/timeline.md>
[timeline-eviction]: <glossary/timeline-eviction.md>
[tombstone]: <glossary/tombstone.md>
[turso]: <glossary/turso.md>
[viewpoint]: <glossary/viewpoint.md>

# Datashed backup and recovery

Keep *Reservoir*[~][reservoir] keyspaces separate using the shared
*object-key patterns*[~][object-keys].

Use *complemented Lexidecimal*[~][lexidecimal] positions for *markers*[~][commit-marker]
in both *Epilogues*[~][epilogue]; their *PromotionSigns*[~][promotion-sign] use
uncomplemented positions in those same namespaces. *Datashed*[~][datashed]
*Viewpoints*[~][viewpoint] count backups independently of *Milestones*[~][milestone].
A *Datashed*[~][datashed] root is a *FlexBuffers*[~][flexbuffers] message:

```ts
{
  version: string;             // Turso release and revision
  milestone: string;           // covered decimal Milestone M
  coordinate: string;          // exclusive decimal coordinate
  crc32c: string;              // cumulative CRC32C, eight lowercase hex digits
  parent: string;              // event SHA-384 at M
  databases: Record<"default.turso" | "kernel.turso", {
    id: string; revision: string; length: number; sha384: string;
  }>;
}
```

Both database images are height-zero *KernelSpace*[~][kernelspace] *nodes*[~][node]
at the same prefix. Verify their lengths, checksums, and recorded
*Milestones*[~][milestone] before exposing either. Restore both, then replay
from `M + 1`; the *marker's*[~][commit-marker] own reservation and commit events
follow `M`. Fetch immutable revisions from either generation using the
*object-key patterns*[~][object-keys].
A backup's S3 metadata
binds its image to the recovery prefix:

```text
x-amz-meta-milestone: <canonical decimal>
x-amz-meta-coordinate: <exclusive Timeline byte coordinate, canonical decimal>
x-amz-meta-crc32c: <cumulative coordinate-run CRC32C, eight lowercase hex digits>
x-amz-meta-event-sha384: <digest of DifferentialEvent at that Milestone>
x-amz-meta-sha384: <digest of CommitMarker bytes>
x-amz-meta-turso-version: <engine release and revision>
```

Retain the *CRC32C*[~][crc32c] as the recovered prefix's accumulator.
Other digests are hexadecimal *SHA-384*[~][sha384]. Unwrap the raw *EnvBuf message*[~][envbuf], validate metadata and its checksum,
then validate the root's *Turso*[~][turso] version and recovery prefix.

Before publishing a *Datashed*[~][datashed] *CommitMarker*[~][commit-marker], replicate
all *Flow*[~][flow] *markers*[~][commit-marker] through its
*Milestone*[~][milestone] into their *Epilogues*[~][epilogue]. Use
*conditional creation*[~][conditional-write]; preserve *context*[~][context] metadata and verify existing bytes and context on retry.
Competing backup workers adopt the winning *Viewpoint*[~][viewpoint], keeping
backed-up *Milestones*[~][milestone] increasing.

Plan future *Epilogue compaction*[~][epilogue-compaction] around a contiguous
available prefix of stored *CommitMarker*[~][commit-marker]/*PromotionSign*[~][promotion-sign]
pairs. *Markers*[~][commit-marker] must have reached their *Epilogues*[~][epilogue]
from the *Datashed*[~][datashed] *Timeline*[~][timeline]; a recovery anchor alone
is insufficient. Compact up to, but not through, the retained *Viewpoint*[~][viewpoint].
Replace compacted *markers*[~][commit-marker] with bucket-proven
*tombstones*[~][tombstone],
retaining their keys and metadata, and DELETE their
*PromotionSigns*[~][promotion-sign]. A sign LISTer checks the corresponding
*marker*[~][commit-marker] metadata, deleting and skipping stale signs on demand
before selecting the oldest available snapshot; see *compaction*[~][epilogue-compaction].

A stored *Datashed*[~][datashed] recovery anchor permits *Timeline eviction*[~][timeline-eviction]
through its *Milestone*[~][milestone] once the corresponding *Flow*[~][flow]
*markers*[~][commit-marker] are replicated. Serialize cleanup requests through the
*Datashed*[~][datashed] *Timeline*[~][timeline]. *Prologue vacuuming*[~][prologue-vacuuming]
is a separate process.

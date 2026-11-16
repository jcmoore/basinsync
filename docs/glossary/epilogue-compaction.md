[commit-marker]: <commit-marker.md>
[empty-sentinel]: <empty-sentinel.md>
[envbuf]: <envbuf.md>
[epilogue]: <epilogue.md>
[promotion-sign]: <promotion-sign.md>
[viewpoint]: <viewpoint.md>

# Epilogue compaction

Reclaim *CommitMarker*[^][commit-marker] payloads in an *Epilogue*[^][epilogue]
up to, but not through, a retained *Viewpoint*[^][viewpoint].

Plan for a contiguous available prefix of stored *CommitMarker*[^][commit-marker]/
*PromotionSign*[^][promotion-sign] pairs. Tombstone compacted *markers*[^][commit-marker]
with *empty sentinels*[^][empty-sentinel] and explicit metadata, such as
`x-amz-meta-compacted-at`; valid empty *markers*[^][commit-marker] retain their
*EnvBuf*[^][envbuf] framing. Then DELETE their signs.

Sign publishers check for tombstones before and after PUT, removing their sign
if compaction overtook publication. LISTers validate the corresponding
*markers*[^][commit-marker], deleting and skipping signs for tombstones on demand
before selecting the oldest available snapshot.

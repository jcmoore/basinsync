---
prerequisites: []
---

[commit-marker]: <../../../glossary/commit-marker.md>
[conditional-read]: <../../../glossary/conditional-read.md>
[conditional-write]: <../../../glossary/conditional-write.md>
[context]: <../../../glossary/context.md>
[development]: <../../../development.md>
[empty-sentinel]: <../../../glossary/empty-sentinel.md>
[envbuf]: <../../../glossary/envbuf.md>
[epilogue]: <../../../glossary/epilogue.md>
[epilogue-protocol]: <../../../epilogue-protocol.md>
[etag]: <../../../glossary/etag.md>
[json0]: <../../../glossary/json0.md>
[margin]: <../../../glossary/margin.md>
[object-keys]: <../../../object-keys.md>
[range-get]: <../../../glossary/range-get.md>
[sha384]: <../../../glossary/sha384.md>
[tombstone]: <../../../glossary/tombstone.md>
[viewpoint]: <../../../glossary/viewpoint.md>

# Shallow epilogue commits

Start with an *Epilogue*[^][epilogue] of opaque blobs, each stored as a raw
*EnvBuf message*[^][envbuf]. Reserve zero-byte objects for *metadata-only
sentinels*[^][empty-sentinel]; even an empty blob has framing.

Bootstrap the *empty-reference object*[^][object-keys] with `If-None-Match: *`,
verify it is empty, and learn
its *ETag*[^][etag] from the bucket. Probe distinct *tombstone*[^][tombstone] and
standalone *margins*[^][margin] with one-byte counters 1–7 too; fail startup
if none differs. After each successful upload, require
an *ETag*[^][etag] different from the empty reference. Feeds have a trailing
*margin*[^][margin] whose eight-byte little-endian `u64` message is zero; increment
it on each `If-Match` retry until distinct.

1. Build a Rust server that recovers its next *viewpoint*[^][viewpoint] from the
   bucket's lowest non-sentinel *Epilogue*[^][epilogue] key, treating
   *sentinels*[^][empty-sentinel] as exclusive bounds and starting at zero when empty.
2. Accept `PUT /api/v0/epilogue`: a raw `blob` part and *JSON0*[^][json0]
   `details` containing a bigint *viewpoint*[^][viewpoint] and byte-array
   *SHA-384*[^][sha384], plus optional `context: Record<string, string>`.
   Attach *context*[^][context] as `x-amz-meta-ctx-` metadata on the stored
   *marker*[^][commit-marker]. Reject a wrong *viewpoint*[^][viewpoint] or
   *digest*[^][sha384]; otherwise *conditionally create*[^][conditional-write]
   its *marker*[^][commit-marker], using `If-Match` with the observed *ETag*[^][etag]
   when replacing a *sentinel*[^][empty-sentinel]. Advance after success; return an error and
   terminate after write failure.
3. Offer `GET /api/v0/epilogue/status`. Return `latest` with the committed
   *marker's*[^][commit-marker] *viewpoint*[^][viewpoint], length, and presigned
   GET URL with signed *Range*[^][range-get] and *conditional read headers*[^][conditional-read]
   selecting only the message bytes. For length zero,
   omit the URL and headers;
   when empty, return `latest: null`.
   On *viewpoint*[^][viewpoint] mismatch, return the expected next
   *viewpoint*[^][viewpoint].
4. Stress concurrent writers with TypeScript clients against local S3.
   Check decreasing, gapless keys and winning blob contents, then restart to
   verify recovery.

Use the *wire protocol*[^][epilogue-protocol] and
*development checks*[^][development]; leave richer *markers*[^][commit-marker]
and graceful race recovery for later milestones.

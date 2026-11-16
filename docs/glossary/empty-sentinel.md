[conditional-write]: <conditional-write.md>
[envbuf]: <envbuf.md>
[etag]: <etag.md>
[object-keys]: <../object-keys.md>

# Empty sentinel

A zero-byte sequence object carrying only metadata. It is an exclusive search
bound, not a published entry. Bootstrap the bucket's *reference object*[^][object-keys]
with `If-None-Match: *`, verify zero length, and retain its observed *ETag*[^][etag].
Do not hardcode a provider's empty-object validator.

Create absent entries *conditionally*[^][conditional-write]; replace a sentinel
only with `If-Match: <observed ETag>`. Nonempty entries remain first-writer-wins.
Metadata-only rewrites may retain the same *ETag*[^][etag], so this does not fence
competing metadata revisions.

Server-controlled *EnvBuf feeds*[^][envbuf] append padding if their predicted
validator matches the reference; verify returned validators before acknowledging
publication. Opaque user blobs outside sequences may still be empty.

[conditional-write]: <conditional-write.md>
[envbuf]: <envbuf.md>
[etag]: <etag.md>
[object-keys]: <../object-keys.md>
[tombstone]: <tombstone.md>

# Empty sentinel

A zero-byte sequence object carrying only metadata. A trailing sentinel is an
exclusive upper search bound, not a published entry. Retired leading positions
use immutable *tombstones*[^][tombstone] instead. Bootstrap the bucket's *reference object*[^][object-keys]
with `If-None-Match: *`, verify zero length, and retain its observed *ETag*[^][etag].
Do not hardcode a provider's empty-object validator.

Create absent entries *conditionally*[^][conditional-write]; replace a sentinel
only with `If-Match: <observed ETag>`. Nonempty entries remain first-writer-wins.
Metadata-only rewrites may retain the same *ETag*[^][etag], so this does not fence
competing metadata revisions.

Require every successful sequence write to return an *ETag*[^][etag] distinct
from the empty reference before acknowledging publication. A match fails the
postcondition: append one bucket-proven padding *EnvBuf*[^][envbuf]
and retry with `If-Match` on that observed validator. At startup, conditionally
probe single padding *EnvBufs*[^][envbuf] with message lengths 1 through 8,
in order, using `0xee` message bytes (each envelope is 32 bytes); fail startup
if none has a distinct validator. Reuse each fixed *probe key*[^][object-keys],
advancing colliding candidates with `If-Match` and verifying the selected bytes
on later boots. Opaque user blobs outside sequences may still be empty.

[empty-sentinel]: <empty-sentinel.md>
[envbuf]: <envbuf.md>
[etag]: <etag.md>
[flexbuffers]: <flexbuffers.md>
[promotion-sign]: <promotion-sign.md>

# Margin

A skippable format `110` *EnvBuf*[^][envbuf], distinct from the `0xee` alignment
padding between an envelope's message and footer.

At startup, probe standalone margins with message lengths 1–7 filled with `0xee`;
terminate if none has an *ETag*[^][etag] distinct from the *empty sentinel's*[^][empty-sentinel].
Each is 32 bytes with default alignment; use the selected margin for *PromotionSigns*[^][promotion-sign].

For a colliding sequence upload, append one 32-byte margin with an eight-byte message:
a `u64` counter in *FlexBuffers'*[^][flexbuffers] little-endian order. Start at zero,
incrementing and replacing that same margin until the response's *ETag*[^][etag]
differs from the empty reference; never wrap the counter. Preserve the original
feed prefix and retry with `If-Match` on the observed validator.

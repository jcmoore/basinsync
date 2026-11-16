[empty-sentinel]: <empty-sentinel.md>
[etag]: <etag.md>
[tombstone]: <tombstone.md>

# Conditional write

S3 `PutObject` with `If-None-Match: *` creates an object only when its key is
absent. A zero-byte *sentinel*[^][empty-sentinel] can instead be replaced using
its observed *ETag*[^][etag] in `If-Match`. Competing writers cannot replace a
nonempty first writer's bytes, including *tombstones*[^][tombstone]. A successful
sequence write must also satisfy its *ETag postcondition*[^][empty-sentinel].

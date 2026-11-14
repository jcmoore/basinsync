[commit-marker]: <commit-marker.md>
[heap-reservation]: <heap-reservation.md>
[json0]: <json0.md>
[node]: <node.md>

# Context

A `Record<string, string>` supplied in commit `details` or per *node*[^][node]
in a *heap reservation*[^][heap-reservation], using *JSON0*[^][json0]
string values. Each entry becomes `x-amz-meta-ctx-${key}` metadata on the stored
*CommitMarker*[^][commit-marker] or uploaded *node*[^][node]; omitted context means an empty map.

Names must be nonempty HTTP header suffixes without NUL and distinct ignoring
case. Values must be printable ASCII; metadata, including service metadata, must fit
S3's 2 KiB user-metadata limit. Validate before accepting the request.

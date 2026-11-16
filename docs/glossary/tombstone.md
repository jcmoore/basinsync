[empty-sentinel]: <empty-sentinel.md>
[envbuf]: <envbuf.md>
[etag]: <etag.md>
[object-keys]: <../object-keys.md>

# Tombstone

An immutable retired sequence position, distinct from a replaceable trailing
*empty sentinel*[~][empty-sentinel]. Probe the bucket with `0xdead`, `0xdeaddead`,
and successive repetitions, choosing the first whose *ETag*[~][etag] differs
from the empty reference. Refuse a sixteen-byte candidate and terminate startup
if no shorter candidate works.

Tombstones remain below the sixteen-byte *EnvBuf*[~][envbuf] floor. They occupy
older positions as exclusive lower bounds; they are neither published data nor
eligible for sentinel replacement. Keep their keys and retirement metadata.
The *probe keys*[~][object-keys] preserve the bucket's chosen representation.

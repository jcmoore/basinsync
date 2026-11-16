[empty-sentinel]: <empty-sentinel.md>
[http-preconditions]: <https://www.rfc-editor.org/rfc/rfc9110.html#section-13.2.1>
[tigris-pricing]: <https://www.tigrisdata.com/pricing/>

# Conditional read

A GET or HEAD guarded by HTTP preconditions. Use `If-Match: *` for S3 reads.
Sequence probes also send `If-None-Match: <bucket empty-reference ETag>`:
304 marks an *empty sentinel*[^][empty-sentinel]; 200 with positive content length
marks a published entry; 404 or 412 means absent. Check length when an endpoint
ignores conditions. Treat sentinels as exclusive bounds. Other responses are
errors. Reads needing bytes or metadata omit `If-None-Match`.

*Tigris pricing*[^][tigris-pricing] exempts 304 and 412 responses. Missing objects
can still return 404: *HTTP preconditions*[^][http-preconditions] do not guarantee
that `If-Match` takes precedence over absence.

Presigned GETs bind `If-Match` into their signature; clients must send it.

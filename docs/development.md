[conditional-write]: <glossary/conditional-write.md>
[go-faster-fs]: <https://github.com/go-faster/fs>
[sha384]: <glossary/sha384.md>
[versitygw]: <https://github.com/versity/versitygw>
[viewpoint]: <glossary/viewpoint.md>

# Development

Use *go-faster/fs*[^][go-faster-fs] or *Versity Gateway*[^][versitygw] as the
S3-compatible endpoint. Write TypeScript clients that stress one or more
concurrent Rust server processes.

Verify gapless sequences, winning blob contents, and restart recovery. Wrong
*viewpoints*[^][viewpoint] and *digests*[^][sha384] must fail without advancing
state. Race servers against the same position: only the first
*conditional writer*[^][conditional-write] wins.

Queued mutations belong to workers, not HTTP handlers: a lost reply must not
cancel publication. Trace cancellation through callers before adding timeouts
or `select!`, especially around temporarily removed state or remote writes;
cancellation does not roll back S3.

Streaming producer failures must reach uploaders as read errors, never successful
EOF. Await multipart abort cleanup rather than dropping the uploader. Shutdown
must close admission, stop background producers, drain queued publications, and
await tracked tasks. `BASINSYNC_SHUTDOWN_TIMEOUT_MS` defaults to `30000`; exhausting
it exits unsuccessfully and leaves in-flight outcomes for recovery. Test these
boundaries with gated I/O, disconnects, source failures, and shutdown signals.
Process crashes or forced termination can still leave unfinished multipart uploads
for eventual reclamation.

[analogue]: <analogue.md>
[bulk-manifest]: <bulk-manifest.md>
[byte-coordinate]: <byte-coordinate.md>
[crc32c]: <crc32c.md>
[differential-event]: <differential-event.md>
[envbuf]: <envbuf.md>
[margin]: <margin.md>
[object-keys]: <../object-keys.md>

# BulkOrder

An *EnvBuf feed*[~][envbuf] in an *Analogue*[~][analogue]: complete, contiguous
*DifferentialEvents*[~][differential-event], followed by a *BulkManifest*[~][bulk-manifest]
footer and a *counter margin*[~][margin]. Its *object key*[~][object-keys] carries
the event interval, exclusive end of the event bytes, and starting *Timeline byte coordinate*[~][byte-coordinate].

Omit each *event's*[~][differential-event] separate trailing retry *margins*[~][margin]
and copy its complete *EnvBuf*[~][envbuf] unchanged, preserving all bytes and digests. Retain one *counter margin*[~][margin] for the order itself.

S3 metadata records cumulative *CRC32C*[~][crc32c] at the order's start and
exclusive event end; footer bytes and the order's *margin*[~][margin] are excluded.

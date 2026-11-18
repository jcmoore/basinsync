[bulk-manifest]: <bulk-manifest.md>
[bulk-order]: <bulk-order.md>
[byte-coordinate]: <byte-coordinate.md>
[crc32c-api]: <https://docs.rs/crc32c/0.6.8/crc32c/fn.crc32c_append.html>
[differential-event]: <differential-event.md>
[margin]: <margin.md>
[reservoir]: <reservoir.md>

# CRC32C

A cumulative Castagnoli checksum of a *Reservoir's*[~][reservoir] logical
*coordinate bytes*[~][byte-coordinate], starting from zero (`00000000` for the empty run). Continue the
*incremental checksum*[~][crc32c-api] through each complete margin-free
*DifferentialEvent*[~][differential-event], including its complete outer framing
and alignment. Retry *margins*[~][margin], *BulkManifests*[~][bulk-manifest], and
*BulkOrder*[~][bulk-order] trailing *margins*[~][margin] contribute nothing.

S3 user metadata stores eight lowercase hexadecimal digits:

```text
x-amz-meta-base-crc32c: <cumulative CRC at the object's starting coordinate>
x-amz-meta-crc32c: <cumulative CRC at its exclusive ending coordinate>
```

These describe the coordinate run from zero, rather than the HTTP response body
or the complete stored object. A backup retains the ending checksum to seed
recovery and subsequent writes without rereading its covered history.

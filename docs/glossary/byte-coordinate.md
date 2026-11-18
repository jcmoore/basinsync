[bulk-manifest]: <bulk-manifest.md>
[bulk-order]: <bulk-order.md>
[crc32c]: <crc32c.md>
[differential-event]: <differential-event.md>
[envbuf]: <envbuf.md>
[margin]: <margin.md>
[reservoir]: <reservoir.md>
[timeline]: <timeline.md>

# Byte coordinate

An arbitrary-precision nonnegative logical byte offset in a *Reservoir's*[~][reservoir]
*Timeline*[~][timeline]. Start at zero and advance by each *DifferentialEvent's*[~][differential-event]
*EnvBuf's*[~][envbuf] exact size, including its header, message, alignment padding,
and footer. Separate trailing storage *margins*[~][margin] have no coordinate extent. Cleanup never renumbers coordinates.

Changing, adding, or removing trailing retry *margins*[~][margin] does not change
coordinates. *BulkManifest*[~][bulk-manifest] bytes and *BulkOrder*[~][bulk-order]
*margins*[~][margin] also contribute nothing. *BulkOrders*[~][bulk-order] omit event
storage suffixes and copy each event envelope unchanged. Physical
offsets within their event prefix therefore equal relative logical coordinates;
the *BulkManifest*[~][bulk-manifest] records plain exclusive ends.

*CRC32C*[~][crc32c] accumulates over this entire run, independently of storage
*margins*[~][margin] and archive footers.

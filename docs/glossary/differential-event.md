[byte-coordinate]: <byte-coordinate.md>
[crc32c]: <crc32c.md>
[envbuf]: <envbuf.md>
[flexbuffers]: <flexbuffers.md>
[margin]: <margin.md>
[milestone]: <milestone.md>
[range-manifest]: <range-manifest.md>
[reservoir]: <reservoir.md>
[timeline]: <timeline.md>

# DifferentialEvent

A batch of *Reservoir*[~][reservoir] commands stored at one *Milestone*[~][milestone]
in its *Timeline*[~][timeline]. One nested *EnvBuf*[~][envbuf] (`100`) wraps its
command feed of *FlexBuffers*[~][flexbuffers] envelopes and final manifest.
Storage appends a separate retry *margin EnvBuf*[~][margin] after the event's
outer footer. Retry changes leave the complete event envelope unchanged;
archives concatenate those event envelopes without their storage suffixes.

Its *range manifest*[~][range-manifest] records its starting *byte coordinate*[~][byte-coordinate].

S3 metadata records the cumulative *CRC32C*[~][crc32c] at its start and exclusive end.

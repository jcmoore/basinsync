[envbuf]: <envbuf.md>
[flexbuffers]: <flexbuffers.md>
[milestone]: <milestone.md>
[reservoir]: <reservoir.md>
[timeline]: <timeline.md>

# DifferentialEvent

A batch of *Reservoir*[~][reservoir] commands stored at one *Milestone*[~][milestone]
in its *Timeline*[~][timeline]. One nested *EnvBuf*[~][envbuf] (`100`) wraps its
command feed of *FlexBuffers*[~][flexbuffers] envelopes, manifest, and retry margin,
allowing events to concatenate without losing their boundaries.

[analogue]: <analogue.md>
[differential-event]: <differential-event.md>
[envbuf]: <envbuf.md>
[flexbuffers]: <flexbuffers.md>
[margin]: <margin.md>
[milestone]: <milestone.md>
[object-keys]: <../object-keys.md>

# BulkOrder

An *EnvBuf feed*[~][envbuf] in an *Analogue*[~][analogue]: complete, contiguous
*DifferentialEvents*[~][differential-event], followed by a format `101` footer tuple
and a *counter margin*[~][margin]:

```ts
[Symbol.species, "sync.memoremix.com", "sts/0/databasin/bulkorder", {
  lowest: 42n,
  offsets: [0, 512, 1408, 2048]
}]
```

`lowest` is the first inclusive *Milestone*[~][milestone]. Each adjacent pair in
`offsets` bounds one complete *event*[~][differential-event], relative to the object's beginning. Require
zero first, strictly increasing eight-byte boundaries, and the last offset at
the footer's start. Derive the exclusive upper bound as
`lowest + BigInt(offsets.length - 1)`; verify both bounds against the
*object key*[~][object-keys]. Encode `lowest` with the *bigint extension*[~][flexbuffers]
and offsets with an ordinary signed-integer vector, keeping `VT_VECTOR_UINT`
reserved for extensions.

S3 metadata `x-amz-meta-bulkorder-range: bytes=start-end` identifies the complete
footer envelope's inclusive byte range. Fetch it directly, or traverse backward
past trailing *margins*[~][margin]. Neighboring offsets then select event ranges.

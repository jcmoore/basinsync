[bulk-order]: <bulk-order.md>
[differential-event]: <differential-event.md>
[envbuf]: <envbuf.md>
[flexbuffers]: <flexbuffers.md>
[milestone]: <milestone.md>
[object-keys]: <../object-keys.md>

# BulkManifest

A format `101` *EnvBuf*[~][envbuf] indexing complete *DifferentialEvents*[~][differential-event]
in a *BulkOrder*[~][bulk-order]:

```ts
[Symbol.species, "sync.memoremix.com", "sts/0/databasin/bulkmanifest", {
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

The *object key's*[~][object-keys] final component is its HTTP `bytes=start-end`
range, with an inclusive end. S3 metadata `x-amz-meta-bulkmanifest-range` repeats
that range. LIST therefore locates the footer without HEAD; neighboring indexed
offsets select complete event ranges.

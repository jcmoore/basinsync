[envbuf]: <envbuf.md>
[flexbuffers]: <flexbuffers.md>
[lexidecimal]: <lexidecimal.md>
[margin]: <margin.md>

# Range manifest

The final *non-margin*[~][margin] *EnvBuf*[~][envbuf] in a command feed. Its
*FlexBuffers*[~][flexbuffers] message indexes preceding command *envelopes*[~][envbuf] and *margin runs*[~][margin] by their
exclusive end offset in the containing object, encoded as positive
*Lexidecimal*[~][lexidecimal]:

```ts
[Symbol.species, "sync.memoremix.com", "sts/0/databasin/rangemanifest", {
  envelopes: { [lex_exclusive: string]: [Symbol.species, host, path] | null }
}]
```

Command values are prefixes without query payloads. A consecutive run of *margin
envelopes*[~][margin] has one `null` entry at the last envelope's exclusive end.
The manifest omits itself. Containing S3 objects carry
`x-amz-meta-manifest-range: bytes=start-end`, its complete *EnvBuf's*[~][envbuf]
inclusive decimal byte range, excluding trailing *margins*[~][margin]. Fetch that range directly,
or locate it by reading backward from the object's final footer, skipping *margin
envelopes*[~][margin].

*FlexBuffers*[~][flexbuffers] sorts these keys lexicographically, so neighboring
decoded offsets bound one command *EnvBuf*[~][envbuf] or *margin run*[~][margin]: `[previous, current)`, with
zero preceding the first key. Its HTTP Range is `bytes=previous-(current - 1)`.

[envbuf]: <envbuf.md>
[flexbuffers]: <flexbuffers.md>
[lexidecimal]: <lexidecimal.md>

# Range manifest

The final non-padding *EnvBuf*[^][envbuf] in a command feed. Its
*FlexBuffers*[^][flexbuffers] message indexes preceding command *envelopes*[^][envbuf] and padding runs by their
exclusive end offset in the containing object, encoded as positive
*Lexidecimal*[^][lexidecimal]:

```ts
[Symbol.species, "sync.memoremix.com", "sts/0/databasin/rangemanifest", {
  envelopes: { [lex_exclusive: string]: [Symbol.species, host, path] | null }
}]
```

Command values are prefixes without query payloads. A consecutive run of padding
*envelopes*[^][envbuf] has one `null` entry at the last envelope's exclusive end.
The manifest omits itself. Containing S3 objects carry
`x-amz-meta-manifest-range: bytes=start-end`, its complete *EnvBuf's*[^][envbuf]
inclusive decimal byte range, excluding trailing padding. Fetch that range directly,
or locate it by reading backward from the object's final footer, skipping padding
*envelopes*[^][envbuf].

*FlexBuffers*[^][flexbuffers] sorts these keys lexicographically, so neighboring
decoded offsets bound one command *EnvBuf*[^][envbuf] or padding run: `[previous, current)`, with
zero preceding the first key. Its HTTP Range is `bytes=previous-(current - 1)`.

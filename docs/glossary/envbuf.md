[flexbuffers]: <flexbuffers.md>

# EnvBuf

“Enveloped Buffers” (pronounced “Envy Buff”): concatenable binary envelopes
with matching headers and footers for forward and backward traversal.
Prefer `.nv` filenames.

Each header and footer occupies eight bytes:

```text
header: "enV" | format:3, alignment_power:5 | content_length:u32le
footer: content_length:u32le | format:3, alignment_power:5 | "vNE"
```

The descriptor's high three bits select the format: `000` unknown/raw,
`100` nested EnvBuf, `101` *FlexBuffers*[^][flexbuffers], and `110` padding.
`111` is reserved for platform extensions; other codes are reserved.
Readers skip `110` payloads while validating their framing;
`000` payloads remain readable.
The low five bits encode the alignment as `1 << alignment_power`, defaulting
to sixteen bytes (`00100`). Pad between content and footer with `0xee` so
the complete envelope, including header and footer, is a multiple of that alignment:
`ceil((content_length + 16) / alignment) * alignment`. Content begins eight bytes after
the header starts.
Content length excludes padding and framing. The default descriptor for
*FlexBuffers*[^][flexbuffers] is `0xa4`.

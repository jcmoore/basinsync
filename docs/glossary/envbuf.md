[flexbuffers]: <flexbuffers.md>

# EnvBuf

“Enveloped Buffers” (pronounced “Envy Buff”): concatenable binary envelopes
with matching headers and footers for forward and backward traversal.
A concatenation of envelopes is a feed; each envelope carries a message.
Prefer `.nv` filenames.

Each header and footer occupies eight bytes:

```text
header: "enV" | format:3, alignment_power:5 | message_length:u32le
footer: message_length:u32le | format:3, alignment_power:5 | "vNE"
```

The descriptor's high three bits select the format: `000` unknown/raw,
`100` nested EnvBuf, `101` *FlexBuffers*[^][flexbuffers], and `110` padding.
`111` is reserved for platform extensions, including larger headers and footers
supporting `u64` message lengths; that support is pending. Other codes are reserved.
Readers skip `110` messages while validating their framing;
`000` messages remain readable.
The low five bits encode the alignment as `1 << alignment_power`, defaulting
to sixteen bytes (`00100`). Pad between message and footer with `0xee` so
the complete envelope, including header and footer, is a multiple of that alignment:
`ceil((message_length + 16) / alignment) * alignment`. The message begins eight bytes after
the header starts.
Message length excludes padding and framing. The default descriptor for
*FlexBuffers*[^][flexbuffers] is `0xa4`.

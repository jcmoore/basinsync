# Codec kit

Taken from clean room `261122-claude-r3` (workspace commit
`acd667829c00cf0ad4d2fae3d0fca4942e70dfb3`, docs from basinsync
`41736b231e500ad97b2fa8fdea9a0fe1e9511e96`): its `crates/codec`, `crates/oracle`,
and `scripts/kani-all.sh`, with a standalone workspace manifest and lockfile.
It passed all 164 vector cases, and 26 of its 27 Kani harnesses verify; see
`crates/codec/KANI.md`.

Changed since: JSON0 escapes every literal string beginning with `0`,
including `"0"`, and rejects a lone `"0"` (docs round 5); `json0_literal_escape`
re-verified afterward. FlexBuffers decoding enforces the layout rule (a value
ends at or before the slot that refers to it; docs round 5) in `layout.rs` and
the extension core; the four `flex_ext_*` harnesses re-verified afterward.

Refresh it from a codec-only clean room with `scripts/cleanroom/kit-import.sh`.

# Kani proofs for the codecs

Harnesses live in `src/kani_proofs.rs` (`#[cfg(kani)]`). Kani 0.68.0 with
CBMC 6.11.0. Run them one at a time, each under a timeout and a memory cap:

```sh
timeout 900 cargo kani -p basinsync-codec --harness <name>
# or sequentially, with a 16 GB virtual-memory cap per harness:
HARNESSES="a b" MEM_KB=16000000 scripts/kani-all.sh 900 .local/kani.txt
```

Inputs are symbolic byte arrays with a symbolic length `len ≤ N`, written
"≤ N bytes" below. The BigInt-facing APIs wrap allocation-free cores over
digit and byte slices, and those cores are what the harnesses prove. CBMC
handles `num-bigint`, symbolic-capacity `Vec`s, `std::str::from_utf8`, and
symbolic-length `copy_from_slice` poorly. In the table, *Time* is wall-clock
and *Peak* is the peak resident memory of the run.

| Harness | Property | Bound | Result | Time | Peak |
| --- | --- | --- | --- | ---: | ---: |
| `lex_i64_roundtrip` | Lexidecimal `decode(encode(n)) == n` | \|n\| < 2^15, unwind 18 | ✅ | 24 s | 0.7 GB |
| `lex_decode_canonical` | no panic on arbitrary text; accepted text re-encodes to itself | ≤ 8 bytes | ✅ | 15 s | 0.4 GB |
| `lex_digits_roundtrip` | digit-core round trip (both signs) | ≤ 5 payload digits | ✅ | 14 s | 0.4 GB |
| `lex_rejects_nonminimal_payload` | leading `0` (or `f` for negatives) in a multi-digit payload is rejected | ≤ 8 bytes | ✅ | 7 s | 0.3 GB |
| `lex_size_prefix_shape` | size prefix uses the smallest width and the right letter for any `u64` size and either sign | all `u64` | ✅ | 32 s | 1.2 GB |
| `d16_i64_roundtrip` | Diffuse16 `decode(encode(n)) == n` | \|n\| < 2^15 | ✅ | 14 s | 0.5 GB |
| `d16_decode_canonical` | no panic; accepted text re-encodes to itself (non-minimal and uppercase rejected) | ≤ 8 bytes | ✅ | 7 s | 0.3 GB |
| `d16_digits_roundtrip` | digit-core round trip | ≤ 5 digits | ✅ | 8 s | 0.3 GB |
| `envbuf_feed_no_panic` | `parse_feed` never panics | ≤ 16 bytes | ✅ | 329 s | 9.8 GB |
| `envbuf_backward_no_panic` | `parse_backward` never panics, for any `end` | ≤ 24 bytes | ✅ (after fix, see below) | 12 s | 0.5 GB |
| `envbuf_decode_canonical` | an accepted envelope re-encodes byte-for-byte, and the backward parse agrees | ≤ 32 bytes | ✅ | 50 s | 1.7 GB |
| `envbuf_roundtrip` | encode then decode returns format, power and message | message ≤ 8 bytes, power ≤ 4 | ✅ | 30 s | 1.2 GB |
| `envbuf_margins` | the counter margin is 24 bytes and its counter round-trips | all `u64` counters | ✅ | 22 s | 0.6 GB |
| `flex_ext_decode_no_panic` | the extension validator never panics | ≤ 8 bytes | ✅ | 53 s | 1.4 GB |
| `flex_ext_roundtrip` | encode then validate returns prefix and payload, for any prefix byte | payload ≤ 3 bytes | ✅ | 58 s | 1.7 GB |
| `flex_ext_decode_canonical_minimal_layout` | a valid minimal-layout buffer re-encodes to identical bytes | ≤ 8 bytes | ✅ | 91 s | 2.1 GB |
| `flex_ext_rejects_noncanonical_magnitude` | an empty or zero-led bigint magnitude is rejected by both the encoder and the decoder | magnitude ≤ 3 bytes | ✅ | 38 s | 1.4 GB |
| `flex_decode_no_panic` | the general FlexBuffers decoder never panics | ≤ 3 bytes | ⏱ timeout (900 s) | — | — |
| `crc_table_matches_bitwise` | the table-driven CRC32C equals the bitwise reference for any seed | ≤ 4 bytes | ✅ | 24 s | 0.3 GB |
| `crc_incremental` | `update(update(c, a), b) == update(c, a ++ b)` | ≤ 2 + 2 bytes | ✅ | 62 s | 0.3 GB |
| `crc_hex8_parse` | an accepted 8-digit hex value re-renders identically; parse(hex8(x)) == x | 8 bytes | ✅ | 106 s | 3.2 GB |
| `sha_hex_parse_canonical` | hex core: accepted lowercase hex re-encodes identically | 4-byte digest (8 hex digits) | ✅ | 5 s | 0.2 GB |
| `sha_hex_roundtrip` | hex core round trip | 4-byte digest | ✅ | 4 s | 0.2 GB |
| `sha_hex_rejects_wrong_length` | anything other than 96 digits is rejected | ≤ 2 bytes | ✅ | 4 s | 0.3 GB |
| `flow_id_normalize` | no panic; accepted ids are the lowercased input and idempotent; `.`, `..`, `/` and other characters are rejected | ≤ 5 bytes | ✅ | 417 s | 14.6 GB |
| `json0_literal_escape` | escaping a literal `0`-prefixed string and classifying it yields the original | ≤ 5 bytes | ✅ | 8 s | 0.4 GB |

26 of 27 harnesses verify.

## Findings

- **Bug found by Kani:** `envbuf_backward_no_panic` found inputs where the
  trailing footer and the header at `end - total` describe different
  envelopes. Debug builds hit a `debug_assert` panic, and release builds
  returned the wrong envelope. `parse_backward` now rejects the mismatch, and
  `tests/utf8.rs::backward_parse_rejects_mismatched_header` is a regression
  test.
- **General FlexBuffers decoder (`flex_decode_no_panic`):** this times out
  even on 3 bytes because of its recursion (depth ≤ 128), map construction
  and `Vec`s. Two proptests cover it instead: `decode_never_panics` (random
  buffers up to 64 bytes) and `decode_mutated_never_panics` (single-byte
  mutations and truncations of valid encodings). Each ran 20,000 cases with
  no panic.

## How the bounds were reached

| Harness | Change | Reason |
| --- | --- | --- |
| `envbuf_no_panic` | split into `envbuf_feed_no_panic` (16 bytes) and `envbuf_backward_no_panic` (24 bytes) | the original 32-byte harness timed out at 900 s |
| FlexBuffers extension harnesses | now prove allocation-free cores (`validate_extension`, `encode_extension_into`); see below | ran out of memory at about 15–18 GB, almost regardless of bound; the cause was symbolic-capacity `Vec` code, not input size |
| SHA-384 hex harnesses | moved to slice-generic cores on 4-byte digests | 96 symbolic digits timed out |
| `crc_table_matches_bitwise` | `unwind(6)` raised to 9 | the bitwise reference loops 8 times per byte |

The FlexBuffers extension cores replaced `std::str::from_utf8` with the
explicit `is_utf8`. `tests/utf8.rs` checks it against `std` on all 1–2-byte
inputs, a grid of 3-byte inputs, and 200,000 random strings. They also use a
loop-based `read_uint`.

Peak memory stayed below the 25 GB shared cap throughout. Each run was
capped at 16 GB.

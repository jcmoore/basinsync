//! Kani proof harnesses for the codec cores. Bounds are recorded in
//! `crates/codec/KANI.md`. Run one at a time, e.g.
//! `timeout 900 cargo kani -p basinsync-codec --harness lex_i64_roundtrip`.
//!
//! Canonicality: each "canonical" harness proves `decode(s) = Ok(v) ==>
//! encode(v) == s`. Because `encode` is a function, any noncanonical
//! spelling `s'` of a value (one with `s' != encode(v)`) therefore cannot be
//! accepted, which is the noncanonical-rejection property; additional
//! harnesses check specific noncanonical shapes directly.

use crate::{base64 as _, crc32c, diffuse16, envbuf, flexbuffers, ids, json0, lexidecimal, sha384};

/// An arbitrary slice of length <= N.
macro_rules! any_slice {
    ($n:expr) => {{
        let arr: [u8; $n] = kani::any();
        let len: usize = kani::any();
        kani::assume(len <= $n);
        (arr, len)
    }};
}

// ---------------------------------------------------------------- Lexidecimal

#[kani::proof]
#[kani::unwind(18)]
fn lex_i64_roundtrip() {
    // bound: |n| < 2^15 (at most four payload digits)
    let n: i64 = kani::any();
    kani::assume(n >= -(1 << 15) && n < (1 << 15));
    let mut out = [0u8; 20];
    let len = lexidecimal::encode_i64_into(n, &mut out);
    assert_eq!(lexidecimal::decode_i64_bytes(&out[..len]), Ok(n));
}

#[kani::proof]
#[kani::unwind(10)]
fn lex_decode_canonical() {
    let (arr, len) = any_slice!(8);
    let s = &arr[..len];
    let mut d = [0u8; 8];
    if let Ok((neg, n)) = lexidecimal::decode_digits_into(s, &mut d) {
        let mut again = [0u8; 8];
        let m = lexidecimal::encode_digits_into(neg, &d[..n], &mut again).unwrap();
        assert!(&again[..m] == s);
    }
}

#[kani::proof]
#[kani::unwind(10)]
fn lex_digits_roundtrip() {
    let (arr, len) = any_slice!(5);
    let neg: bool = kani::any();
    let d = &arr[..len];
    let mut out = [0u8; 9];
    match lexidecimal::encode_digits_into(neg, d, &mut out) {
        Ok(m) => {
            let mut back = [0u8; 9];
            let (n2, l2) = lexidecimal::decode_digits_into(&out[..m], &mut back).unwrap();
            assert!(n2 == neg && &back[..l2] == d);
        }
        // rejected digit strings are exactly the empty, out-of-range or non-minimal ones
        Err(_) => assert!(len == 0 || (len > 1 && d[0] == 0) || d.iter().any(|&x| x > 15)),
    }
}

#[kani::proof]
#[kani::unwind(10)]
fn lex_rejects_nonminimal_payload() {
    // a leading zero payload digit (leading 'f' for negatives) with more
    // than one payload digit is never accepted
    let (arr, len) = any_slice!(7);
    let s = &arr[..len];
    kani::assume(len >= 5 && s[0] == b'0' && s[2] >= b'g' && s[2] <= b'v');
    let neg = s[1] == b'y';
    kani::assume(s[3] == if neg { b'f' } else { b'0' });
    let mut d = [0u8; 8];
    assert!(lexidecimal::decode_digits_into(s, &mut d).is_err());
}

#[kani::proof]
#[kani::unwind(18)]
fn lex_size_prefix_shape() {
    let size: u64 = kani::any();
    let neg: bool = kani::any();
    let mut out = Vec::with_capacity(17);
    lexidecimal::size_prefix_into(&mut out, size, neg);
    let w = lexidecimal::size_width(size) as usize;
    assert_eq!(out.len(), if w == 1 { 1 } else { w + 1 });
    // minimal width: the size does not fit half the width
    if w > 1 {
        assert!(size >> (4 * (w as u32 / 2)) != 0);
    }
}

// ------------------------------------------------------------------ Diffuse16

#[kani::proof]
#[kani::unwind(18)]
fn d16_i64_roundtrip() {
    // bound: |n| < 2^15 (at most four payload digits)
    let n: i64 = kani::any();
    kani::assume(n >= -(1 << 15) && n < (1 << 15));
    let mut out = [0u8; 19];
    let len = diffuse16::encode_i64_into(n, &mut out);
    assert_eq!(diffuse16::decode_i64_bytes(&out[..len]), Ok(n));
}

#[kani::proof]
#[kani::unwind(10)]
fn d16_decode_canonical() {
    let (arr, len) = any_slice!(8);
    let s = &arr[..len];
    let mut d = [0u8; 8];
    if let Ok((neg, n)) = diffuse16::decode_digits_into(s, &mut d) {
        let mut again = [0u8; 8];
        let m = diffuse16::encode_digits_into(neg, &d[..n], &mut again).unwrap();
        assert!(&again[..m] == s);
    }
}

#[kani::proof]
#[kani::unwind(10)]
fn d16_digits_roundtrip() {
    let (arr, len) = any_slice!(5);
    let neg: bool = kani::any();
    let d = &arr[..len];
    let mut out = [0u8; 8];
    match diffuse16::encode_digits_into(neg, d, &mut out) {
        Ok(m) => {
            let mut back = [0u8; 8];
            let (n2, l2) = diffuse16::decode_digits_into(&out[..m], &mut back).unwrap();
            assert!(n2 == neg && &back[..l2] == d);
        }
        Err(_) => assert!(len == 0 || (len > 1 && d[0] == 0) || d.iter().any(|&x| x > 15)),
    }
}

// --------------------------------------------------------------------- EnvBuf

// bound: feed of 16 arbitrary bytes (one minimal envelope); backward: 24 bytes
#[kani::proof]
#[kani::unwind(18)]
fn envbuf_feed_no_panic() {
    let (arr, len) = any_slice!(16);
    let _ = envbuf::parse_feed(&arr[..len]);
}

#[kani::proof]
#[kani::unwind(26)]
fn envbuf_backward_no_panic() {
    let (arr, len) = any_slice!(24);
    let end: usize = kani::any();
    let _ = envbuf::parse_backward(&arr[..len], end);
}

#[kani::proof]
#[kani::unwind(34)]
fn envbuf_decode_canonical() {
    let (arr, len) = any_slice!(32);
    let b = &arr[..len];
    if let Ok(env) = envbuf::decode_one(b) {
        let mut again = [0u8; 32];
        let n = envbuf::encode_to(&mut again, env.format, env.alignment_power, &b[env.message.clone()]).unwrap();
        assert!(&again[..n] == b);
        assert_eq!(envbuf::parse_backward(b, len), Ok(env));
    }
}

#[kani::proof]
#[kani::unwind(34)]
fn envbuf_roundtrip() {
    // bound: message <= 8 bytes, alignment power <= 4 (envelope <= 32 bytes)
    let (arr, len) = any_slice!(8);
    let msg = &arr[..len];
    let f: u8 = kani::any();
    let ap: u8 = kani::any();
    kani::assume(ap <= 4);
    let mut wire = [0u8; 32];
    match envbuf::encode_to(&mut wire, f, ap, msg) {
        Ok(n) => {
            assert!(matches!(f, 0 | 4 | 5 | 6));
            assert_eq!(n % (1 << ap), 0);
            let env = envbuf::decode_one(&wire[..n]).unwrap();
            assert_eq!(env.format, f);
            assert_eq!(env.alignment_power, ap);
            assert!(&wire[env.message.clone()] == msg);
        }
        Err(_) => assert!(!matches!(f, 0 | 4 | 5 | 6)),
    }
}

#[kani::proof]
#[kani::unwind(26)]
fn envbuf_margins() {
    let c: u64 = kani::any();
    let m = envbuf::counter_margin(c);
    let env = envbuf::decode_one(&m).unwrap();
    assert_eq!(envbuf::margin_counter(&m, &env), Ok(c));
    let s: u8 = kani::any();
    match envbuf::standalone_margin(s) {
        Ok(m) => {
            let env = envbuf::decode_one(&m).unwrap();
            assert_eq!(envbuf::margin_counter(&m, &env), Ok(s as u64));
        }
        Err(_) => assert!(s == 0 || s > 7),
    }
}

// --------------------------------------------------------- FlexBuffers ext.

// FlexBuffers extensions: proved on the allocation-free cores
// (`validate_extension`, `encode_extension_into`) that the Vec-returning API wraps.

// bound: 8 arbitrary bytes
#[kani::proof]
#[kani::unwind(10)]
fn flex_ext_decode_no_panic() {
    let (arr, len) = any_slice!(8);
    let _ = flexbuffers::validate_extension(&arr[..len]);
}

// bound: payload <= 3 bytes, any prefix byte
#[kani::proof]
#[kani::unwind(8)]
fn flex_ext_roundtrip() {
    let (arr, len) = any_slice!(3);
    let payload = &arr[..len];
    let prefix: u8 = kani::any();
    let mut out = [0u8; 56];
    if let Ok(n) = flexbuffers::encode_extension_into(prefix, payload, &mut out) {
        let v = flexbuffers::validate_extension(&out[..n]).unwrap();
        assert!(v.prefix == prefix && v.payload_len == len);
        let mut i = 0;
        while i < len {
            assert!(v.byte(&out[..n], i) == payload[i]);
            i += 1;
        }
    }
}

// bound: 8-byte buffers in the minimal layout (8-bit elements, 1-byte root)
#[kani::proof]
#[kani::unwind(10)]
fn flex_ext_decode_canonical_minimal_layout() {
    let (arr, len) = any_slice!(8);
    let b = &arr[..len];
    kani::assume(len >= 4 && b[len - 1] == 1 && b[len - 2] == 0x30);
    if let Ok(v) = flexbuffers::validate_extension(b) {
        // minimal layout: size byte, payload+1 elements, 1-byte root offset
        // pointing at the first element, packed type, root width
        let p = v.payload_len;
        if b[0] as usize == p + 1 && len == p + 5 && b[p + 2] as usize == p + 1 {
            let mut payload = [0u8; 8];
            let mut i = 0;
            while i < v.payload_len {
                payload[i] = v.byte(b, i);
                i += 1;
            }
            let mut out = [0u8; 96];
            let n = flexbuffers::encode_extension_into(v.prefix, &payload[..v.payload_len], &mut out).unwrap();
            assert!(&out[..n] == b);
        }
    }
}

// bound: magnitude <= 3 bytes
#[kani::proof]
#[kani::unwind(8)]
fn flex_ext_rejects_noncanonical_magnitude() {
    let (arr, len) = any_slice!(3);
    let payload = &arr[..len];
    let neg: bool = kani::any();
    let prefix = if neg { b'~' } else { b'x' };
    kani::assume(len == 0 || (len > 1 && payload[0] == 0));
    let mut out = [0u8; 56];
    assert!(flexbuffers::encode_extension_into(prefix, payload, &mut out).is_err());
    // the same elements laid out by hand are rejected by the decoder
    let mut wire = [0u8; 8];
    wire[0] = len as u8 + 1;
    wire[1] = prefix;
    let mut i = 0;
    while i < len {
        wire[2 + i] = payload[i];
        i += 1;
    }
    wire[2 + len] = len as u8 + 1;
    wire[3 + len] = 0x30;
    wire[4 + len] = 1;
    assert!(flexbuffers::validate_extension(&wire[..len + 5]).is_err());
}

#[kani::proof]
#[kani::unwind(6)]
fn flex_decode_no_panic() {
    // bound: 3 arbitrary bytes (a root triple: inline scalars or an offset)
    let (arr, len) = any_slice!(3);
    let _ = flexbuffers::decode(&arr[..len]);
}

// --------------------------------------------------------------------- CRC32C

#[kani::proof]
#[kani::unwind(9)]
fn crc_table_matches_bitwise() {
    let (arr, len) = any_slice!(4);
    let seed: u32 = kani::any();
    assert_eq!(crc32c::update(seed, &arr[..len]), crc32c::update_bitwise(seed, &arr[..len]));
}

#[kani::proof]
#[kani::unwind(5)]
fn crc_incremental() {
    let (a, la) = any_slice!(2);
    let (b, lb) = any_slice!(2);
    let mut whole = [0u8; 4];
    whole[..la].copy_from_slice(&a[..la]);
    whole[la..la + lb].copy_from_slice(&b[..lb]);
    assert_eq!(crc32c::update(crc32c::update(0, &a[..la]), &b[..lb]), crc32c::update(0, &whole[..la + lb]));
}

#[kani::proof]
#[kani::unwind(10)]
fn crc_hex8_parse() {
    let s: [u8; 8] = kani::any();
    if let Ok(v) = crc32c::parse_hex8_bytes(&s) {
        assert!(crc32c::hex8(v).as_bytes() == s);
    }
}

// -------------------------------------------------------------------- SHA-384

// bound: the 48-byte API runs the same per-byte step as these slice cores;
// prove the cores on 4-byte digests (8 hex digits) plus the length check.
#[kani::proof]
#[kani::unwind(10)]
fn sha_hex_parse_canonical() {
    let s: [u8; 8] = kani::any();
    let mut d = [0u8; 4];
    if sha384::decode_hex_into(&s, &mut d).is_ok() {
        let mut again = [0u8; 8];
        sha384::encode_hex_into(&d, &mut again);
        assert!(again == s);
    }
}

#[kani::proof]
#[kani::unwind(6)]
fn sha_hex_roundtrip() {
    let d: [u8; 4] = kani::any();
    let mut s = [0u8; 8];
    sha384::encode_hex_into(&d, &mut s);
    let mut back = [0u8; 4];
    assert!(sha384::decode_hex_into(&s, &mut back).is_ok());
    assert!(back == d);
}

#[kani::proof]
#[kani::unwind(3)]
fn sha_hex_rejects_wrong_length() {
    let (arr, len) = any_slice!(2);
    assert!(sha384::parse_hex_bytes(&arr[..len]).is_err());
}

// -------------------------------------------------------------------- Flow ids

#[kani::proof]
#[kani::unwind(7)]
fn flow_id_normalize() {
    let (arr, len) = any_slice!(5);
    let s = &arr[..len];
    match ids::normalize_flow_id_bytes(s) {
        Ok(n) => {
            assert_eq!(n.len(), len);
            assert!(n.iter().all(|c| matches!(c, b'a'..=b'z' | b'0'..=b'9' | b'.' | b'_' | b'-')));
            assert_eq!(ids::normalize_flow_id_bytes(&n), Ok(n.clone()));
        }
        Err(_) => {
            assert!(
                len == 0
                    || s == b"."
                    || s == b".."
                    || s.iter().any(|c| !matches!(c, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'.' | b'_' | b'-'))
            );
        }
    }
}

// ------------------------------------------------------------- JSON0 prefixes

#[kani::proof]
#[kani::unwind(7)]
fn json0_literal_escape() {
    let (arr, len) = any_slice!(5);
    let s = &arr[..len];
    let _ = json0::classify(s);
    let w = json0::escape_literal(s);
    match json0::classify(&w) {
        Ok(json0::Prefix::Plain) => assert!(w.as_slice() == s),
        Ok(json0::Prefix::Literal) => assert!(&w[1..] == s),
        _ => panic!("escaped literal must read back as a literal"),
    }
}


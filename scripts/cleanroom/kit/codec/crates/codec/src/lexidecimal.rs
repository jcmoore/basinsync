//! Lexidecimal: doubly size-prefixed, big-endian radix-16 integer text whose
//! lexicographic order follows numeric order (docs/glossary/lexidecimal.md).
//!
//! ```text
//! 0 -> 0zg0, 16 -> 0zh10, -1 -> 0yvf, -17 -> 0yuef,
//! 2^64 -> 0zwhg10000000000000000; a size of 0x100 is spelled xghgg
//! ```
//!
//! The core ([`encode_digits`], [`decode_digits`], [`size_prefix_into`]) works
//! on nibble slices and is what the Kani proofs verify; the `BigInt` API wraps
//! it.

use crate::bigint::{join_signed, split_signed, u64_nibbles};
use crate::error::{err, Result};
use num_bigint::BigInt;

const HEX: &[u8; 16] = b"0123456789abcdef";
/// Magnitude letters for widths 2, 4, 8, 16 (m = 0..3).
const POS_MAG: &[u8; 4] = b"wxyz";
const NEG_MAG: &[u8; 4] = b"ZYXW";

/// Number of hex digits needed for `s` (1 for zero).
fn hex_len(s: u64) -> u32 {
    if s == 0 {
        1
    } else {
        (64 - s.leading_zeros() + 3) / 4
    }
}

/// Size width (1, 2, 4, 8 or 16 digits) for size value `s`.
pub fn size_width(s: u64) -> u32 {
    let d = hex_len(s);
    let mut w = 1;
    while w < d {
        w *= 2;
    }
    w
}

/// Append the size prefix for size value `s` (= payload digits - 1): the
/// optional magnitude letter followed by the fixed-width anti-hex size digits.
/// The `0z`/`0y` sign prefix is not included.
pub fn size_prefix_into(out: &mut Vec<u8>, s: u64, negative: bool) {
    let width = size_width(s);
    if width > 1 {
        let m = width.trailing_zeros() as usize - 1; // 1 << (1 + m) == width
        out.push(if negative { NEG_MAG[m] } else { POS_MAG[m] });
    }
    let mut i = width;
    while i > 0 {
        i -= 1;
        let d = if i >= 16 { 0 } else { ((s >> (4 * i)) & 0xf) as u8 };
        out.push(if negative { b'v' - d } else { b'g' + d });
    }
}

/// The size prefix text for size value `size` (magnitude letter, if any, and
/// size digits; no sign prefix). `size_prefix(0x100, false) == "xghgg"`.
pub fn size_prefix(size: u64, negative: bool) -> String {
    let mut out = Vec::with_capacity(17);
    size_prefix_into(&mut out, size, negative);
    String::from_utf8(out).expect("ASCII")
}

/// Allocation-free encoder core: writes the spelling into `out` and returns
/// its length. `nibbles` are the minimal big-endian digits (0..=15) of `n`
/// (when `negative` is false) or of `~n` (when `negative` is true).
pub fn encode_digits_into(negative: bool, nibbles: &[u8], out: &mut [u8]) -> core::result::Result<usize, &'static str> {
    if nibbles.is_empty() {
        return Err("lexidecimal: empty digit string");
    }
    if nibbles.len() > 1 && nibbles[0] == 0 {
        return Err("lexidecimal: non-minimal digits");
    }
    let s = (nibbles.len() - 1) as u64;
    let width = size_width(s) as usize;
    let total = 2 + width + (width > 1) as usize + nibbles.len();
    if out.len() < total {
        return Err("lexidecimal: output buffer too small");
    }
    out[0] = b'0';
    out[1] = if negative { b'y' } else { b'z' };
    let mut i = 2;
    if width > 1 {
        let m = width.trailing_zeros() as usize - 1; // 1 << (1 + m) == width
        out[i] = if negative { NEG_MAG[m] } else { POS_MAG[m] };
        i += 1;
    }
    let mut k = width;
    while k > 0 {
        k -= 1;
        let d = if k >= 16 { 0 } else { ((s >> (4 * k)) & 0xf) as u8 };
        out[i] = if negative { b'v' - d } else { b'g' + d };
        i += 1;
    }
    for &d in nibbles {
        if d > 15 {
            return Err("lexidecimal: digit out of range");
        }
        out[i] = HEX[(if negative { 15 - d } else { d }) as usize];
        i += 1;
    }
    Ok(i)
}

/// Encode from minimal big-endian nibbles (values 0..=15) of `n` (when
/// `negative` is false) or of `~n` (when `negative` is true). Rejects empty,
/// out-of-range, or non-minimal digit slices.
pub fn encode_digits(negative: bool, nibbles: &[u8]) -> Result<Vec<u8>> {
    let mut out = vec![0u8; nibbles.len() + 20];
    let n = encode_digits_into(negative, nibbles, &mut out).map_err(crate::CodecError::new)?;
    out.truncate(n);
    Ok(out)
}

fn hex_val(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        _ => None,
    }
}

/// Allocation-free decoder core: validates a canonical spelling and writes
/// the minimal big-endian nibbles of `n` or `~n` into `out`, returning
/// `(negative, digit count)`.
pub fn decode_digits_into(s: &[u8], out: &mut [u8]) -> core::result::Result<(bool, usize), &'static str> {
    if s.len() < 4 || s[0] != b'0' {
        return Err("lexidecimal: missing 0z/0y prefix");
    }
    let negative = match s[1] {
        b'z' => false,
        b'y' => true,
        _ => return Err("lexidecimal: missing 0z/0y prefix"),
    };
    let mut i = 2;
    let c = s[i];
    let width: usize = if (b'g'..=b'v').contains(&c) {
        1
    } else {
        let m = if negative {
            if (b'W'..=b'Z').contains(&c) {
                b'Z' - c
            } else {
                return Err("lexidecimal: invalid size or magnitude");
            }
        } else if (b'w'..=b'z').contains(&c) {
            c - b'w'
        } else {
            return Err("lexidecimal: invalid size or magnitude");
        };
        i += 1;
        2usize << m
    };
    if s.len() < i + width {
        return Err("lexidecimal: truncated size");
    }
    let mut size: u64 = 0;
    for &c in &s[i..i + width] {
        if !(b'g'..=b'v').contains(&c) {
            return Err("lexidecimal: invalid size digit");
        }
        let d = if negative { b'v' - c } else { c - b'g' };
        size = (size << 4) | d as u64;
    }
    i += width;
    if width > 1 {
        // The smallest permitted width: a smaller width must not suffice.
        let half_bits = 4 * (width as u32 / 2);
        if size >> half_bits == 0 {
            return Err("lexidecimal: size width is not minimal");
        }
    }
    let payload = &s[i..];
    match size.checked_add(1) {
        Some(n) if n == payload.len() as u64 => {}
        _ => return Err("lexidecimal: payload length does not match size"),
    }
    if out.len() < payload.len() {
        return Err("lexidecimal: output buffer too small");
    }
    for (k, &c) in payload.iter().enumerate() {
        let v = match hex_val(c) {
            Some(v) => v,
            None => return Err("lexidecimal: invalid payload digit"),
        };
        out[k] = if negative { 15 - v } else { v };
    }
    if payload.len() > 1 && out[0] == 0 {
        return Err("lexidecimal: payload is not minimal");
    }
    Ok((negative, payload.len()))
}

/// Decode a canonical Lexidecimal spelling into `(negative, nibbles)` where
/// nibbles are the minimal big-endian digits of `n` or `~n`. Every
/// noncanonical spelling is rejected, so `encode_digits` of the result
/// reproduces the input exactly.
pub fn decode_digits(s: &[u8]) -> Result<(bool, Vec<u8>)> {
    let mut out = vec![0u8; s.len()];
    let (neg, n) = decode_digits_into(s, &mut out).map_err(crate::CodecError::new)?;
    out.truncate(n);
    Ok((neg, out))
}

/// Encode an arbitrary-precision integer.
pub fn encode(n: &BigInt) -> String {
    let (neg, nibbles) = split_signed(n);
    String::from_utf8(encode_digits(neg, &nibbles).expect("minimal digits")).expect("ASCII")
}

/// Decode canonical Lexidecimal text; rejects noncanonical spellings.
pub fn decode(s: &str) -> Result<BigInt> {
    let (neg, nibbles) = decode_digits(s.as_bytes())?;
    Ok(join_signed(neg, &nibbles))
}

/// Encode `lex(~n)`, the complemented form used for descending keys.
pub fn encode_complement(n: &BigInt) -> String {
    encode(&crate::bigint::complement(n))
}

/// Encode a `u64` without allocating a `BigInt`.
pub fn encode_u64(n: u64) -> String {
    let mut d = [0u8; 16];
    let len = u64_nibbles(n, &mut d);
    String::from_utf8(encode_digits(false, &d[..len]).expect("minimal")).expect("ASCII")
}

/// Encode an `i64` without allocating a `BigInt`.
pub fn encode_i64(n: i64) -> String {
    String::from_utf8(encode_i64_bytes(n)).expect("ASCII")
}

/// Byte-level `i64` encoder (Kani-friendly).
pub fn encode_i64_bytes(n: i64) -> Vec<u8> {
    let mut out = [0u8; 20];
    let len = encode_i64_into(n, &mut out);
    out[..len].to_vec()
}

/// Allocation-free `i64` encoder; returns the length written (at most 20).
pub fn encode_i64_into(n: i64, out: &mut [u8; 20]) -> usize {
    let (neg, mag) = if n < 0 { (true, !n as u64) } else { (false, n as u64) };
    let mut d = [0u8; 16];
    let len = u64_nibbles(mag, &mut d);
    encode_digits_into(neg, &d[..len], out).expect("minimal")
}

/// Decode into an `i64`; errors if out of range.
pub fn decode_i64(s: &str) -> Result<i64> {
    decode_i64_bytes(s.as_bytes()).map_err(crate::CodecError::new)
}

/// Allocation-free `i64` decoder (Kani-friendly).
pub fn decode_i64_bytes(s: &[u8]) -> core::result::Result<i64, &'static str> {
    let mut d = [0u8; 16];
    let (neg, len) = match decode_digits_into(s, &mut d) {
        Ok(r) => r,
        Err("lexidecimal: output buffer too small") => return Err("lexidecimal: out of i64 range"),
        Err(e) => return Err(e),
    };
    let mut m: u64 = 0;
    for &x in &d[..len] {
        m = (m << 4) | x as u64;
    }
    if m > i64::MAX as u64 {
        return Err("lexidecimal: out of i64 range");
    }
    Ok(if neg { !(m as i64) } else { m as i64 })
}

/// Decode a nonnegative value into a `u64`; errors if negative or too large.
pub fn decode_u64(s: &str) -> Result<u64> {
    let (neg, nibbles) = decode_digits(s.as_bytes())?;
    if neg {
        return err("lexidecimal: negative value where nonnegative expected");
    }
    if nibbles.len() > 16 {
        return err("lexidecimal: out of u64 range");
    }
    Ok(nibbles.iter().fold(0u64, |m, &d| (m << 4) | d as u64))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bigint::parse_decimal;
    use num_bigint::BigInt;
    use proptest::prelude::*;

    fn b(s: &str) -> BigInt {
        parse_decimal(s).unwrap()
    }

    #[test]
    fn doc_examples() {
        assert_eq!(encode(&b("0")), "0zg0");
        assert_eq!(encode(&b("16")), "0zh10");
        assert_eq!(encode(&b("-1")), "0yvf");
        assert_eq!(encode(&b("-17")), "0yuef");
        assert_eq!(encode(&b("18446744073709551616")), "0zwhg10000000000000000");
        assert_eq!(size_prefix(0x100, false), "xghgg");
        for (t, v) in [("0zg0", "0"), ("0zh10", "16"), ("0yvf", "-1"), ("0yuef", "-17"),
                       ("0zwhg10000000000000000", "18446744073709551616")] {
            assert_eq!(decode(t).unwrap(), b(v));
        }
    }

    #[test]
    fn size_prefixes() {
        assert_eq!(size_prefix(0, false), "g");
        assert_eq!(size_prefix(15, false), "v");
        assert_eq!(size_prefix(16, false), "whg");
        assert_eq!(size_prefix(0xff, false), "wvv");
        assert_eq!(size_prefix(0xffff, false), "xvvvv");
        assert_eq!(size_prefix(0x10000, false), "yggghgggg");
        assert_eq!(size_prefix(0x1_0000_0000, false), "zggggggghgggggggg");
        assert_eq!(size_prefix(0, true), "v");
        assert_eq!(size_prefix(16, true), "Zuv");
        assert_eq!(size_prefix(0x100, true), "Yvuvv");
        assert_eq!(size_prefix(u64::MAX, false), "zvvvvvvvvvvvvvvvv");
    }

    #[test]
    fn small_values() {
        assert_eq!(encode_u64(1), "0zg1");
        assert_eq!(encode_u64(15), "0zgf");
        assert_eq!(encode_u64(255), "0zhff");
        assert_eq!(encode_i64(-16), "0yv0");
        assert_eq!(encode_i64(i64::MIN), encode(&BigInt::from(i64::MIN)));
        assert_eq!(decode_i64(&encode_i64(i64::MIN)).unwrap(), i64::MIN);
        assert_eq!(decode_u64(&encode_u64(u64::MAX)).unwrap(), u64::MAX);
        assert!(decode_u64("0yvf").is_err());
    }

    #[test]
    fn rejects_noncanonical() {
        for bad in [
            "", "0", "0z", "0zg", "0zg00", "0zh01", "0zh1", "0zg10", "0zgA", "0zG0", "0zwgh10",
            "0zwgf0123456789abcdef", // width 2 used for a size that fits width 1
            "0yvf0", "0yuff", // non-minimal complemented payload (~n = 0x0e)
            "0yv0", // fine? (~n = 15 -> n = -16) checked below as valid
            "1zg0", "0xg0", "0zwhg1", "0zw", "0zxghgg", "0Zg0", "0yZvv", "0zg0 ", "0zz",
        ] {
            if bad == "0yv0" {
                assert_eq!(decode(bad).unwrap(), BigInt::from(-16));
                continue;
            }
            assert!(decode(bad).is_err(), "{bad:?} accepted");
        }
    }

    #[test]
    fn ordering_across_widths() {
        let vals: Vec<BigInt> = [
            "-340282366920938463463374607431768211456", "-18446744073709551617",
            "-18446744073709551616", "-4096", "-257", "-256", "-17", "-16", "-2", "-1", "0", "1",
            "15", "16", "255", "256", "18446744073709551615", "18446744073709551616",
            "340282366920938463463374607431768211456",
        ].iter().map(|s| b(s)).collect();
        let enc: Vec<String> = vals.iter().map(encode).collect();
        let mut sorted = enc.clone();
        sorted.sort();
        assert_eq!(enc, sorted);
    }

    proptest! {
        #[test]
        fn roundtrip_i128(n in any::<i128>()) {
            let v = BigInt::from(n);
            let s = encode(&v);
            prop_assert_eq!(decode(&s).unwrap(), v);
        }

        #[test]
        fn order_preserving(a in any::<i128>(), c in any::<i128>()) {
            let (ea, ec) = (encode(&BigInt::from(a)), encode(&BigInt::from(c)));
            prop_assert_eq!(a.cmp(&c), ea.cmp(&ec));
        }

        #[test]
        fn roundtrip_big(digits in "[1-9a-f][0-9a-f]{0,300}", neg in any::<bool>()) {
            let m = BigInt::parse_bytes(digits.as_bytes(), 16).unwrap();
            let v = if neg { -m } else { m };
            prop_assert_eq!(decode(&encode(&v)).unwrap(), v);
        }

        #[test]
        fn canonical_or_rejected(s in "0[yz][g-vw-zW-Z][g-v0-9a-f]{0,6}") {
            if let Ok(v) = decode(&s) {
                prop_assert_eq!(encode(&v), s);
            }
        }
    }
}

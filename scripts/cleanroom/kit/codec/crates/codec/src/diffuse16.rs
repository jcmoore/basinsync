//! Diffuse16: bucket-safe little-endian radix-16 integer text
//! (docs/glossary/diffuse16.md).
//!
//! ```text
//! 0 -> 0w0z, 16 -> 0w01z, 27 -> 0wb1z, -1 -> 0wfy, -16 -> 0w0y
//! ```

use crate::bigint::{join_signed, split_signed, u64_nibbles};
use crate::error::Result;
use num_bigint::BigInt;

const HEX: &[u8; 16] = b"0123456789abcdef";

/// Allocation-free encoder core; returns the length written to `out`.
pub fn encode_digits_into(negative: bool, nibbles: &[u8], out: &mut [u8]) -> core::result::Result<usize, &'static str> {
    if nibbles.is_empty() {
        return Err("diffuse16: empty digit string");
    }
    if nibbles.len() > 1 && nibbles[0] == 0 {
        return Err("diffuse16: non-minimal digits");
    }
    let total = nibbles.len() + 3;
    if out.len() < total {
        return Err("diffuse16: output buffer too small");
    }
    out[0] = b'0';
    out[1] = b'w';
    let n = nibbles.len();
    for (k, &d) in nibbles.iter().enumerate() {
        if d > 15 {
            return Err("diffuse16: digit out of range");
        }
        out[2 + n - 1 - k] = HEX[(if negative { 15 - d } else { d }) as usize];
    }
    out[total - 1] = if negative { b'y' } else { b'z' };
    Ok(total)
}

/// Encode from minimal big-endian nibbles of `n` (nonnegative) or `~n`
/// (negative). Rejects empty, out-of-range or non-minimal digits.
pub fn encode_digits(negative: bool, nibbles: &[u8]) -> Result<Vec<u8>> {
    let mut out = vec![0u8; nibbles.len() + 3];
    let n = encode_digits_into(negative, nibbles, &mut out).map_err(crate::CodecError::new)?;
    out.truncate(n);
    Ok(out)
}

/// Allocation-free decoder core: writes big-endian nibbles of `n` or `~n`
/// into `out` and returns `(negative, digit count)`. Rejects uppercase,
/// empty payloads, wrong prefix/suffix and non-minimal digit strings.
pub fn decode_digits_into(s: &[u8], out: &mut [u8]) -> core::result::Result<(bool, usize), &'static str> {
    if s.len() < 4 || s[0] != b'0' || s[1] != b'w' {
        return Err("diffuse16: missing 0w prefix or empty payload");
    }
    let negative = match s[s.len() - 1] {
        b'z' => false,
        b'y' => true,
        _ => return Err("diffuse16: missing z/y suffix"),
    };
    let payload = &s[2..s.len() - 1];
    let n = payload.len();
    if out.len() < n {
        return Err("diffuse16: output buffer too small");
    }
    for (k, &c) in payload.iter().enumerate() {
        let v = match c {
            b'0'..=b'9' => c - b'0',
            b'a'..=b'f' => c - b'a' + 10,
            _ => return Err("diffuse16: invalid digit"),
        };
        out[n - 1 - k] = if negative { 15 - v } else { v };
    }
    if n > 1 && out[0] == 0 {
        return Err("diffuse16: non-minimal digits");
    }
    Ok((negative, n))
}

/// Decode canonical Diffuse16 into `(negative, big-endian nibbles of n or ~n)`;
/// `encode_digits` reproduces any accepted input.
pub fn decode_digits(s: &[u8]) -> Result<(bool, Vec<u8>)> {
    let mut out = vec![0u8; s.len()];
    let (neg, n) = decode_digits_into(s, &mut out).map_err(crate::CodecError::new)?;
    out.truncate(n);
    Ok((neg, out))
}

pub fn encode(n: &BigInt) -> String {
    let (neg, nibbles) = split_signed(n);
    String::from_utf8(encode_digits(neg, &nibbles).expect("minimal")).expect("ASCII")
}

pub fn decode(s: &str) -> Result<BigInt> {
    let (neg, nibbles) = decode_digits(s.as_bytes())?;
    Ok(join_signed(neg, &nibbles))
}

/// Byte-level `i64` encoder.
pub fn encode_i64_bytes(n: i64) -> Vec<u8> {
    let mut out = [0u8; 19];
    let len = encode_i64_into(n, &mut out);
    out[..len].to_vec()
}

/// Allocation-free `i64` encoder; returns the length written (at most 19).
pub fn encode_i64_into(n: i64, out: &mut [u8; 19]) -> usize {
    let (neg, mag) = if n < 0 { (true, !n as u64) } else { (false, n as u64) };
    let mut d = [0u8; 16];
    let len = u64_nibbles(mag, &mut d);
    encode_digits_into(neg, &d[..len], out).expect("minimal")
}

/// Allocation-free `i64` decoder.
pub fn decode_i64_bytes(s: &[u8]) -> core::result::Result<i64, &'static str> {
    let mut d = [0u8; 16];
    let (neg, len) = match decode_digits_into(s, &mut d) {
        Ok(r) => r,
        Err("diffuse16: output buffer too small") => return Err("diffuse16: out of i64 range"),
        Err(e) => return Err(e),
    };
    let m = d[..len].iter().fold(0u64, |m, &x| (m << 4) | x as u64);
    if m > i64::MAX as u64 {
        return Err("diffuse16: out of i64 range");
    }
    Ok(if neg { !(m as i64) } else { m as i64 })
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn doc_examples() {
        for (v, t) in [(0i64, "0w0z"), (16, "0w01z"), (27, "0wb1z"), (-1, "0wfy"), (-16, "0w0y")] {
            assert_eq!(encode(&BigInt::from(v)), t);
            assert_eq!(decode(t).unwrap(), BigInt::from(v));
            assert_eq!(encode_i64_bytes(v), t.as_bytes());
            assert_eq!(decode_i64_bytes(t.as_bytes()).unwrap(), v);
        }
    }

    #[test]
    fn rejects() {
        for bad in ["", "0w", "0wz", "0wy", "0w0", "0w00z", "0w10z", "0wffy", "0wB1z", "0Wb1z",
                    "0wb1Z", "0wg1z", "1wb1z", "0w0x", "0wb1z "] {
            assert!(decode(bad).is_err(), "{bad:?} accepted");
        }
        // -17: ~n = 16 = "10" -> complement "ef" -> reversed "fe"
        assert_eq!(encode(&BigInt::from(-17)), "0wfey");
        assert_eq!(decode("0wfey").unwrap(), BigInt::from(-17));
    }

    proptest! {
        #[test]
        fn roundtrip(n in any::<i128>()) {
            let v = BigInt::from(n);
            prop_assert_eq!(decode(&encode(&v)).unwrap(), v);
        }
        #[test]
        fn canonical_or_rejected(s in "0w[0-9a-g]{0,5}[yz]") {
            if let Ok(v) = decode(&s) { prop_assert_eq!(encode(&v), s); }
        }
    }
}

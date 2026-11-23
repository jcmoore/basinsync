//! Base64 (RFC 4648) in the standard and URL-safe alphabets, used by JSON0
//! `0%` and `0*` strings.

use crate::error::{err, Result};

const STD: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
const URL: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

fn enc(data: &[u8], alpha: &[u8; 64], pad: bool) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        let chars = chunk.len() + 1;
        for i in 0..4 {
            if i < chars {
                out.push(alpha[((n >> (18 - 6 * i)) & 63) as usize] as char);
            } else if pad {
                out.push('=');
            }
        }
    }
    out
}

/// URL-safe alphabet without padding (JSON0 `0*` writers).
pub fn encode_url(data: &[u8]) -> String {
    enc(data, URL, false)
}

/// Standard alphabet with padding.
pub fn encode_std(data: &[u8]) -> String {
    enc(data, STD, true)
}

fn val(c: u8, url: bool) -> Option<u8> {
    match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        b'-' if url => Some(62),
        b'_' if url => Some(63),
        b'+' if !url => Some(62),
        b'/' if !url => Some(63),
        _ => None,
    }
}

/// Decode with optional (but then complete) `=` padding. Rejects characters
/// outside the alphabet, impossible lengths and nonzero trailing bits.
pub fn decode(s: &[u8], url: bool) -> Result<Vec<u8>> {
    let body_len = s.iter().position(|&c| c == b'=').unwrap_or(s.len());
    let (body, pad) = s.split_at(body_len);
    if !pad.iter().all(|&c| c == b'=') {
        return err("base64: data after padding");
    }
    if !pad.is_empty() && (s.len() % 4 != 0 || pad.len() > 2) {
        return err("base64: invalid padding");
    }
    if body.len() % 4 == 1 {
        return err("base64: invalid length");
    }
    let mut out = Vec::with_capacity(body.len() * 3 / 4);
    let mut acc: u32 = 0;
    let mut bits = 0;
    for &c in body {
        let v = val(c, url).ok_or_else(|| crate::CodecError::new("base64: invalid character"))?;
        acc = (acc << 6) | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    if acc != 0 {
        return err("base64: nonzero trailing bits");
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vectors() {
        for (d, s) in [("", ""), ("f", "Zg=="), ("fo", "Zm8="), ("foo", "Zm9v"), ("foob", "Zm9vYg=="),
                       ("fooba", "Zm9vYmE="), ("foobar", "Zm9vYmFy")] {
            assert_eq!(encode_std(d.as_bytes()), s);
            assert_eq!(decode(s.as_bytes(), false).unwrap(), d.as_bytes());
            let u = s.trim_end_matches('=');
            assert_eq!(encode_url(d.as_bytes()), u);
            assert_eq!(decode(u.as_bytes(), true).unwrap(), d.as_bytes());
        }
        assert_eq!(encode_url(&[0xfb, 0xff]), "-_8");
        assert_eq!(encode_std(&[0xfb, 0xff]), "+/8=");
        assert!(decode(b"-_8", false).is_err());
        assert!(decode(b"+/8", true).is_err());
        assert!(decode(b"Zg=", false).is_err());
        assert!(decode(b"Zh", false).is_err()); // trailing bits
        assert!(decode(b"Z", false).is_err());
        assert!(decode(b"Zg==Zg==", false).is_err());
    }
}

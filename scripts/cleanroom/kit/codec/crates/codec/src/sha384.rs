//! SHA-384 digests and their 96-digit lowercase hexadecimal rendering
//! (docs/glossary/sha384.md).

use crate::error::{err, Result};
use sha2::{Digest, Sha384};

pub const DIGEST_LEN: usize = 48;

pub fn digest(data: &[u8]) -> [u8; 48] {
    Sha384::digest(data).into()
}

/// Lowercase hexadecimal with leading zeros preserved.
pub fn hex(d: &[u8]) -> String {
    let mut v = vec![0u8; d.len() * 2];
    encode_hex_into(d, &mut v);
    String::from_utf8(v).expect("hex digits are ASCII")
}

/// Lowercase hex of `d` into `out` (`out.len() == 2 * d.len()`).
pub fn encode_hex_into(d: &[u8], out: &mut [u8]) {
    const H: &[u8; 16] = b"0123456789abcdef";
    for (i, &b) in d.iter().enumerate() {
        out[2 * i] = H[(b >> 4) as usize];
        out[2 * i + 1] = H[(b & 0xf) as usize];
    }
}

/// Parse lowercase hex into `out`; requires `s.len() == 2 * out.len()`.
pub fn decode_hex_into(s: &[u8], out: &mut [u8]) -> Result<()> {
    if s.len() != 2 * out.len() {
        return err("sha384: hexadecimal length mismatch");
    }
    for i in 0..out.len() {
        let hi = nib(s[2 * i])?;
        let lo = nib(s[2 * i + 1])?;
        out[i] = hi << 4 | lo;
    }
    Ok(())
}

/// Parse exactly 96 lowercase hexadecimal digits.
pub fn parse_hex(s: &str) -> Result<[u8; 48]> {
    parse_hex_bytes(s.as_bytes())
}

pub fn parse_hex_bytes(s: &[u8]) -> Result<[u8; 48]> {
    if s.len() != 96 {
        return err("sha384: expected 96 lowercase hexadecimal digits");
    }
    let mut out = [0u8; 48];
    decode_hex_into(s, &mut out)?;
    Ok(out)
}

fn nib(c: u8) -> Result<u8> {
    match c {
        b'0'..=b'9' => Ok(c - b'0'),
        b'a'..=b'f' => Ok(c - b'a' + 10),
        _ => err("sha384: expected 96 lowercase hexadecimal digits"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known() {
        assert_eq!(
            hex(&digest(b"abc")),
            "cb00753f45a35e8bb5a03d699ac65007272c32ab0eded1631a8b605a43ff5bed8086072ba1e7cc2358baeca134c825a7"
        );
        assert_eq!(
            hex(&digest(b"")),
            "38b060a751ac96384cd9327eb1b1e36a21fdb71114be07434c0cc7bf63f6e1da274edebfe76f65fbd51ad2f14898b95b"
        );
        let h = hex(&digest(b"abc"));
        assert_eq!(parse_hex(&h).unwrap(), digest(b"abc"));
        assert!(parse_hex(&h.to_uppercase()).is_err());
        assert!(parse_hex(&h[1..]).is_err());
        assert!(parse_hex(&format!("{h}0")).is_err());
        assert_eq!(hex(&[0, 1, 0xab]), "0001ab");
    }
}

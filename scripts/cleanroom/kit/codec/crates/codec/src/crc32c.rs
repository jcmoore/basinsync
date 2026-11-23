//! CRC32C (Castagnoli), incremental like `crc32c::crc32c_append`
//! (docs/glossary/crc32c.md). Table-driven; equivalence with the bitwise
//! reference is checked by unit tests and Kani.

use crate::error::{err, Result};

/// Reflected Castagnoli polynomial.
pub const POLY: u32 = 0x82f6_3b78;

const fn make_table() -> [u32; 256] {
    let mut t = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut c = i as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 { (c >> 1) ^ POLY } else { c >> 1 };
            k += 1;
        }
        t[i] = c;
        i += 1;
    }
    t
}

pub static TABLE: [u32; 256] = make_table();

/// Continue a cumulative checksum: `update(0, b"") == 0`,
/// `update(update(0, a), b) == update(0, a ++ b)`.
pub fn update(crc: u32, data: &[u8]) -> u32 {
    let mut c = !crc;
    for &b in data {
        c = TABLE[((c ^ b as u32) & 0xff) as usize] ^ (c >> 8);
    }
    !c
}

/// Bitwise reference implementation (slow; for verification).
pub fn update_bitwise(crc: u32, data: &[u8]) -> u32 {
    let mut c = !crc;
    for &b in data {
        c ^= b as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { (c >> 1) ^ POLY } else { c >> 1 };
        }
    }
    !c
}

/// Eight lowercase hexadecimal digits.
pub fn hex8(crc: u32) -> String {
    format!("{crc:08x}")
}

/// Parse exactly eight lowercase hexadecimal digits.
pub fn parse_hex8(s: &str) -> Result<u32> {
    parse_hex8_bytes(s.as_bytes())
}

pub fn parse_hex8_bytes(s: &[u8]) -> Result<u32> {
    if s.len() != 8 {
        return err("crc32c: expected eight lowercase hexadecimal digits");
    }
    let mut v = 0u32;
    for &c in s {
        let d = match c {
            b'0'..=b'9' => c - b'0',
            b'a'..=b'f' => c - b'a' + 10,
            _ => return err("crc32c: expected eight lowercase hexadecimal digits"),
        };
        v = (v << 4) | d as u32;
    }
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn check_value() {
        assert_eq!(update(0, b"123456789"), 0xe306_9283);
        assert_eq!(update(0, b""), 0);
        assert_eq!(hex8(0), "00000000");
        assert_eq!(hex8(0xe3069283), "e3069283");
        assert_eq!(parse_hex8("e3069283").unwrap(), 0xe3069283);
        for bad in ["E3069283", "e306928", "e30692830", "e306928g", " 3069283"] {
            assert!(parse_hex8(bad).is_err());
        }
        // RFC 3720 test vector: 32 bytes of zeros
        assert_eq!(update(0, &[0u8; 32]), 0x8a91_36aa);
        assert_eq!(update(0, &[0xffu8; 32]), 0x62a8_ab43);
    }

    proptest! {
        #[test]
        fn table_matches_bitwise(seed in any::<u32>(), d in proptest::collection::vec(any::<u8>(), 0..64)) {
            prop_assert_eq!(update(seed, &d), update_bitwise(seed, &d));
        }
        #[test]
        fn incremental(a in proptest::collection::vec(any::<u8>(), 0..64), b in proptest::collection::vec(any::<u8>(), 0..64)) {
            let whole = [a.clone(), b.clone()].concat();
            prop_assert_eq!(update(update(0, &a), &b), update(0, &whole));
        }
    }
}

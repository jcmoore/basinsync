//! CRC32C over the logical coordinate bytes, independent of transport margins.
use anyhow::{Context, Result, ensure};
use axum::http::HeaderMap;
use std::collections::HashMap;

pub fn append(before: u32, bytes: &[u8]) -> u32 {
    crc32c::crc32c_append(before, bytes)
}
pub fn event(before: u32, bytes: &[u8]) -> Result<u32> {
    Ok(append(before, crate::event::coordinate_bytes(bytes)?))
}

pub fn format(crc: u32) -> String {
    format!("{crc:08x}")
}
pub fn parse(text: &str) -> Result<u32> {
    ensure!(
        text.len() == 8
            && text
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "invalid CRC32C"
    );
    Ok(u32::from_str_radix(text, 16)?)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Run {
    pub before: u32,
    pub after: u32,
}
impl Run {
    pub fn headers(self, headers: &mut HeaderMap) -> Result<()> {
        headers.insert("x-amz-meta-base-crc32c", format(self.before).parse()?);
        headers.insert("x-amz-meta-crc32c", format(self.after).parse()?);
        Ok(())
    }
    pub fn from_headers(headers: &HashMap<String, String>) -> Result<Option<Self>> {
        let field = |name: &str| {
            headers
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(name))
                .map(|(_, v)| v.as_str())
        };
        let before = field("x-amz-meta-base-crc32c");
        let after = field("x-amz-meta-crc32c");
        if before.is_none() && after.is_none() {
            return Ok(None);
        }
        Ok(Some(Self {
            before: parse(before.context("missing CRC32C base")?)?,
            after: parse(after.context("missing CRC32C")?)?,
        }))
    }
    pub fn verify(self, bytes: &[u8]) -> Result<()> {
        ensure!(
            append(self.before, bytes) == self.after,
            "coordinate CRC32C mismatch"
        );
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn known_vectors_incremental_and_canonical_text() -> Result<()> {
        assert_eq!(append(0, b""), 0);
        assert_eq!(format(append(0, b"123456789")), "e3069283");
        for n in 0..=9 {
            assert_eq!(
                append(append(0, &b"123456789"[..n]), &b"123456789"[n..]),
                append(0, b"123456789")
            );
        }
        for bad in ["0", "E3069283", "e30692830", "+1234567", "zzzzzzzz"] {
            assert!(parse(bad).is_err());
        }
        assert_eq!(parse("e3069283")?, 0xe3069283);
        Ok(())
    }
}

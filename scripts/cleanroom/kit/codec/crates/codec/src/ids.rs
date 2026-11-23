//! Flow ids, node ids, and database retainers (docs/object-keys.md).

use crate::error::{err, Result};
use crate::{lexidecimal, sha384};
use num_bigint::BigInt;

pub const MAX_FLOW_ID_LEN: usize = 160;
/// Internal owner standing in for `${flow}` in KernelSpace retainers.
pub const KERNEL_OWNER: &str = "basin";

/// Byte-level Flow id normalization (Kani-friendly): 1–160 ASCII letters,
/// digits, `.`, `_` or `-`, excluding `.` and `..`; letters are lowercased.
pub fn normalize_flow_id_bytes(s: &[u8]) -> Result<Vec<u8>> {
    if s.is_empty() || s.len() > MAX_FLOW_ID_LEN {
        return err("flow id must be 1 to 160 characters");
    }
    if s == b"." || s == b".." {
        return err("flow id must not be \".\" or \"..\"");
    }
    let mut out = Vec::with_capacity(s.len());
    for &c in s {
        match c {
            b'a'..=b'z' | b'0'..=b'9' | b'.' | b'_' | b'-' => out.push(c),
            b'A'..=b'Z' => out.push(c + 32),
            _ => return err("flow id may contain only ASCII letters, digits, '.', '_' and '-'"),
        }
    }
    Ok(out)
}

/// Canonical (lowercased) Flow id.
pub fn normalize_flow_id(s: &str) -> Result<String> {
    Ok(String::from_utf8(normalize_flow_id_bytes(s.as_bytes())?).expect("ASCII"))
}

/// `n-${lex(~height)}/v.${lex(version)}/t.${lex(token.length)}/d_${sha384(token)}`.
pub fn node_id(height: u64, version: u64, token: &[u8]) -> String {
    node_id_from_digest(height, version, token.len() as u64, &sha384::digest(token))
}

/// Node id from an already derived subkey (the token is never persisted).
pub fn node_id_from_digest(height: u64, version: u64, token_len: u64, digest: &[u8; 48]) -> String {
    format!(
        "n-{}/v.{}/t.{}/d_{}",
        lexidecimal::encode_complement(&BigInt::from(height)),
        lexidecimal::encode_u64(version),
        lexidecimal::encode_u64(token_len),
        sha384::hex(digest)
    )
}

/// Parse a node id into (height, version, token length, digest hex).
pub fn parse_node_id(s: &str) -> Result<(u64, u64, u64, String)> {
    let parts: Vec<&str> = s.split('/').collect();
    if parts.len() != 4 {
        return err("node id: expected four segments");
    }
    let h = parts[0].strip_prefix("n-").ok_or_else(|| crate::CodecError::new("node id: missing n- prefix"))?;
    let v = parts[1].strip_prefix("v.").ok_or_else(|| crate::CodecError::new("node id: missing v. segment"))?;
    let t = parts[2].strip_prefix("t.").ok_or_else(|| crate::CodecError::new("node id: missing t. segment"))?;
    let d = parts[3].strip_prefix("d_").ok_or_else(|| crate::CodecError::new("node id: missing d_ segment"))?;
    let hc = lexidecimal::decode(h)?;
    let height = crate::bigint::complement(&hc);
    let height: u64 = u64::try_from(&height).map_err(|_| crate::CodecError::new("node id: height out of range"))?;
    let version = lexidecimal::decode_u64(v)?;
    let tlen = lexidecimal::decode_u64(t)?;
    sha384::parse_hex(d)?;
    Ok((height, version, tlen, d.to_string()))
}

fn retainer(tag: &str, owner: &str, version: u64, viewpoint: &BigInt) -> String {
    format!("_{tag}_{owner}/v.{}/e.{}", lexidecimal::encode_u64(version), lexidecimal::encode(viewpoint))
}

/// Staging-area retainer `_a_${owner}/v.${lex(version)}/e.${lex(viewpoint)}`.
pub fn staging_retainer(owner: &str, version: u64, viewpoint: &BigInt) -> String {
    retainer("a", owner, version, viewpoint)
}

/// CommitMarker retainer `_m_${owner}/v.${lex(version)}/e.${lex(viewpoint)}`.
pub fn marker_retainer(owner: &str, version: u64, viewpoint: &BigInt) -> String {
    retainer("m", owner, version, viewpoint)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flow_ids() {
        assert_eq!(normalize_flow_id("My.Flow_1-x").unwrap(), "my.flow_1-x");
        assert_eq!(normalize_flow_id("...").unwrap(), "...");
        assert_eq!(normalize_flow_id(&"A".repeat(160)).unwrap(), "a".repeat(160));
        for bad in ["", ".", "..", "a/b", "a b", "é", "a\u{0}", &"a".repeat(161), "a+b", "a:b"] {
            assert!(normalize_flow_id(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn node_ids() {
        let id = node_id(0, 0, b"");
        assert_eq!(
            id,
            format!("n-0yvf/v.0zg0/t.0zg0/d_{}", sha384::hex(&sha384::digest(b"")))
        );
        let id2 = node_id(1, 0, b"tok");
        assert!(id2.starts_with("n-0yve/v.0zg0/t.0zg3/d_"));
        // complemented heights order parents (higher) before children
        assert!(node_id(5, 0, b"x") < node_id(4, 0, b"x"));
        assert_eq!(parse_node_id(&id2).unwrap().0, 1);
        assert_eq!(parse_node_id(&id2).unwrap().2, 3);
        assert!(parse_node_id("n-0zg0/v.0zg0/t.0zg0/d_00").is_err());
    }

    #[test]
    fn retainers() {
        assert_eq!(staging_retainer("flow", 0, &BigInt::from(0)), "_a_flow/v.0zg0/e.0zg0");
        assert_eq!(marker_retainer("basin", 0, &BigInt::from(16)), "_m_basin/v.0zg0/e.0zh10");
        assert!(marker_retainer("zzz", 0, &BigInt::from(0)).as_str() < "n-");
    }
}

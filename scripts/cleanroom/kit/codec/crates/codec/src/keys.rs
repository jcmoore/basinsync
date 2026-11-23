//! S3 object keys (docs/object-keys.md).

use crate::bigint::complement;
use crate::error::{err, Result};
use crate::lexidecimal;
use num_bigint::{BigInt, Sign};
use num_traits::One;

pub const EMPTY_SENTINEL: &str = "kernelspace/sentinel/empty";
pub const TOMBSTONE_PROBE_PREFIX: &str = "kernelspace/sentinel/tombstone/";
pub const MARGIN_PROBE_PREFIX: &str = "kernelspace/sentinel/margin/";
pub const TIMELINE_PREFIX: &str = "kernelspace/basin/timeline/";
pub const BASIN_EPILOGUE_PREFIX: &str = "kernelspace/basin/epilogue/";
pub const ANALOGUE_PREFIX: &str = "kernelspace/basin/analogue/";
pub const KERNEL_HEAP_PREFIX: &str = "kernelspace/basin/heap/";
pub const USER_HEAP_PREFIX: &str = "userspace/heap/";
pub const USER_FLOWS_PREFIX: &str = "userspace/flows/";
/// S3's key length limit.
pub const MAX_KEY_LEN: usize = 1024;

pub fn tombstone_probe_key(length: usize) -> String {
    format!("{TOMBSTONE_PROBE_PREFIX}{length}")
}

pub fn margin_probe_key(counter: u8) -> String {
    format!("{MARGIN_PROBE_PREFIX}{counter}")
}

/// `kernelspace/basin/timeline/${lex(~milestone)}`; milestones start at one.
pub fn timeline_key(milestone: &BigInt) -> Result<String> {
    if milestone < &BigInt::one() {
        return err("timeline milestone must be at least 1");
    }
    Ok(format!("{TIMELINE_PREFIX}{}", lexidecimal::encode_complement(milestone)))
}

/// Strict inverse of [`timeline_key`].
pub fn milestone_from_timeline_key(key: &str) -> Result<BigInt> {
    let suffix = key
        .strip_prefix(TIMELINE_PREFIX)
        .ok_or_else(|| crate::CodecError::new("not a timeline key"))?;
    let v = lexidecimal::decode(suffix)?;
    let m = complement(&v);
    if m < BigInt::one() {
        return err("timeline key does not encode a milestone >= 1");
    }
    Ok(m)
}

fn nonneg(viewpoint: &BigInt) -> Result<()> {
    if viewpoint.sign() == Sign::Minus {
        return err("viewpoint must be nonnegative");
    }
    Ok(())
}

/// Basin backup CommitMarker key `kernelspace/basin/epilogue/${lex(~viewpoint)}`.
pub fn basin_marker_key(viewpoint: &BigInt) -> Result<String> {
    nonneg(viewpoint)?;
    Ok(format!("{BASIN_EPILOGUE_PREFIX}{}", lexidecimal::encode_complement(viewpoint)))
}

/// Basin PromotionSign key `kernelspace/basin/epilogue/${lex(viewpoint)}`.
pub fn basin_sign_key(viewpoint: &BigInt) -> Result<String> {
    nonneg(viewpoint)?;
    Ok(format!("{BASIN_EPILOGUE_PREFIX}{}", lexidecimal::encode(viewpoint)))
}

/// `userspace/flows/${flow}/${flow_revision}/epilogue/`.
pub fn flow_epilogue_prefix(flow: &str, flow_revision: &str) -> String {
    format!("{USER_FLOWS_PREFIX}{flow}/{flow_revision}/epilogue/")
}

/// `userspace/flows/${flow}/`, the parent of every revision of a Flow.
pub fn flow_prefix(flow: &str) -> String {
    format!("{USER_FLOWS_PREFIX}{flow}/")
}

pub fn flow_marker_key(flow: &str, rev: &str, viewpoint: &BigInt) -> Result<String> {
    nonneg(viewpoint)?;
    Ok(format!("{}{}", flow_epilogue_prefix(flow, rev), lexidecimal::encode_complement(viewpoint)))
}

pub fn flow_sign_key(flow: &str, rev: &str, viewpoint: &BigInt) -> Result<String> {
    nonneg(viewpoint)?;
    Ok(format!("{}{}", flow_epilogue_prefix(flow, rev), lexidecimal::encode(viewpoint)))
}

/// An entry under an epilogue prefix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EpilogueEntry {
    /// CommitMarker at this viewpoint (complemented key).
    Marker(BigInt),
    /// PromotionSign at this viewpoint (uncomplemented key).
    Sign(BigInt),
}

/// Classify an epilogue key suffix: negative Lexidecimal is a complemented
/// CommitMarker viewpoint, nonnegative is a PromotionSign viewpoint.
pub fn parse_epilogue_suffix(suffix: &str) -> Result<EpilogueEntry> {
    let v = lexidecimal::decode(suffix)?;
    if v.sign() == Sign::Minus {
        Ok(EpilogueEntry::Marker(complement(&v)))
    } else {
        Ok(EpilogueEntry::Sign(v))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Space {
    User,
    Kernel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Generation {
    Young,
    Old,
}

pub fn heap_prefix(space: Space, gen: Generation) -> String {
    let p = match space {
        Space::User => USER_HEAP_PREFIX,
        Space::Kernel => KERNEL_HEAP_PREFIX,
    };
    let g = match gen {
        Generation::Young => "young",
        Generation::Old => "old",
    };
    format!("{p}{g}/")
}

/// `{kernelspace/basin|userspace}/heap/{young|old}/${node_id}/${node_revision}`.
pub fn heap_key(space: Space, gen: Generation, node_id: &str, revision: &str) -> String {
    format!("{}{node_id}/{revision}", heap_prefix(space, gen))
}

/// `kernelspace/basin/analogue/${lex(~lowest)}/${lex(~highest)}/v0/${bulkmanifest_start}/${coordinate}`.
/// Requires `1 <= lowest < highest` and nonnegative start and coordinate.
pub fn analogue_key(lowest: &BigInt, highest: &BigInt, bulkmanifest_start: &BigInt, coordinate: &BigInt) -> Result<String> {
    if lowest < &BigInt::one() {
        return err("analogue: lowest must be at least 1");
    }
    if highest <= lowest {
        return err("analogue: highest must exceed lowest");
    }
    if bulkmanifest_start.sign() == Sign::Minus || coordinate.sign() == Sign::Minus {
        return err("analogue: offsets must be nonnegative");
    }
    Ok(format!(
        "{ANALOGUE_PREFIX}{}/{}/v0/{}/{}",
        lexidecimal::encode_complement(lowest),
        lexidecimal::encode_complement(highest),
        bulkmanifest_start,
        coordinate
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b(i: i64) -> BigInt {
        BigInt::from(i)
    }

    #[test]
    fn timeline() {
        assert_eq!(timeline_key(&b(1)).unwrap(), "kernelspace/basin/timeline/0yve");
        assert_eq!(timeline_key(&b(16)).unwrap(), "kernelspace/basin/timeline/0yuef");
        assert!(timeline_key(&b(0)).is_err());
        assert!(timeline_key(&b(-3)).is_err());
        for m in [1i64, 2, 15, 16, 17, 255, 256, 1 << 40] {
            assert_eq!(milestone_from_timeline_key(&timeline_key(&b(m)).unwrap()).unwrap(), b(m));
        }
        // newer milestones sort first
        assert!(timeline_key(&b(17)).unwrap() < timeline_key(&b(16)).unwrap());
        assert!(timeline_key(&b(2)).unwrap() < timeline_key(&b(1)).unwrap());
        for bad in ["kernelspace/basin/timeline/0yvf", "kernelspace/basin/timeline/0zg1",
                    "kernelspace/basin/timeline/0yve0", "kernelspace/basin/timeline/", "x/0yve",
                    "kernelspace/basin/timeline/0yuff"] {
            assert!(milestone_from_timeline_key(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn epilogue() {
        assert_eq!(basin_marker_key(&b(0)).unwrap(), "kernelspace/basin/epilogue/0yvf");
        assert_eq!(basin_sign_key(&b(0)).unwrap(), "kernelspace/basin/epilogue/0zg0");
        assert_eq!(flow_marker_key("f", "rev", &b(1)).unwrap(), "userspace/flows/f/rev/epilogue/0yve");
        assert_eq!(flow_sign_key("f", "rev", &b(1)).unwrap(), "userspace/flows/f/rev/epilogue/0zg1");
        assert!(basin_marker_key(&b(-1)).is_err());
        assert_eq!(parse_epilogue_suffix("0yve").unwrap(), EpilogueEntry::Marker(b(1)));
        assert_eq!(parse_epilogue_suffix("0zg1").unwrap(), EpilogueEntry::Sign(b(1)));
        assert!(parse_epilogue_suffix("0zg01").is_err());
        // markers (complemented) sort before signs, newest marker first
        assert!(basin_marker_key(&b(5)).unwrap() < basin_marker_key(&b(4)).unwrap());
        assert!(basin_marker_key(&b(0)).unwrap() < basin_sign_key(&b(0)).unwrap());
    }

    #[test]
    fn heap_and_analogue() {
        assert_eq!(heap_key(Space::User, Generation::Young, "n-x", "r"), "userspace/heap/young/n-x/r");
        assert_eq!(heap_key(Space::Kernel, Generation::Old, "n-x", "r"), "kernelspace/basin/heap/old/n-x/r");
        assert_eq!(
            analogue_key(&b(1), &b(3), &b(488), &b(0)).unwrap(),
            "kernelspace/basin/analogue/0yve/0yvc/v0/488/0"
        );
        assert!(analogue_key(&b(0), &b(3), &b(1), &b(0)).is_err());
        assert!(analogue_key(&b(3), &b(3), &b(1), &b(0)).is_err());
        assert_eq!(tombstone_probe_key(2), "kernelspace/sentinel/tombstone/2");
        assert_eq!(margin_probe_key(7), "kernelspace/sentinel/margin/7");
    }
}

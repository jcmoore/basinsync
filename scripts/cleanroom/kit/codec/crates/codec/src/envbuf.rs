//! EnvBuf ("Enveloped Buffers"): concatenable binary envelopes with matching
//! headers and footers (docs/glossary/envbuf.md, docs/glossary/margin.md).
//!
//! ```text
//! header: "enV" | format:3, alignment_power:5 | message_length:u32le
//! footer: message_length:u32le | format:3, alignment_power:5 | "vNE"
//! ```

use crate::error::{err, Result};
use std::ops::Range;

pub const FORMAT_RAW: u8 = 0b000;
pub const FORMAT_NESTED: u8 = 0b100;
pub const FORMAT_FLEXBUFFERS: u8 = 0b101;
pub const FORMAT_MARGIN: u8 = 0b110;
/// Reserved for platform extensions (u64 lengths); support is pending.
pub const FORMAT_EXTENSION: u8 = 0b111;
pub const DEFAULT_ALIGNMENT_POWER: u8 = 3;
/// Largest alignment power this encoder writes (64 KiB alignment). Decoders
/// accept every 5-bit power; the buffer length bounds them.
pub const MAX_ENCODE_ALIGNMENT_POWER: u8 = 16;
pub const HEADER_MAGIC: &[u8; 3] = b"enV";
pub const FOOTER_MAGIC: &[u8; 3] = b"vNE";
pub const PADDING: u8 = 0xee;
pub const FRAME_LEN: usize = 8;
/// Length of counter margins and standalone margins.
pub const MARGIN_LEN: usize = 24;

fn check_format(format: u8) -> Result<()> {
    match format {
        FORMAT_RAW | FORMAT_NESTED | FORMAT_FLEXBUFFERS | FORMAT_MARGIN => Ok(()),
        FORMAT_EXTENSION => err("envbuf: format 111 (platform extension) is not supported"),
        f if f > 7 => err("envbuf: format must be a 3-bit number"),
        _ => err("envbuf: reserved format code"),
    }
}

/// Descriptor byte `format << 5 | alignment_power`.
pub fn descriptor(format: u8, alignment_power: u8) -> Result<u8> {
    check_format(format)?;
    if alignment_power > 31 {
        return err("envbuf: alignment power must fit five bits");
    }
    Ok(format << 5 | alignment_power)
}

/// `ceil((message_length + 16) / alignment) * alignment` as u64, checked.
fn envelope_len_u64(message_len: u64, alignment_power: u8) -> Option<u64> {
    if alignment_power > 31 {
        return None;
    }
    let a = 1u64 << alignment_power;
    let raw = message_len.checked_add(16)?;
    raw.checked_add(a - 1).map(|v| v & !(a - 1))
}

/// Complete envelope length including header, padding and footer.
pub fn envelope_len(message_len: usize, alignment_power: u8) -> Result<usize> {
    if message_len as u64 > u32::MAX as u64 {
        return err("envbuf: message exceeds u32 length field");
    }
    if alignment_power > 31 {
        return err("envbuf: alignment power must fit five bits");
    }
    envelope_len_u64(message_len as u64, alignment_power)
        .and_then(|v| usize::try_from(v).ok())
        .ok_or_else(|| crate::CodecError::new("envbuf: envelope length overflow"))
}

/// Allocation-free encoder core: writes one envelope at the start of `out`
/// (which must hold `envelope_len` bytes) and returns its length.
pub fn encode_to(out: &mut [u8], format: u8, alignment_power: u8, message: &[u8]) -> Result<usize> {
    let d = descriptor(format, alignment_power)?;
    if alignment_power > MAX_ENCODE_ALIGNMENT_POWER {
        return err("envbuf: alignment power too large for this encoder");
    }
    let total = envelope_len(message.len(), alignment_power)?;
    if out.len() < total {
        return err("envbuf: output buffer too small");
    }
    let len = (message.len() as u32).to_le_bytes();
    out[0..3].copy_from_slice(HEADER_MAGIC);
    out[3] = d;
    out[4..8].copy_from_slice(&len);
    let msg_end = FRAME_LEN + message.len();
    out[FRAME_LEN..msg_end].copy_from_slice(message);
    for b in &mut out[msg_end..total - FRAME_LEN] {
        *b = PADDING;
    }
    out[total - 8..total - 4].copy_from_slice(&len);
    out[total - 4] = d;
    out[total - 3..total].copy_from_slice(FOOTER_MAGIC);
    Ok(total)
}

/// Append one envelope to `out`; returns the absolute range of the message.
pub fn encode_into(
    out: &mut Vec<u8>,
    format: u8,
    alignment_power: u8,
    message: &[u8],
) -> Result<Range<usize>> {
    descriptor(format, alignment_power)?;
    if alignment_power > MAX_ENCODE_ALIGNMENT_POWER {
        return err("envbuf: alignment power too large for this encoder");
    }
    let total = envelope_len(message.len(), alignment_power)?;
    let base = out.len();
    out.resize(base + total, 0);
    encode_to(&mut out[base..], format, alignment_power, message)?;
    Ok(base + FRAME_LEN..base + FRAME_LEN + message.len())
}

/// Encode one envelope.
pub fn encode(format: u8, alignment_power: u8, message: &[u8]) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    encode_into(&mut out, format, alignment_power, message)?;
    Ok(out)
}

/// A parsed envelope. All offsets are absolute in the parsed buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Envelope {
    pub start: usize,
    /// Exclusive end, including the footer.
    pub end: usize,
    pub format: u8,
    pub alignment_power: u8,
    pub message: Range<usize>,
}

impl Envelope {
    pub fn descriptor(&self) -> u8 {
        self.format << 5 | self.alignment_power
    }
    pub fn len(&self) -> usize {
        self.end - self.start
    }
    pub fn is_empty(&self) -> bool {
        self.end == self.start
    }
    pub fn is_margin(&self) -> bool {
        self.format == FORMAT_MARGIN
    }
}

fn u32le(b: &[u8]) -> u32 {
    u32::from_le_bytes([b[0], b[1], b[2], b[3]])
}

/// Parse and fully validate the envelope starting at `start` (forward
/// traversal): header magic, supported format, exact aligned length within
/// the buffer, `0xee` padding, and a footer mirroring the header.
pub fn parse_at(buf: &[u8], start: usize) -> Result<Envelope> {
    if start > buf.len() || buf.len() - start < FRAME_LEN * 2 {
        return err("envbuf: truncated envelope");
    }
    let h = &buf[start..start + FRAME_LEN];
    if &h[0..3] != HEADER_MAGIC {
        return err("envbuf: bad header magic");
    }
    let d = h[3];
    let format = d >> 5;
    let alignment_power = d & 0x1f;
    check_format(format)?;
    let mlen = u32le(&h[4..8]) as u64;
    let total = match envelope_len_u64(mlen, alignment_power) {
        Some(t) => t,
        None => return err("envbuf: envelope length overflow"),
    };
    if total > (buf.len() - start) as u64 {
        return err("envbuf: envelope exceeds buffer");
    }
    let end = start + total as usize;
    let msg_start = start + FRAME_LEN;
    let msg_end = msg_start + mlen as usize;
    let f = &buf[end - FRAME_LEN..end];
    if &f[5..8] != FOOTER_MAGIC {
        return err("envbuf: bad footer magic");
    }
    if f[4] != d {
        return err("envbuf: footer descriptor does not match header");
    }
    if u32le(&f[0..4]) as u64 != mlen {
        return err("envbuf: footer length does not match header");
    }
    if buf[msg_end..end - FRAME_LEN].iter().any(|&b| b != PADDING) {
        return err("envbuf: invalid alignment padding");
    }
    Ok(Envelope { start, end, format, alignment_power, message: msg_start..msg_end })
}

/// Parse the envelope whose footer ends at `end` (backward traversal).
pub fn parse_backward(buf: &[u8], end: usize) -> Result<Envelope> {
    if end > buf.len() || end < FRAME_LEN * 2 {
        return err("envbuf: truncated envelope");
    }
    let f = &buf[end - FRAME_LEN..end];
    if &f[5..8] != FOOTER_MAGIC {
        return err("envbuf: bad footer magic");
    }
    let mlen = u32le(&f[0..4]) as u64;
    let total = match envelope_len_u64(mlen, f[4] & 0x1f) {
        Some(t) if t <= end as u64 => t as usize,
        _ => return err("envbuf: envelope exceeds buffer"),
    };
    let env = parse_at(&buf[..end], end - total)?;
    // The header may describe a different envelope than the footer we
    // started from (found by Kani: `envbuf_backward_no_panic`).
    if env.end != end {
        return err("envbuf: header and trailing footer describe different envelopes");
    }
    Ok(env)
}

/// Decode exactly one envelope with no trailing bytes.
pub fn decode_one(wire: &[u8]) -> Result<Envelope> {
    let env = parse_at(wire, 0)?;
    if env.end != wire.len() {
        return err("envbuf: trailing bytes after envelope");
    }
    Ok(env)
}

/// Parse a complete feed (all envelopes, margins included). An empty buffer
/// is an empty feed.
pub fn parse_feed(buf: &[u8]) -> Result<Vec<Envelope>> {
    parse_feed_range(buf, 0..buf.len())
}

/// Parse a feed occupying `range` of `buf` (for example a nested message),
/// validating boundaries relative to that range; offsets stay absolute.
pub fn parse_feed_range(buf: &[u8], range: Range<usize>) -> Result<Vec<Envelope>> {
    if range.start > range.end || range.end > buf.len() {
        return err("envbuf: feed range out of bounds");
    }
    let inner = &buf[..range.end];
    let mut pos = range.start;
    let mut out = Vec::new();
    while pos < range.end {
        let env = parse_at(inner, pos)?;
        pos = env.end;
        out.push(env);
    }
    Ok(out)
}

/// Parse the nested feed carried by a format `100` envelope.
pub fn parse_nested(buf: &[u8], env: &Envelope) -> Result<Vec<Envelope>> {
    if env.format != FORMAT_NESTED {
        return err("envbuf: not a nested envelope");
    }
    parse_feed_range(buf, env.message.clone())
}

/// The trailing 24-byte counter margin: format 110, default alignment, an
/// eight-byte little-endian `u64` message.
pub fn counter_margin(counter: u64) -> [u8; 24] {
    let v = encode(FORMAT_MARGIN, DEFAULT_ALIGNMENT_POWER, &counter.to_le_bytes()).expect("valid");
    v.try_into().expect("24 bytes")
}

/// A standalone margin: one `u8` message counting 1 through 7, padded to 24.
pub fn standalone_margin(counter: u8) -> Result<[u8; 24]> {
    if !(1..=7).contains(&counter) {
        return err("envbuf: standalone margin counter must be 1 through 7");
    }
    let v = encode(FORMAT_MARGIN, DEFAULT_ALIGNMENT_POWER, &[counter])?;
    Ok(v.try_into().expect("24 bytes"))
}

/// Counter carried by a margin envelope (eight-byte `u64` or one-byte `u8`).
pub fn margin_counter(buf: &[u8], env: &Envelope) -> Result<u64> {
    if env.format != FORMAT_MARGIN {
        return err("envbuf: not a margin");
    }
    let m = buf
        .get(env.message.clone())
        .ok_or_else(|| crate::CodecError::new("envbuf: margin out of bounds"))?;
    match m.len() {
        8 => Ok(u64::from_le_bytes(m.try_into().expect("8"))),
        1 => Ok(m[0] as u64),
        _ => err("envbuf: margin message must be one or eight bytes"),
    }
}

/// Exclusive end of `buf` after skipping trailing margins backward.
pub fn strip_trailing_margins(buf: &[u8]) -> Result<usize> {
    let mut end = buf.len();
    while end > 0 {
        let env = parse_backward(buf, end)?;
        if !env.is_margin() {
            break;
        }
        end = env.start;
    }
    Ok(end)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn descriptors() {
        assert_eq!(descriptor(FORMAT_FLEXBUFFERS, 3).unwrap(), 0xa3);
        assert_eq!(descriptor(FORMAT_NESTED, 3).unwrap(), 0x83);
        assert_eq!(descriptor(FORMAT_MARGIN, 3).unwrap(), 0xc3);
        assert_eq!(descriptor(FORMAT_RAW, 3).unwrap(), 0x03);
        for f in [1, 2, 3, 7, 8] {
            assert!(descriptor(f, 3).is_err());
        }
        assert!(descriptor(0, 32).is_err());
    }

    #[test]
    fn lengths() {
        assert_eq!(envelope_len(0, 3).unwrap(), 16);
        assert_eq!(envelope_len(1, 3).unwrap(), 24);
        assert_eq!(envelope_len(8, 3).unwrap(), 24);
        assert_eq!(envelope_len(9, 3).unwrap(), 32);
        assert_eq!(envelope_len(5, 0).unwrap(), 21);
        assert_eq!(envelope_len(5, 5).unwrap(), 32);
    }

    #[test]
    fn layout() {
        let e = encode(FORMAT_FLEXBUFFERS, 3, b"abc").unwrap();
        assert_eq!(
            e,
            [b"enV".as_slice(), &[0xa3, 3, 0, 0, 0], b"abc", &[0xee; 5], &[3, 0, 0, 0, 0xa3],
             b"vNE"].concat()
        );
        let env = decode_one(&e).unwrap();
        assert_eq!(env, Envelope { start: 0, end: 24, format: 5, alignment_power: 3, message: 8..11 });
        assert_eq!(parse_backward(&e, 24).unwrap(), env);
        let empty = encode(FORMAT_RAW, 3, b"").unwrap();
        assert_eq!(empty.len(), 16);
        assert_eq!(decode_one(&empty).unwrap().message, 8..8);
    }

    #[test]
    fn margins() {
        let m = counter_margin(0);
        assert_eq!(m.len(), 24);
        assert_eq!(&m[0..8], &[b'e', b'n', b'V', 0xc3, 8, 0, 0, 0]);
        assert_eq!(&m[8..16], &[0; 8]);
        let env = decode_one(&m).unwrap();
        assert_eq!(margin_counter(&m, &env).unwrap(), 0);
        let m5 = counter_margin(0x0102030405060708);
        assert_eq!(&m5[8..16], &[8, 7, 6, 5, 4, 3, 2, 1]);
        let s = standalone_margin(3).unwrap();
        assert_eq!(&s[0..9], &[b'e', b'n', b'V', 0xc3, 1, 0, 0, 0, 3]);
        assert_eq!(&s[9..16], &[0xee; 7]);
        assert_eq!(margin_counter(&s, &decode_one(&s).unwrap()).unwrap(), 3);
        assert!(standalone_margin(0).is_err());
        assert!(standalone_margin(8).is_err());
    }

    #[test]
    fn feeds_and_nesting() {
        let mut inner = Vec::new();
        encode_into(&mut inner, FORMAT_FLEXBUFFERS, 3, b"one").unwrap();
        encode_into(&mut inner, FORMAT_RAW, 3, b"two!!!!!!").unwrap();
        inner.extend_from_slice(&counter_margin(1));
        let mut outer = Vec::new();
        let r = encode_into(&mut outer, FORMAT_NESTED, 3, &inner).unwrap();
        outer.extend_from_slice(&counter_margin(0));
        let feed = parse_feed(&outer).unwrap();
        assert_eq!(feed.len(), 2);
        assert_eq!(feed[0].message, r);
        let nested = parse_nested(&outer, &feed[0]).unwrap();
        assert_eq!(nested.len(), 3);
        assert_eq!(&outer[nested[0].message.clone()], b"one");
        assert_eq!(&outer[nested[1].message.clone()], b"two!!!!!!");
        assert!(nested[2].is_margin());
        assert_eq!(strip_trailing_margins(&outer).unwrap(), feed[0].end);
        let back = parse_backward(&outer, feed[0].end).unwrap();
        assert_eq!(back, feed[0]);
        assert_eq!(parse_feed(&[]).unwrap(), vec![]);
    }

    #[test]
    fn rejections() {
        let good = encode(FORMAT_RAW, 3, b"hello").unwrap();
        let mutate = |i: usize, v: u8| {
            let mut b = good.clone();
            b[i] = v;
            b
        };
        assert!(decode_one(&mutate(0, b'E')).is_err()); // magic
        assert!(decode_one(&mutate(3, 0x04)).is_err()); // header/footer descriptor mismatch
        assert!(decode_one(&mutate(4, 6)).is_err()); // header length mismatch
        assert!(decode_one(&mutate(14, 0)).is_err()); // padding
        assert!(decode_one(&mutate(20, 0x03 | 0x20)).is_err()); // footer descriptor
        assert!(decode_one(&mutate(23, b'e')).is_err()); // footer magic
        assert!(decode_one(&good[..23]).is_err());
        let mut long = good.clone();
        long.push(0);
        assert!(decode_one(&long).is_err());
        // reserved formats
        for f in [1u8, 2, 3, 7] {
            let mut b = good.clone();
            b[3] = f << 5 | 3;
            b[20] = f << 5 | 3;
            assert!(decode_one(&b).is_err());
        }
        // wrong alignment (header says power 4 -> must be 32 bytes)
        let mut b = good.clone();
        b[3] = 4;
        b[20] = 4;
        assert!(decode_one(&b).is_err());
        assert!(parse_feed(&[1, 2, 3]).is_err());
    }

    proptest! {
        #[test]
        fn roundtrip(msg in proptest::collection::vec(any::<u8>(), 0..100), f in 0usize..4, ap in 0u8..6) {
            let format = [0u8, 4, 5, 6][f];
            let e = encode(format, ap, &msg).unwrap();
            prop_assert_eq!(e.len() % (1 << ap), 0);
            let env = decode_one(&e).unwrap();
            prop_assert_eq!(&e[env.message.clone()], &msg[..]);
            prop_assert_eq!(env.format, format);
            prop_assert_eq!(parse_backward(&e, e.len()).unwrap(), env);
        }

        #[test]
        fn no_panic(buf in proptest::collection::vec(any::<u8>(), 0..64)) {
            let _ = parse_feed(&buf);
            let _ = parse_backward(&buf, buf.len());
            if let Ok(env) = decode_one(&buf) {
                prop_assert_eq!(encode(env.format, env.alignment_power, &buf[env.message.clone()]).unwrap(), buf);
            }
        }
    }
}

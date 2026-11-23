use basinsync_codec::flexbuffers::is_utf8;

#[test]
fn matches_std_exhaustive_short() {
    for a in 0..=255u8 {
        assert_eq!(is_utf8(&[a]), std::str::from_utf8(&[a]).is_ok());
        for b in 0..=255u8 {
            assert_eq!(is_utf8(&[a, b]), std::str::from_utf8(&[a, b]).is_ok(), "{a:x} {b:x}");
            if a >= 0xc0 {
                for c in [0x00u8, 0x7f, 0x80, 0x8f, 0x90, 0x9f, 0xa0, 0xbf, 0xc0, 0xff] {
                    let v = [a, b, c];
                    assert_eq!(is_utf8(&v), std::str::from_utf8(&v).is_ok(), "{v:x?}");
                }
            }
        }
    }
}

#[test]
fn matches_std_random() {
    let mut x: u64 = 0x9e3779b97f4a7c15;
    for _ in 0..200_000 {
        let mut v = Vec::new();
        x ^= x << 13; x ^= x >> 7; x ^= x << 17;
        let n = (x % 7) as usize;
        for i in 0..n {
            let byte = (x >> (8 * i)) as u8;
            v.push(if x & (1 << (40 + i)) != 0 { 0x80 | (byte & 0x3f) } else { byte });
        }
        assert_eq!(is_utf8(&v), std::str::from_utf8(&v).is_ok(), "{v:x?}");
        let s = "héllo €𝄞";
        assert!(is_utf8(s.as_bytes()));
    }
}

/// Regression for the Kani counterexample: a trailing footer whose header
/// describes a shorter envelope must be rejected, not mis-parsed.
#[test]
fn backward_parse_rejects_mismatched_header() {
    use basinsync_codec::envbuf;
    // Two envelopes back to back; claim the second's footer but give the
    // region a header from the first, shorter one.
    let a = envbuf::encode(0, 3, b"").unwrap(); // 16 bytes
    let b = envbuf::encode(0, 3, &[1u8; 8]).unwrap(); // 24 bytes
    let mut buf = a.clone();
    buf.extend_from_slice(&b[16..]); // trailing 8 bytes = b's footer (len 8)
    // b's footer says total 24 == buf.len(), header at 0 is a's (total 16).
    assert!(envbuf::parse_backward(&buf, buf.len()).is_err());
}

//! Arbitrary-precision helpers: two's complement and canonical decimal text.

use crate::error::{err, Result};
use num_bigint::{BigInt, BigUint, Sign};
use num_traits::{One, Zero};

/// Arbitrary-precision bitwise complement: `~n = -n - 1`.
pub fn complement(n: &BigInt) -> BigInt {
    -n - BigInt::one()
}

/// Parse canonical decimal: optional `-`, digits without leading zeros,
/// no `-0`, no `+`, no whitespace.
pub fn parse_decimal(s: &str) -> Result<BigInt> {
    let b = s.as_bytes();
    let (neg, digits) = match b.first() {
        Some(b'-') => (true, &b[1..]),
        _ => (false, b),
    };
    if digits.is_empty() {
        return err(format!("invalid decimal integer {s:?}: no digits"));
    }
    if !digits.iter().all(u8::is_ascii_digit) {
        return err(format!("invalid decimal integer {s:?}"));
    }
    if digits.len() > 1 && digits[0] == b'0' {
        return err(format!("noncanonical decimal integer {s:?}: leading zero"));
    }
    if neg && digits == b"0" {
        return err("noncanonical decimal integer \"-0\"");
    }
    let mag = BigUint::parse_bytes(digits, 10)
        .ok_or_else(|| crate::CodecError::new(format!("invalid decimal integer {s:?}")))?;
    Ok(BigInt::from_biguint(if neg { Sign::Minus } else { Sign::Plus }, mag))
}

/// Canonical decimal text.
pub fn to_decimal(n: &BigInt) -> String {
    n.to_str_radix(10)
}

/// Minimal big-endian hexadecimal nibbles (values 0..=15) of a nonnegative
/// integer; zero is `[0]`.
pub(crate) fn nibbles_of(n: &BigInt) -> Vec<u8> {
    debug_assert!(n.sign() != Sign::Minus);
    let (_, mut digits) = n.to_radix_be(16);
    if digits.is_empty() {
        digits.push(0);
    }
    digits
}

/// Nonnegative integer from big-endian nibbles.
pub(crate) fn from_nibbles(nibbles: &[u8]) -> BigInt {
    if nibbles.iter().all(|&d| d == 0) {
        return BigInt::zero();
    }
    BigInt::from_radix_be(Sign::Plus, nibbles, 16).expect("nibbles are < 16")
}

/// Split a signed integer into (negative, minimal nibbles of `n` or `~n`).
pub(crate) fn split_signed(n: &BigInt) -> (bool, Vec<u8>) {
    if n.sign() == Sign::Minus {
        (true, nibbles_of(&complement(n)))
    } else {
        (false, nibbles_of(n))
    }
}

/// Inverse of [`split_signed`].
pub(crate) fn join_signed(negative: bool, nibbles: &[u8]) -> BigInt {
    let m = from_nibbles(nibbles);
    if negative {
        complement(&m)
    } else {
        m
    }
}

/// Minimal big-endian nibbles of a `u64` written into a fixed buffer;
/// returns the number of digits used (at the front of `out`).
pub(crate) fn u64_nibbles(mut v: u64, out: &mut [u8; 16]) -> usize {
    let mut tmp = [0u8; 16];
    let mut n = 0;
    loop {
        tmp[n] = (v & 0xf) as u8;
        n += 1;
        v >>= 4;
        if v == 0 {
            break;
        }
    }
    for i in 0..n {
        out[i] = tmp[n - 1 - i];
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complement_examples() {
        assert_eq!(complement(&BigInt::from(0)), BigInt::from(-1));
        assert_eq!(complement(&BigInt::from(-1)), BigInt::from(0));
        assert_eq!(complement(&BigInt::from(16)), BigInt::from(-17));
        let x = BigInt::from(123456789);
        assert_eq!(complement(&complement(&x)), x);
    }

    #[test]
    fn decimal_canonical() {
        assert_eq!(parse_decimal("0").unwrap(), BigInt::from(0));
        assert_eq!(parse_decimal("-42").unwrap(), BigInt::from(-42));
        assert_eq!(
            to_decimal(&parse_decimal("18446744073709551616").unwrap()),
            "18446744073709551616"
        );
        for bad in ["", "-", "-0", "+1", "01", " 1", "1 ", "1.0", "1e3", "0x1", "--1", "00"] {
            assert!(parse_decimal(bad).is_err(), "{bad:?}");
        }
    }
}

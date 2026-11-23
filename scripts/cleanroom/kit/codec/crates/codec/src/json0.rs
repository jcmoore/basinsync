//! JSON0: a JSON dialect interpreting value strings of two or more characters
//! beginning with `0` (docs/glossary/json0.md).
//!
//! | Prefix | Value |
//! | --- | --- |
//! | `00` | literal string minus the first `0` |
//! | `0x` | nonnegative bigint, hexadecimal |
//! | `0~` | complement of a hexadecimal bigint |
//! | `0.` | arbitrary-precision decimal real |
//! | `0*` | bytes, base64url (writers omit padding) |
//! | `0%` | bytes, standard base64 |
//! | `0#` | well-known symbol |
//! | `0$` | global symbol |
//!
//! `0w`, `0y`, `0z`, `0A`–`0Z` are reserved; `0_` needs application
//! agreement (none here) and every other `0` prefix is invalid.
//! Object keys are never interpreted.

use crate::base64;
use crate::bigint::complement;
use crate::error::{err, Result};
use num_bigint::{BigInt, BigUint, Sign};

pub const MAX_DEPTH: usize = 128;
pub const WELL_KNOWN_SYMBOLS: [&str; 4] = crate::flexbuffers::WELL_KNOWN_SYMBOLS;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    /// Raw JSON number text (validated against the JSON grammar).
    Number(String),
    String(String),
    BigInt(BigInt),
    /// Decimal real text following `0.` (e.g. `"0.-1.5e3"` -> `"-1.5e3"`).
    Decimal(String),
    Bytes(Vec<u8>),
    WellKnownSymbol(String),
    GlobalSymbol(String),
    Array(Vec<Value>),
    /// Insertion order, unique keys.
    Object(Vec<(String, Value)>),
}

/// The interpretation selected by a value string's prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Prefix {
    /// Not of the `0?…` form: an ordinary string.
    Plain,
    /// `00…`: literal string, drop the first `0`.
    Literal,
    Hex,
    Complement,
    Decimal,
    Base64Url,
    Base64,
    WellKnownSymbol,
    GlobalSymbol,
}

/// Classify a value string (Kani-friendly core). Errors for reserved,
/// application-specific and unknown prefixes.
pub fn classify(s: &[u8]) -> Result<Prefix> {
    if s.first() != Some(&b'0') {
        return Ok(Prefix::Plain);
    }
    if s.len() < 2 {
        return err("json0: lone 0");
    }
    Ok(match s[1] {
        b'0' => Prefix::Literal,
        b'x' => Prefix::Hex,
        b'~' => Prefix::Complement,
        b'.' => Prefix::Decimal,
        b'*' => Prefix::Base64Url,
        b'%' => Prefix::Base64,
        b'#' => Prefix::WellKnownSymbol,
        b'$' => Prefix::GlobalSymbol,
        b'w' | b'y' | b'z' => return err("json0: reserved prefix (Diffuse16/Lexidecimal)"),
        b'A'..=b'Z' => return err("json0: reserved uppercase prefix"),
        b'_' => return err("json0: application-specific 0_ prefix not agreed"),
        _ => return err("json0: invalid 0 prefix"),
    })
}

/// Text a writer emits for a literal string (Kani-friendly core): strings
/// beginning with `0`, including "0" itself, gain another `0`.
pub fn escape_literal(s: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len() + 1);
    if s.first() == Some(&b'0') {
        out.push(b'0');
    }
    out.extend_from_slice(s);
    out
}

fn parse_hex_big(h: &str) -> Result<BigInt> {
    if h.is_empty() || !h.bytes().all(|c| c.is_ascii_hexdigit()) {
        return err("json0: invalid hexadecimal bigint");
    }
    let m = BigUint::parse_bytes(h.as_bytes(), 16).ok_or_else(|| crate::CodecError::new("json0: invalid hex"))?;
    Ok(BigInt::from_biguint(Sign::Plus, m))
}

/// Validate decimal real text: `-?digits(.digits)?([eE][+-]?digits)?`.
pub fn is_decimal_text(s: &[u8]) -> bool {
    let mut i = 0;
    if s.first() == Some(&b'-') {
        i += 1;
    }
    let digits = |i: &mut usize| {
        let st = *i;
        while *i < s.len() && s[*i].is_ascii_digit() {
            *i += 1;
        }
        *i > st
    };
    if !digits(&mut i) {
        return false;
    }
    if i < s.len() && s[i] == b'.' {
        i += 1;
        if !digits(&mut i) {
            return false;
        }
    }
    if i < s.len() && (s[i] == b'e' || s[i] == b'E') {
        i += 1;
        if i < s.len() && (s[i] == b'+' || s[i] == b'-') {
            i += 1;
        }
        if !digits(&mut i) {
            return false;
        }
    }
    i == s.len()
}

/// Interpret a JSON string in value position.
pub fn interpret_string(s: String) -> Result<Value> {
    let p = classify(s.as_bytes())?;
    let rest = || s[2..].to_string();
    Ok(match p {
        Prefix::Plain => Value::String(s),
        Prefix::Literal => Value::String(s[1..].to_string()),
        Prefix::Hex => Value::BigInt(parse_hex_big(&s[2..])?),
        Prefix::Complement => Value::BigInt(complement(&parse_hex_big(&s[2..])?)),
        Prefix::Decimal => {
            if !is_decimal_text(&s.as_bytes()[2..]) {
                return err("json0: invalid decimal real");
            }
            Value::Decimal(rest())
        }
        Prefix::Base64Url => Value::Bytes(base64::decode(&s.as_bytes()[2..], true)?),
        Prefix::Base64 => Value::Bytes(base64::decode(&s.as_bytes()[2..], false)?),
        Prefix::WellKnownSymbol => {
            let n = rest();
            if !WELL_KNOWN_SYMBOLS.contains(&n.as_str()) {
                return err("json0: unknown well-known symbol");
            }
            Value::WellKnownSymbol(n)
        }
        Prefix::GlobalSymbol => Value::GlobalSymbol(rest()),
    })
}

// ---------------------------------------------------------------------------
// Parser

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
}

impl<'a> Parser<'a> {
    fn ws(&mut self) {
        while self.i < self.s.len() && matches!(self.s[self.i], b' ' | b'\t' | b'\n' | b'\r') {
            self.i += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }

    fn lit(&mut self, word: &[u8], v: Value) -> Result<Value> {
        if self.s[self.i..].starts_with(word) {
            self.i += word.len();
            Ok(v)
        } else {
            err(format!("json0: invalid token at byte {}", self.i))
        }
    }

    fn value(&mut self, depth: usize) -> Result<Value> {
        if depth > MAX_DEPTH {
            return err("json0: nesting too deep");
        }
        self.ws();
        match self.peek() {
            None => err("json0: unexpected end of input"),
            Some(b'n') => self.lit(b"null", Value::Null),
            Some(b't') => self.lit(b"true", Value::Bool(true)),
            Some(b'f') => self.lit(b"false", Value::Bool(false)),
            Some(b'"') => {
                let s = self.string()?;
                interpret_string(s)
            }
            Some(b'[') => {
                self.i += 1;
                let mut out = Vec::new();
                self.ws();
                if self.peek() == Some(b']') {
                    self.i += 1;
                    return Ok(Value::Array(out));
                }
                loop {
                    out.push(self.value(depth + 1)?);
                    self.ws();
                    match self.peek() {
                        Some(b',') => self.i += 1,
                        Some(b']') => {
                            self.i += 1;
                            return Ok(Value::Array(out));
                        }
                        _ => return err(format!("json0: expected ',' or ']' at byte {}", self.i)),
                    }
                }
            }
            Some(b'{') => {
                self.i += 1;
                let mut out: Vec<(String, Value)> = Vec::new();
                let mut seen = std::collections::HashSet::new();
                self.ws();
                if self.peek() == Some(b'}') {
                    self.i += 1;
                    return Ok(Value::Object(out));
                }
                loop {
                    self.ws();
                    if self.peek() != Some(b'"') {
                        return err(format!("json0: expected object key at byte {}", self.i));
                    }
                    let k = self.string()?;
                    if !seen.insert(k.clone()) {
                        return err(format!("json0: duplicate object key {k:?}"));
                    }
                    self.ws();
                    if self.peek() != Some(b':') {
                        return err(format!("json0: expected ':' at byte {}", self.i));
                    }
                    self.i += 1;
                    let v = self.value(depth + 1)?;
                    out.push((k, v));
                    self.ws();
                    match self.peek() {
                        Some(b',') => self.i += 1,
                        Some(b'}') => {
                            self.i += 1;
                            return Ok(Value::Object(out));
                        }
                        _ => return err(format!("json0: expected ',' or '}}' at byte {}", self.i)),
                    }
                }
            }
            Some(b'-' | b'0'..=b'9') => self.number(),
            Some(_) => err(format!("json0: unexpected character at byte {}", self.i)),
        }
    }

    fn number(&mut self) -> Result<Value> {
        let st = self.i;
        let s = self.s;
        let mut i = self.i;
        if s.get(i) == Some(&b'-') {
            i += 1;
        }
        match s.get(i) {
            Some(b'0') => i += 1,
            Some(b'1'..=b'9') => {
                while i < s.len() && s[i].is_ascii_digit() {
                    i += 1;
                }
            }
            _ => return err("json0: invalid number"),
        }
        if s.get(i) == Some(&b'.') {
            i += 1;
            let d = i;
            while i < s.len() && s[i].is_ascii_digit() {
                i += 1;
            }
            if i == d {
                return err("json0: invalid number");
            }
        }
        if matches!(s.get(i), Some(b'e' | b'E')) {
            i += 1;
            if matches!(s.get(i), Some(b'+' | b'-')) {
                i += 1;
            }
            let d = i;
            while i < s.len() && s[i].is_ascii_digit() {
                i += 1;
            }
            if i == d {
                return err("json0: invalid number");
            }
        }
        self.i = i;
        Ok(Value::Number(String::from_utf8(s[st..i].to_vec()).expect("ASCII")))
    }

    fn hex4(&mut self) -> Result<u32> {
        if self.i + 4 > self.s.len() {
            return err("json0: truncated \\u escape");
        }
        let mut v = 0;
        for k in 0..4 {
            let c = self.s[self.i + k];
            let d = (c as char).to_digit(16).ok_or_else(|| crate::CodecError::new("json0: invalid \\u escape"))?;
            v = v * 16 + d;
        }
        self.i += 4;
        Ok(v)
    }

    fn string(&mut self) -> Result<String> {
        self.i += 1; // opening quote
        let mut out: Vec<u8> = Vec::new();
        loop {
            let c = match self.peek() {
                None => return err("json0: unterminated string"),
                Some(c) => c,
            };
            self.i += 1;
            match c {
                b'"' => break,
                b'\\' => {
                    let e = self.peek().ok_or_else(|| crate::CodecError::new("json0: unterminated string"))?;
                    self.i += 1;
                    let ch = match e {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'b' => '\u{8}',
                        b'f' => '\u{c}',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'u' => {
                            let u = self.hex4()?;
                            let cp = if (0xd800..0xdc00).contains(&u) {
                                if self.s[self.i..].starts_with(b"\\u") {
                                    self.i += 2;
                                    let lo = self.hex4()?;
                                    if !(0xdc00..0xe000).contains(&lo) {
                                        return err("json0: lone surrogate");
                                    }
                                    0x10000 + ((u - 0xd800) << 10) + (lo - 0xdc00)
                                } else {
                                    return err("json0: lone surrogate");
                                }
                            } else if (0xdc00..0xe000).contains(&u) {
                                return err("json0: lone surrogate");
                            } else {
                                u
                            };
                            char::from_u32(cp).ok_or_else(|| crate::CodecError::new("json0: invalid code point"))?
                        }
                        _ => return err("json0: invalid escape"),
                    };
                    let mut b = [0u8; 4];
                    out.extend_from_slice(ch.encode_utf8(&mut b).as_bytes());
                }
                0..=0x1f => return err("json0: control character in string"),
                _ => out.push(c),
            }
        }
        // input was &str, and escapes produce valid UTF-8
        String::from_utf8(out).map_err(|_| crate::CodecError::new("json0: invalid UTF-8"))
    }
}

/// Parse JSON0 text strictly (RFC 8259 grammar, unique keys, scalar text).
pub fn parse(text: &str) -> Result<Value> {
    let mut p = Parser { s: text.as_bytes(), i: 0 };
    let v = p.value(0)?;
    p.ws();
    if p.i != p.s.len() {
        return err(format!("json0: trailing characters at byte {}", p.i));
    }
    Ok(v)
}

/// Parse JSON0 from bytes (must be UTF-8).
pub fn parse_bytes(b: &[u8]) -> Result<Value> {
    parse(std::str::from_utf8(b).map_err(|_| crate::CodecError::new("json0: input is not UTF-8"))?)
}

// ---------------------------------------------------------------------------
// Writer

fn write_str(out: &mut String, s: &str) {
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

fn write(out: &mut String, v: &Value) {
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => out.push_str(n),
        Value::String(s) => {
            if s.starts_with('0') {
                write_str(out, &format!("0{s}"));
            } else {
                write_str(out, s);
            }
        }
        Value::BigInt(n) => {
            let t = if n.sign() == Sign::Minus {
                format!("0~{}", complement(n).to_str_radix(16))
            } else {
                format!("0x{}", n.to_str_radix(16))
            };
            write_str(out, &t);
        }
        Value::Decimal(d) => write_str(out, &format!("0.{d}")),
        Value::Bytes(b) => write_str(out, &format!("0*{}", base64::encode_url(b))),
        Value::WellKnownSymbol(s) => write_str(out, &format!("0#{s}")),
        Value::GlobalSymbol(s) => write_str(out, &format!("0${s}")),
        Value::Array(a) => {
            out.push('[');
            for (i, x) in a.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write(out, x);
            }
            out.push(']');
        }
        Value::Object(o) => {
            out.push('{');
            for (i, (k, x)) in o.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_str(out, k);
                out.push(':');
                write(out, x);
            }
            out.push('}');
        }
    }
}

/// Serialize compactly. Bigints as `0x<hex>` / `0~<hex of ~n>`, bytes as
/// unpadded `0*` base64url, `0`-prefixed literal strings gain another `0`.
pub fn to_string(v: &Value) -> String {
    let mut out = String::new();
    write(&mut out, v);
    out
}

impl Value {
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Object(o) => o.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }
    pub fn as_bool(&self) -> Option<bool> {
        match *self {
            Value::Bool(b) => Some(b),
            _ => None,
        }
    }
    pub fn as_bigint(&self) -> Option<&BigInt> {
        match self {
            Value::BigInt(b) => Some(b),
            _ => None,
        }
    }
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Value::Bytes(b) => Some(b),
            _ => None,
        }
    }
    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Value::Array(a) => Some(a),
            _ => None,
        }
    }
    pub fn as_object(&self) -> Option<&[(String, Value)]> {
        match self {
            Value::Object(o) => Some(o),
            _ => None,
        }
    }
    /// Raw number text.
    pub fn as_number(&self) -> Option<&str> {
        match self {
            Value::Number(n) => Some(n),
            _ => None,
        }
    }
    /// Integer-valued JSON number (no fraction/exponent) as `i64`.
    pub fn as_i64(&self) -> Option<i64> {
        self.as_number().and_then(|n| n.parse().ok())
    }
    /// Integer-valued JSON number as `u64`.
    pub fn as_u64(&self) -> Option<u64> {
        self.as_number().and_then(|n| n.parse().ok())
    }
    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }
    pub fn object(entries: Vec<(&str, Value)>) -> Value {
        Value::Object(entries.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
    }
    pub fn str(s: impl Into<String>) -> Value {
        Value::String(s.into())
    }
    pub fn int(n: i64) -> Value {
        Value::Number(n.to_string())
    }
}

impl From<BigInt> for Value {
    fn from(b: BigInt) -> Self {
        Value::BigInt(b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn doc_examples() {
        let v = parse(r#"{"viewpoint":"0x0","timelines":{"basin":{"milestone":"0x1"}}}"#).unwrap();
        assert_eq!(v.get("viewpoint").unwrap().as_bigint(), Some(&BigInt::from(0)));
        assert_eq!(
            v.get("timelines").unwrap().get("basin").unwrap().get("milestone").unwrap().as_bigint(),
            Some(&BigInt::from(1))
        );
        assert_eq!(to_string(&v), r#"{"viewpoint":"0x0","timelines":{"basin":{"milestone":"0x1"}}}"#);
        let v = parse(r#"{"latest":{"viewpoint":"0x1","length":10,"headers":{"If-Match":"*","Range":"bytes=128-137"}}}"#).unwrap();
        assert_eq!(v.get("latest").unwrap().get("length").unwrap().as_u64(), Some(10));
    }

    #[test]
    fn prefixes() {
        let p = |s: &str| parse(&format!("\"{s}\""));
        assert_eq!(p("").unwrap(), Value::str(""));
        assert_eq!(p("00").unwrap(), Value::str("0"));
        assert_eq!(p("000x1").unwrap(), Value::str("00x1"));
        assert_eq!(p("0x1f").unwrap(), Value::BigInt(31.into()));
        assert_eq!(p("0xFF").unwrap(), Value::BigInt(255.into()));
        assert_eq!(p("0~0").unwrap(), Value::BigInt((-1).into()));
        assert_eq!(p("0~10").unwrap(), Value::BigInt((-17).into()));
        assert_eq!(p("0.-1.5e3").unwrap(), Value::Decimal("-1.5e3".into()));
        assert_eq!(p("0.12").unwrap(), Value::Decimal("12".into()));
        assert_eq!(p("0*-_8").unwrap(), Value::Bytes(vec![0xfb, 0xff]));
        assert_eq!(p("0*-_8=").unwrap(), Value::Bytes(vec![0xfb, 0xff]));
        assert_eq!(p("0%+/8=").unwrap(), Value::Bytes(vec![0xfb, 0xff]));
        assert_eq!(p("0*").unwrap(), Value::Bytes(vec![]));
        assert_eq!(p("0#species").unwrap(), Value::WellKnownSymbol("species".into()));
        assert_eq!(p("0$app.sym").unwrap(), Value::GlobalSymbol("app.sym".into()));
        assert_eq!(p("hello").unwrap(), Value::str("hello"));
        for bad in ["0w0z", "0zg0", "0yvf", "0A", "0Z1", "0_x", "0!", "0a", "01", "0x", "0xg", "0~",
                    "0.", "0.1.", "0.e5", "0*a", "0*+/", "0%-_", "0#foo", "0#Symbol.species", "0 ", "0"] {
            assert!(p(bad).is_err(), "{bad:?} accepted");
        }
        // object keys are not interpreted
        let o = parse(r#"{"0x1":"a","0w":1}"#).unwrap();
        assert_eq!(o.get("0x1").unwrap(), &Value::str("a"));
    }

    #[test]
    fn writer() {
        assert_eq!(to_string(&Value::BigInt((-1).into())), "\"0~0\"");
        assert_eq!(to_string(&Value::BigInt(255.into())), "\"0xff\"");
        assert_eq!(to_string(&Value::Bytes(vec![0xfb, 0xff])), "\"0*-_8\"");
        assert_eq!(to_string(&Value::str("0x1")), "\"00x1\"");
        assert_eq!(to_string(&Value::str("0")), "\"00\"");
        assert_eq!(to_string(&Value::str("a\"\n\u{1}")), "\"a\\\"\\n\\u0001\"");
        assert_eq!(to_string(&Value::WellKnownSymbol("species".into())), "\"0#species\"");
        assert_eq!(to_string(&Value::Decimal("1.5".into())), "\"0.1.5\"");
    }

    #[test]
    fn strict_json() {
        for bad in ["", " ", "{", "[1,]", "{\"a\":1,}", "{\"a\":1,\"a\":2}", "01", "1.", "-", "+1", ".5",
                    "1e", "\"\\ud800\"", "\"\\udc00\"", "\"\\ud800\\u0041\"", "\"a\nb\"", "\"\\x\"",
                    "nul", "true false", "[1] x", "'a'", "NaN", "{1:2}"] {
            assert!(parse(bad).is_err(), "{bad:?} accepted");
        }
        assert_eq!(parse("\"\\ud83d\\ude00\"").unwrap(), Value::str("😀"));
        assert_eq!(parse(" [1, -0.5e+3, true, null] ").unwrap(),
                   Value::Array(vec![Value::int(1), Value::Number("-0.5e+3".into()), Value::Bool(true), Value::Null]));
        let deep = "[".repeat(200) + &"]".repeat(200);
        assert!(parse(&deep).is_err());
    }

    fn arb() -> impl Strategy<Value = Value> {
        let leaf = prop_oneof![
            Just(Value::Null),
            any::<bool>().prop_map(Value::Bool),
            any::<i64>().prop_map(Value::int),
            any::<String>().prop_map(Value::String),
            "0[0-9a-z~.*]{0,4}".prop_map(Value::String),
            any::<i128>().prop_map(|i| Value::BigInt(i.into())),
            proptest::collection::vec(any::<u8>(), 0..20).prop_map(Value::Bytes),
            any::<String>().prop_map(Value::GlobalSymbol),
            "-?[0-9]{1,5}(\\.[0-9]{1,3})?(e[+-]?[0-9]{1,2})?".prop_map(Value::Decimal),
        ];
        leaf.prop_recursive(3, 32, 6, |inner| {
            prop_oneof![
                proptest::collection::vec(inner.clone(), 0..6).prop_map(Value::Array),
                proptest::collection::btree_map(any::<String>(), inner, 0..5)
                    .prop_map(|m| Value::Object(m.into_iter().collect())),
            ]
        })
    }

    proptest! {
        #[test]
        fn roundtrip(v in arb()) {
            let s = to_string(&v);
            prop_assert_eq!(parse(&s).unwrap(), v);
        }
        #[test]
        fn no_panic(s in any::<String>()) {
            let _ = parse(&s);
        }
    }
}

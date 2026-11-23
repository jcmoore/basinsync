//! FlexBuffers (<https://flatbuffers.dev/flexbuffers/>) with the basinsync
//! `VT_VECTOR_UINT` extensions (docs/glossary/flexbuffers.md).
//!
//! The encoder mirrors the reference builder (minimal widths, standard
//! alignment, sorted map keys, shared keys, root value + packed type + root
//! byte width). The decoder accepts any valid layout: every width, sharing,
//! indirect scalars, typed and fixed-length vectors. It is fully bounds
//! checked, depth limited, and limits decoded output to defend against
//! shared-reference blowups.
//!
//! `VT_VECTOR_UINT` is reserved for extensions; its first element is one
//! ASCII prefix byte: `x` nonnegative bigint (big-endian minimal magnitude),
//! `~` magnitude of `~n` for negative `n`, `#` well-known symbol name, `$`
//! global symbol name.

use crate::bigint::complement;
use crate::error::{err, Result};
use num_bigint::{BigInt, BigUint, Sign};
use std::collections::HashMap;
use std::ops::Range;

pub const FBT_NULL: u8 = 0;
pub const FBT_INT: u8 = 1;
pub const FBT_UINT: u8 = 2;
pub const FBT_FLOAT: u8 = 3;
pub const FBT_KEY: u8 = 4;
pub const FBT_STRING: u8 = 5;
pub const FBT_INDIRECT_INT: u8 = 6;
pub const FBT_INDIRECT_UINT: u8 = 7;
pub const FBT_INDIRECT_FLOAT: u8 = 8;
pub const FBT_MAP: u8 = 9;
pub const FBT_VECTOR: u8 = 10;
pub const FBT_VECTOR_INT: u8 = 11;
pub const FBT_VECTOR_UINT: u8 = 12;
pub const FBT_VECTOR_FLOAT: u8 = 13;
pub const FBT_VECTOR_KEY: u8 = 14;
pub const FBT_VECTOR_STRING_DEPRECATED: u8 = 15;
pub const FBT_VECTOR_INT2: u8 = 16;
pub const FBT_VECTOR_UINT2: u8 = 17;
pub const FBT_VECTOR_FLOAT2: u8 = 18;
pub const FBT_VECTOR_INT3: u8 = 19;
pub const FBT_VECTOR_UINT3: u8 = 20;
pub const FBT_VECTOR_FLOAT3: u8 = 21;
pub const FBT_VECTOR_INT4: u8 = 22;
pub const FBT_VECTOR_UINT4: u8 = 23;
pub const FBT_VECTOR_FLOAT4: u8 = 24;
pub const FBT_BLOB: u8 = 25;
pub const FBT_BOOL: u8 = 26;
pub const FBT_VECTOR_BOOL: u8 = 36;

/// Extension prefixes.
pub const EXT_BIGINT: u8 = b'x';
pub const EXT_NEG_BIGINT: u8 = b'~';
pub const EXT_WELL_KNOWN_SYMBOL: u8 = b'#';
pub const EXT_GLOBAL_SYMBOL: u8 = b'$';

/// Well-known symbol names (JSON0).
pub const WELL_KNOWN_SYMBOLS: [&str; 4] = ["asyncIterator", "iterator", "species", "unscopables"];

/// Maximum nesting depth accepted by the decoder and encoder.
pub const MAX_DEPTH: usize = 128;

/// A decoded FlexBuffers value.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    UInt(u64),
    Float(f64),
    String(String),
    Blob(Vec<u8>),
    /// Untyped `VT_VECTOR` (typed float/bool/key/fixed-uint vectors also
    /// decode to this).
    Vector(Vec<Value>),
    /// `VT_VECTOR_INT` (fixed-length int vectors also decode to this).
    IntVector(Vec<i64>),
    /// Keys sorted bytewise; the encoder sorts, the decoder returns sorted.
    Map(Vec<(String, Value)>),
    /// `x`/`~` extension.
    BigInt(BigInt),
    /// `#` extension, bare name such as `species`.
    WellKnownSymbol(String),
    /// `$` extension.
    GlobalSymbol(String),
}

impl Value {
    pub fn species() -> Value {
        Value::WellKnownSymbol("species".into())
    }
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Map(m) => m.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }
    pub fn as_i64(&self) -> Option<i64> {
        match *self {
            Value::Int(i) => Some(i),
            Value::UInt(u) => i64::try_from(u).ok(),
            _ => None,
        }
    }
    pub fn as_u64(&self) -> Option<u64> {
        match *self {
            Value::Int(i) => u64::try_from(i).ok(),
            Value::UInt(u) => Some(u),
            _ => None,
        }
    }
    pub fn as_f64(&self) -> Option<f64> {
        match *self {
            Value::Float(f) => Some(f),
            _ => None,
        }
    }
    pub fn as_bool(&self) -> Option<bool> {
        match *self {
            Value::Bool(b) => Some(b),
            _ => None,
        }
    }
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Value::Blob(b) => Some(b),
            _ => None,
        }
    }
    pub fn as_vec(&self) -> Option<&[Value]> {
        match self {
            Value::Vector(v) => Some(v),
            _ => None,
        }
    }
    pub fn as_int_vec(&self) -> Option<&[i64]> {
        match self {
            Value::IntVector(v) => Some(v),
            _ => None,
        }
    }
    pub fn as_map(&self) -> Option<&[(String, Value)]> {
        match self {
            Value::Map(m) => Some(m),
            _ => None,
        }
    }
    pub fn as_bigint(&self) -> Option<&BigInt> {
        match self {
            Value::BigInt(b) => Some(b),
            _ => None,
        }
    }
    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }
    pub fn is_well_known_symbol(&self, name: &str) -> bool {
        matches!(self, Value::WellKnownSymbol(n) if n == name)
    }
}

// ---------------------------------------------------------------------------
// Extension core (Kani-verified)

/// Validate an extension payload for `prefix`: known prefix, minimal
/// magnitude (zero is one zero byte), UTF-8 names, well-known names from the
/// JSON0 set.
pub fn check_extension_payload(prefix: u8, payload: &[u8]) -> Result<()> {
    match prefix {
        EXT_BIGINT | EXT_NEG_BIGINT => {
            if payload.is_empty() {
                return err("flexbuffers: empty bigint magnitude");
            }
            if payload.len() > 1 && payload[0] == 0 {
                return err("flexbuffers: noncanonical bigint magnitude");
            }
            Ok(())
        }
        EXT_WELL_KNOWN_SYMBOL => {
            if WELL_KNOWN_SYMBOLS.iter().any(|w| w.as_bytes() == payload) {
                Ok(())
            } else {
                err("flexbuffers: unknown well-known symbol")
            }
        }
        EXT_GLOBAL_SYMBOL => {
            if is_utf8(payload) {
                Ok(())
            } else {
                err("flexbuffers: global symbol name is not UTF-8")
            }
        }
        _ => err("flexbuffers: unknown VT_VECTOR_UINT extension prefix"),
    }
}

/// Convert validated bytes to a String without std's (Kani-expensive)
/// validator.
fn utf8_string(bytes: Vec<u8>) -> Option<String> {
    if is_utf8(&bytes) {
        // SAFETY: `is_utf8` accepts exactly the valid UTF-8 byte strings
        // (tests compare it with `std::str::from_utf8`).
        Some(unsafe { String::from_utf8_unchecked(bytes) })
    } else {
        None
    }
}

/// Strict UTF-8 validation (same acceptance as `std::str::from_utf8`),
/// written as a plain loop so Kani can reason about it.
pub fn is_utf8(b: &[u8]) -> bool {
    is_utf8_by(b.len(), |i| b[i])
}

/// [`is_utf8`] over an indexed byte source of length `len`.
pub fn is_utf8_by(len: usize, at: impl Fn(usize) -> u8) -> bool {
    let mut i = 0;
    while i < len {
        let c = at(i);
        let (n, lo, hi) = match c {
            0x00..=0x7f => {
                i += 1;
                continue;
            }
            0xc2..=0xdf => (1, 0x80, 0xbf),
            0xe0 => (2, 0xa0, 0xbf),
            0xe1..=0xec | 0xee..=0xef => (2, 0x80, 0xbf),
            0xed => (2, 0x80, 0x9f),
            0xf0 => (3, 0x90, 0xbf),
            0xf1..=0xf3 => (3, 0x80, 0xbf),
            0xf4 => (3, 0x80, 0x8f),
            _ => return false,
        };
        if i + n >= len {
            return false;
        }
        if at(i + 1) < lo || at(i + 1) > hi {
            return false;
        }
        let mut k = 2;
        while k <= n {
            if at(i + k) & 0xc0 != 0x80 {
                return false;
            }
            k += 1;
        }
        i += n + 1;
    }
    true
}

fn width_u(u: u64) -> u8 {
    if u & !0xff == 0 {
        0
    } else if u & !0xffff == 0 {
        1
    } else if u & !0xffff_ffff == 0 {
        2
    } else {
        3
    }
}

fn width_i(i: i64) -> u8 {
    let u = (i as u64) << 1;
    width_u(if i >= 0 { u } else { !u })
}

fn width_f(f: f64) -> u8 {
    if (f as f32) as f64 == f {
        2
    } else {
        3
    }
}

fn padding(len: usize, byte_width: usize) -> usize {
    (!len).wrapping_add(1) & (byte_width - 1)
}

fn push_uint(buf: &mut Vec<u8>, v: u64, byte_width: usize) {
    buf.extend_from_slice(&v.to_le_bytes()[..byte_width]);
}

/// Byte width (1, 2, 4, 8) of an offset written at index `index` of a
/// vector whose data would begin after `buf_len` (plus alignment) and point
/// back to `loc`.
fn offset_bit_width(buf_len: usize, loc: usize, index: usize) -> u8 {
    for bw in 0..4u8 {
        let byte_width = 1usize << bw;
        let offset_loc = buf_len + padding(buf_len, byte_width) + index * byte_width;
        let offset = (offset_loc - loc) as u64;
        if width_u(offset) <= bw {
            return bw;
        }
    }
    3
}

/// Encode an extension directly (the same bytes the general builder
/// produces for a root extension vector).
pub fn encode_extension_bytes(prefix: u8, payload: &[u8]) -> Result<Vec<u8>> {
    let mut out = vec![0u8; extension_encoded_len(payload.len())];
    let n = encode_extension_into(prefix, payload, &mut out)?;
    out.truncate(n);
    Ok(out)
}

/// Upper bound on the encoded size of an extension with this payload length.
pub fn extension_encoded_len(payload_len: usize) -> usize {
    (payload_len + 2) * 8 + 16
}

fn put_uint(out: &mut [u8], at: usize, v: u64, byte_width: usize) {
    let mut i = 0;
    while i < byte_width {
        out[at + i] = (v >> (8 * i)) as u8;
        i += 1;
    }
}

/// Allocation-free encoder core: writes the extension buffer into `out`
/// (which must hold `extension_encoded_len(payload.len())` bytes) and
/// returns its length.
pub fn encode_extension_into(prefix: u8, payload: &[u8], out: &mut [u8]) -> Result<usize> {
    check_extension_payload(prefix, payload)?;
    if out.len() < extension_encoded_len(payload.len()) {
        return err("flexbuffers: output buffer too small");
    }
    let n = payload.len() + 1;
    let bw = width_u(n as u64);
    let w = 1usize << bw;
    let mut len = 0;
    put_uint(out, len, n as u64, w);
    len += w;
    let vloc = len;
    put_uint(out, len, prefix as u64, w);
    len += w;
    for &b in payload {
        put_uint(out, len, b as u64, w);
        len += w;
    }
    let rbw = offset_bit_width(len, vloc, 0);
    let rw = 1usize << rbw;
    let pad = padding(len, rw);
    let mut i = 0;
    while i < pad {
        out[len + i] = 0;
        i += 1;
    }
    len += pad;
    let off = (len - vloc) as u64;
    put_uint(out, len, off, rw);
    len += rw;
    out[len] = FBT_VECTOR_UINT << 2 | bw;
    out[len + 1] = rw as u8;
    Ok(len + 2)
}

fn read_uint(buf: &[u8], pos: usize, width: usize) -> Result<u64> {
    let end = match pos.checked_add(width) {
        Some(e) if e <= buf.len() => e,
        _ => return err("flexbuffers: read out of bounds"),
    };
    // Explicit loop (rather than a symbolic-length copy) keeps Kani tractable.
    let mut v: u64 = 0;
    let mut i = 0;
    while i < width && i < 8 {
        v |= (buf[pos + i] as u64) << (8 * i);
        i += 1;
    }
    let _ = end;
    Ok(v)
}

fn read_int(buf: &[u8], pos: usize, width: usize) -> Result<i64> {
    let u = read_uint(buf, pos, width)?;
    let shift = 64 - 8 * width as u32;
    Ok(((u << shift) as i64) >> shift)
}

fn read_float(buf: &[u8], pos: usize, width: usize) -> Result<f64> {
    match width {
        4 => Ok(f32::from_bits(read_uint(buf, pos, 4)? as u32) as f64),
        8 => Ok(f64::from_bits(read_uint(buf, pos, 8)?)),
        _ => err("flexbuffers: float must be 4 or 8 bytes wide"),
    }
}

fn deref(buf: &[u8], pos: usize, width: usize) -> Result<usize> {
    let off = read_uint(buf, pos, width)?;
    if off > pos as u64 {
        return err("flexbuffers: offset out of bounds");
    }
    Ok(pos - off as usize)
}

/// Size field preceding the data at `target`, with the element region
/// `target .. target + size * width` bounds-checked.
fn sized(buf: &[u8], target: usize, width: usize, elem_bytes: usize) -> Result<usize> {
    if target < width {
        return err("flexbuffers: size field out of bounds");
    }
    let size = read_uint(buf, target - width, width)?;
    check_region(buf, target, size, elem_bytes)?;
    Ok(size as usize)
}

fn check_region(buf: &[u8], target: usize, count: u64, elem_bytes: usize) -> Result<()> {
    let ok = count
        .checked_mul(elem_bytes as u64)
        .and_then(|b| b.checked_add(target as u64))
        .map(|e| e <= buf.len() as u64)
        .unwrap_or(false);
    if ok {
        Ok(())
    } else {
        err("flexbuffers: vector out of bounds")
    }
}

/// Root reference: (position, parent byte width, packed type).
fn root(buf: &[u8]) -> Result<(usize, usize, u8)> {
    let n = buf.len();
    if n < 3 {
        return err("flexbuffers: buffer too short");
    }
    let bw = buf[n - 1] as usize;
    if !matches!(bw, 1 | 2 | 4 | 8) {
        return err("flexbuffers: invalid root byte width");
    }
    let packed = buf[n - 2];
    if n - 2 < bw {
        return err("flexbuffers: buffer too short for root");
    }
    Ok((n - 2 - bw, bw, packed))
}

/// Location of a validated extension: prefix, first element position,
/// element width, payload length.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExtensionView {
    pub prefix: u8,
    pub target: usize,
    pub width: usize,
    pub payload_len: usize,
}

impl ExtensionView {
    /// Payload byte `i` (validated to fit one byte).
    pub fn byte(&self, buf: &[u8], i: usize) -> u8 {
        buf[self.target + (i + 1) * self.width]
    }
}

fn ext_elem(buf: &[u8], target: usize, w: usize, i: usize) -> Result<u8> {
    let v = read_uint(buf, target + i * w, w)?;
    if v > 0xff {
        return err("flexbuffers: extension element exceeds one byte");
    }
    Ok(v as u8)
}

/// Allocation-free validation of the `VT_VECTOR_UINT` at `target`.
fn view_extension(buf: &[u8], target: usize, w: usize) -> Result<ExtensionView> {
    let size = sized(buf, target, w, w)?;
    if size == 0 {
        return err("flexbuffers: VT_VECTOR_UINT without extension prefix");
    }
    let mut i = 0;
    while i < size {
        ext_elem(buf, target, w, i)?;
        i += 1;
    }
    let prefix = ext_elem(buf, target, w, 0)?;
    let v = ExtensionView { prefix, target, width: w, payload_len: size - 1 };
    let n = v.payload_len;
    match prefix {
        EXT_BIGINT | EXT_NEG_BIGINT => {
            if n == 0 {
                return err("flexbuffers: empty bigint magnitude");
            }
            if n > 1 && v.byte(buf, 0) == 0 {
                return err("flexbuffers: noncanonical bigint magnitude");
            }
        }
        EXT_WELL_KNOWN_SYMBOL => {
            let known = WELL_KNOWN_SYMBOLS.iter().any(|name| {
                let nb = name.as_bytes();
                nb.len() == n && (0..n).all(|i| nb[i] == v.byte(buf, i))
            });
            if !known {
                return err("flexbuffers: unknown well-known symbol");
            }
        }
        EXT_GLOBAL_SYMBOL => {
            if !is_utf8_by(n, |i| v.byte(buf, i)) {
                return err("flexbuffers: global symbol name is not UTF-8");
            }
        }
        _ => return err("flexbuffers: unknown VT_VECTOR_UINT extension prefix"),
    }
    Ok(v)
}

/// Allocation-free decoder core: validate a buffer whose root must be an
/// extension and locate its payload.
pub fn validate_extension(buf: &[u8]) -> Result<ExtensionView> {
    let (pos, pw, packed) = root(buf)?;
    if packed >> 2 != FBT_VECTOR_UINT {
        return err("flexbuffers: root is not a VT_VECTOR_UINT extension");
    }
    let target = deref(buf, pos, pw)?;
    view_extension(buf, target, 1 << (packed & 3))
}

fn read_extension(buf: &[u8], target: usize, w: usize) -> Result<(u8, Vec<u8>)> {
    let v = view_extension(buf, target, w)?;
    Ok((v.prefix, (0..v.payload_len).map(|i| v.byte(buf, i)).collect()))
}

/// Decode a buffer whose root is an extension vector into (prefix, payload).
pub fn decode_extension_bytes(buf: &[u8]) -> Result<(u8, Vec<u8>)> {
    let v = validate_extension(buf)?;
    Ok((v.prefix, (0..v.payload_len).map(|i| v.byte(buf, i)).collect()))
}

fn extension_value(prefix: u8, payload: Vec<u8>) -> Value {
    match prefix {
        EXT_BIGINT => Value::BigInt(BigInt::from_biguint(Sign::Plus, BigUint::from_bytes_be(&payload))),
        EXT_NEG_BIGINT => {
            Value::BigInt(complement(&BigInt::from_biguint(Sign::Plus, BigUint::from_bytes_be(&payload))))
        }
        EXT_WELL_KNOWN_SYMBOL => Value::WellKnownSymbol(utf8_string(payload).expect("checked")),
        _ => Value::GlobalSymbol(utf8_string(payload).expect("checked")),
    }
}

/// (prefix, payload) for an extension value; errors for other values.
pub fn extension_parts(v: &Value) -> Result<(u8, Vec<u8>)> {
    match v {
        Value::BigInt(n) => {
            if n.sign() == Sign::Minus {
                Ok((EXT_NEG_BIGINT, complement(n).to_bytes_be().1))
            } else {
                Ok((EXT_BIGINT, n.to_bytes_be().1))
            }
        }
        Value::WellKnownSymbol(s) => Ok((EXT_WELL_KNOWN_SYMBOL, s.as_bytes().to_vec())),
        Value::GlobalSymbol(s) => Ok((EXT_GLOBAL_SYMBOL, s.as_bytes().to_vec())),
        _ => err("flexbuffers: not an extension value"),
    }
}

/// Encode a bigint or symbol as a standalone FlexBuffers extension buffer.
pub fn encode_extension(v: &Value) -> Result<Vec<u8>> {
    let (p, payload) = extension_parts(v)?;
    encode_extension_bytes(p, &payload)
}

/// Decode a FlexBuffers buffer whose root must be an extension.
pub fn decode_extension(buf: &[u8]) -> Result<Value> {
    let (p, payload) = decode_extension_bytes(buf)?;
    Ok(extension_value(p, payload))
}

// ---------------------------------------------------------------------------
// Encoder

#[derive(Clone, Copy)]
struct Sv {
    ty: u8,
    /// Bit width code 0..=3.
    w: u8,
    /// Inline payload bits, or the location an offset points to.
    data: u64,
}

fn is_inline(t: u8) -> bool {
    t <= FBT_FLOAT || t == FBT_BOOL
}

impl Sv {
    fn elem_width(&self, buf_len: usize, index: usize) -> u8 {
        if is_inline(self.ty) {
            self.w
        } else {
            offset_bit_width(buf_len, self.data as usize, index)
        }
    }
    fn stored_packed_type(&self, parent_bw: u8) -> u8 {
        let w = if is_inline(self.ty) { self.w.max(parent_bw) } else { self.w };
        self.ty << 2 | w
    }
}

struct Builder {
    buf: Vec<u8>,
    stack: Vec<Sv>,
    keys: HashMap<Vec<u8>, usize>,
}

impl Builder {
    fn align(&mut self, bw: u8) -> usize {
        let byte_width = 1usize << bw;
        let pad = padding(self.buf.len(), byte_width);
        self.buf.resize(self.buf.len() + pad, 0);
        byte_width
    }

    fn write_any(&mut self, sv: Sv, byte_width: usize) {
        match sv.ty {
            FBT_NULL | FBT_INT | FBT_BOOL | FBT_UINT => push_uint(&mut self.buf, sv.data, byte_width),
            FBT_FLOAT => {
                let f = f64::from_bits(sv.data);
                if byte_width == 4 {
                    push_uint(&mut self.buf, (f as f32).to_bits() as u64, 4)
                } else {
                    push_uint(&mut self.buf, f.to_bits(), 8)
                }
            }
            _ => {
                let off = (self.buf.len() - sv.data as usize) as u64;
                push_uint(&mut self.buf, off, byte_width)
            }
        }
    }

    fn blob(&mut self, data: &[u8], trailing: usize, ty: u8) {
        let bw = width_u(data.len() as u64);
        let byte_width = self.align(bw);
        push_uint(&mut self.buf, data.len() as u64, byte_width);
        let loc = self.buf.len();
        self.buf.extend_from_slice(data);
        self.buf.resize(self.buf.len() + trailing, 0);
        self.stack.push(Sv { ty, w: bw, data: loc as u64 });
    }

    fn key(&mut self, k: &str) -> Result<()> {
        if k.as_bytes().contains(&0) {
            return err("flexbuffers: map key contains NUL");
        }
        let loc = match self.keys.get(k.as_bytes()) {
            Some(&l) => l,
            None => {
                let l = self.buf.len();
                self.buf.extend_from_slice(k.as_bytes());
                self.buf.push(0);
                self.keys.insert(k.as_bytes().to_vec(), l);
                l
            }
        };
        self.stack.push(Sv { ty: FBT_KEY, w: 0, data: loc as u64 });
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn create_vector(&mut self, start: usize, len: usize, step: usize, typed: Option<u8>, keys: Option<Sv>) -> Sv {
        let mut bw = width_u(len as u64);
        let mut prefix = 1;
        if let Some(k) = keys {
            bw = bw.max(k.elem_width(self.buf.len(), 0));
            prefix += 2;
        }
        let mut i = start;
        while i < self.stack.len() {
            bw = bw.max(self.stack[i].elem_width(self.buf.len(), i - start + prefix));
            i += step;
        }
        let byte_width = self.align(bw);
        if let Some(k) = keys {
            self.write_any(k, byte_width);
            push_uint(&mut self.buf, 1u64 << k.w, byte_width);
        }
        push_uint(&mut self.buf, len as u64, byte_width);
        let vloc = self.buf.len();
        let mut i = start;
        while i < self.stack.len() {
            let sv = self.stack[i];
            self.write_any(sv, byte_width);
            i += step;
        }
        if typed.is_none() {
            let mut i = start;
            while i < self.stack.len() {
                let t = self.stack[i].stored_packed_type(bw);
                self.buf.push(t);
                i += step;
            }
        }
        let ty = match (keys, typed) {
            (Some(_), _) => FBT_MAP,
            (None, Some(t)) => t,
            (None, None) => FBT_VECTOR,
        };
        Sv { ty, w: bw, data: vloc as u64 }
    }

    fn push(&mut self, v: &Value, depth: usize) -> Result<()> {
        if depth > MAX_DEPTH {
            return err("flexbuffers: nesting too deep");
        }
        match v {
            Value::Null => self.stack.push(Sv { ty: FBT_NULL, w: 0, data: 0 }),
            Value::Bool(b) => self.stack.push(Sv { ty: FBT_BOOL, w: 0, data: *b as u64 }),
            Value::Int(i) => self.stack.push(Sv { ty: FBT_INT, w: width_i(*i), data: *i as u64 }),
            Value::UInt(u) => self.stack.push(Sv { ty: FBT_UINT, w: width_u(*u), data: *u }),
            Value::Float(f) => self.stack.push(Sv { ty: FBT_FLOAT, w: width_f(*f), data: f.to_bits() }),
            Value::String(s) => self.blob(s.as_bytes(), 1, FBT_STRING),
            Value::Blob(b) => self.blob(b, 0, FBT_BLOB),
            Value::Vector(items) => {
                let start = self.stack.len();
                for it in items {
                    self.push(it, depth + 1)?;
                }
                let sv = self.create_vector(start, items.len(), 1, None, None);
                self.stack.truncate(start);
                self.stack.push(sv);
            }
            Value::IntVector(items) => {
                let start = self.stack.len();
                for &i in items {
                    self.stack.push(Sv { ty: FBT_INT, w: width_i(i), data: i as u64 });
                }
                let sv = self.create_vector(start, items.len(), 1, Some(FBT_VECTOR_INT), None);
                self.stack.truncate(start);
                self.stack.push(sv);
            }
            Value::Map(entries) => {
                let mut sorted: Vec<&(String, Value)> = entries.iter().collect();
                sorted.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
                for w in sorted.windows(2) {
                    if w[0].0 == w[1].0 {
                        return err(format!("flexbuffers: duplicate map key {:?}", w[0].0));
                    }
                }
                let start = self.stack.len();
                for (k, v) in sorted {
                    self.key(k)?;
                    self.push(v, depth + 1)?;
                }
                let len = (self.stack.len() - start) / 2;
                let keys = self.create_vector(start, len, 2, Some(FBT_VECTOR_KEY), None);
                let vals = self.create_vector(start + 1, len, 2, None, Some(keys));
                self.stack.truncate(start);
                self.stack.push(vals);
            }
            Value::BigInt(_) | Value::WellKnownSymbol(_) | Value::GlobalSymbol(_) => {
                let (p, payload) = extension_parts(v)?;
                check_extension_payload(p, &payload)?;
                let start = self.stack.len();
                self.stack.push(Sv { ty: FBT_UINT, w: 0, data: p as u64 });
                for &b in &payload {
                    self.stack.push(Sv { ty: FBT_UINT, w: 0, data: b as u64 });
                }
                let sv = self.create_vector(start, payload.len() + 1, 1, Some(FBT_VECTOR_UINT), None);
                self.stack.truncate(start);
                self.stack.push(sv);
            }
        }
        Ok(())
    }

    fn finish(mut self) -> Vec<u8> {
        let root = self.stack[0];
        let bw = root.elem_width(self.buf.len(), 0);
        let byte_width = self.align(bw);
        self.write_any(root, byte_width);
        self.buf.push(root.stored_packed_type(0));
        self.buf.push(byte_width as u8);
        self.buf
    }
}

/// Encode a complete FlexBuffers buffer with `v` as its root.
pub fn encode(v: &Value) -> Result<Vec<u8>> {
    let mut b = Builder { buf: Vec::new(), stack: Vec::new(), keys: HashMap::new() };
    b.push(v, 0)?;
    Ok(b.finish())
}

// ---------------------------------------------------------------------------
// Decoder

struct Reader<'a> {
    buf: &'a [u8],
    budget: usize,
}

impl<'a> Reader<'a> {
    fn spend(&mut self, n: usize) -> Result<()> {
        match self.budget.checked_sub(n.saturating_add(1)) {
            Some(b) => {
                self.budget = b;
                Ok(())
            }
            None => err("flexbuffers: decoded output too large (shared references?)"),
        }
    }

    fn key_at(&mut self, pos: usize) -> Result<String> {
        let rest = self.buf.get(pos..).unwrap_or(&[]);
        let n = rest
            .iter()
            .position(|&b| b == 0)
            .ok_or_else(|| crate::CodecError::new("flexbuffers: unterminated key"))?;
        self.spend(n)?;
        utf8_string(rest[..n].to_vec()).ok_or_else(|| crate::CodecError::new("flexbuffers: key is not UTF-8"))
    }

    fn scalar(&mut self, ty: u8, pos: usize, width: usize) -> Result<Value> {
        self.spend(0)?;
        let buf = self.buf;
        Ok(match ty {
            FBT_NULL => Value::Null,
            FBT_INT => Value::Int(read_int(buf, pos, width)?),
            FBT_UINT => Value::UInt(read_uint(buf, pos, width)?),
            FBT_FLOAT => Value::Float(read_float(buf, pos, width)?),
            FBT_BOOL => Value::Bool(read_uint(buf, pos, width)? != 0),
            FBT_KEY => {
                let t = deref(buf, pos, width)?;
                Value::String(self.key_at(t)?)
            }
            _ => return err("flexbuffers: not a scalar type"),
        })
    }

    fn value(&mut self, pos: usize, pw: usize, packed: u8, depth: usize) -> Result<Value> {
        if depth > MAX_DEPTH {
            return err("flexbuffers: nesting too deep");
        }
        let ty = packed >> 2;
        let w = 1usize << (packed & 3);
        let buf = self.buf;
        match ty {
            FBT_NULL | FBT_INT | FBT_UINT | FBT_FLOAT | FBT_BOOL | FBT_KEY => self.scalar(ty, pos, pw),
            FBT_INDIRECT_INT | FBT_INDIRECT_UINT | FBT_INDIRECT_FLOAT => {
                let t = deref(buf, pos, pw)?;
                self.scalar(ty - FBT_INDIRECT_INT + FBT_INT, t, w)
            }
            FBT_STRING | FBT_BLOB => {
                let t = deref(buf, pos, pw)?;
                let n = sized(buf, t, w, 1)?;
                self.spend(n)?;
                let bytes = buf[t..t + n].to_vec();
                if ty == FBT_BLOB {
                    Ok(Value::Blob(bytes))
                } else {
                    utf8_string(bytes)
                        .map(Value::String)
                        .ok_or_else(|| crate::CodecError::new("flexbuffers: string is not UTF-8"))
                }
            }
            FBT_VECTOR => {
                let t = deref(buf, pos, pw)?;
                let n = sized(buf, t, w, w + 1)?;
                self.spend(n)?;
                let mut out = Vec::with_capacity(n);
                for i in 0..n {
                    let p = buf[t + n * w + i];
                    out.push(self.value(t + i * w, w, p, depth + 1)?);
                }
                Ok(Value::Vector(out))
            }
            FBT_MAP => {
                let t = deref(buf, pos, pw)?;
                let n = sized(buf, t, w, w + 1)?;
                if t < 3 * w {
                    return err("flexbuffers: map header out of bounds");
                }
                let kt = deref(buf, t - 3 * w, w)?;
                let kw = read_uint(buf, t - 2 * w, w)?;
                if !matches!(kw, 1 | 2 | 4 | 8) {
                    return err("flexbuffers: invalid map key width");
                }
                let kw = kw as usize;
                let kn = sized(buf, kt, kw, kw)?;
                if kn != n {
                    return err("flexbuffers: map key and value counts differ");
                }
                self.spend(n)?;
                let mut out = Vec::with_capacity(n);
                for i in 0..n {
                    let kpos = deref(buf, kt + i * kw, kw)?;
                    let k = self.key_at(kpos)?;
                    let p = buf[t + n * w + i];
                    out.push((k, self.value(t + i * w, w, p, depth + 1)?));
                }
                out.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
                for pair in out.windows(2) {
                    if pair[0].0 == pair[1].0 {
                        return err("flexbuffers: duplicate map key");
                    }
                }
                Ok(Value::Map(out))
            }
            FBT_VECTOR_UINT => {
                let t = deref(buf, pos, pw)?;
                let (p, payload) = read_extension(buf, t, w)?;
                self.spend(payload.len())?;
                Ok(extension_value(p, payload))
            }
            FBT_VECTOR_INT | FBT_VECTOR_FLOAT | FBT_VECTOR_KEY | FBT_VECTOR_STRING_DEPRECATED
            | FBT_VECTOR_BOOL => {
                let t = deref(buf, pos, pw)?;
                let n = sized(buf, t, w, w)?;
                self.typed(t, n, w, ty, depth)
            }
            FBT_VECTOR_INT2..=FBT_VECTOR_FLOAT4 => {
                let k = (ty - FBT_VECTOR_INT2) / 3;
                let n = 2 + k as usize;
                let elem_ty = (ty - FBT_VECTOR_INT2) % 3 + FBT_INT;
                let t = deref(buf, pos, pw)?;
                check_region(buf, t, n as u64, w)?;
                let vt = match elem_ty {
                    FBT_INT => FBT_VECTOR_INT,
                    FBT_UINT => FBT_VECTOR_UINT2, // fixed uint vectors are ordinary data
                    _ => FBT_VECTOR_FLOAT,
                };
                self.typed(t, n, w, vt, depth)
            }
            _ => err(format!("flexbuffers: unsupported type {ty}")),
        }
    }

    fn typed(&mut self, t: usize, n: usize, w: usize, vt: u8, depth: usize) -> Result<Value> {
        if depth + 1 > MAX_DEPTH {
            return err("flexbuffers: nesting too deep");
        }
        self.spend(n)?;
        let buf = self.buf;
        if vt == FBT_VECTOR_INT {
            let mut out = Vec::with_capacity(n);
            for i in 0..n {
                out.push(read_int(buf, t + i * w, w)?);
            }
            return Ok(Value::IntVector(out));
        }
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            let p = t + i * w;
            out.push(match vt {
                FBT_VECTOR_FLOAT => Value::Float(read_float(buf, p, w)?),
                FBT_VECTOR_BOOL => Value::Bool(read_uint(buf, p, w)? != 0),
                FBT_VECTOR_UINT2 => Value::UInt(read_uint(buf, p, w)?),
                FBT_VECTOR_KEY => {
                    let k = deref(buf, p, w)?;
                    Value::String(self.key_at(k)?)
                }
                _ => {
                    // deprecated string vector: strings sized with width w
                    let s = deref(buf, p, w)?;
                    let len = sized(buf, s, w, 1)?;
                    self.spend(len)?;
                    Value::String(
                        utf8_string(buf[s..s + len].to_vec())
                            .ok_or_else(|| crate::CodecError::new("flexbuffers: string is not UTF-8"))?,
                    )
                }
            });
        }
        Ok(Value::Vector(out))
    }
}

/// Decode a complete FlexBuffers buffer.
pub fn decode(buf: &[u8]) -> Result<Value> {
    let (pos, pw, packed) = root(buf)?;
    let mut r = Reader { buf, budget: buf.len().saturating_mul(64).saturating_add(4096) };
    r.value(pos, pw, packed, 0)
}

// ---------------------------------------------------------------------------
// Locating payload bytes

/// One navigation step for [`locate`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathElem {
    Index(usize),
    Key(String),
}

/// Navigate from the root through untyped vectors (`Index`) and maps (`Key`)
/// and return the absolute byte range of the target's payload: the raw bytes
/// of a `VT_BLOB` or `VT_STRING` (excluding the size prefix and the string's
/// NUL terminator). Other target types are an error.
pub fn locate(buf: &[u8], path: &[PathElem]) -> Result<Range<usize>> {
    let (mut pos, mut pw, mut packed) = root(buf)?;
    for step in path {
        let ty = packed >> 2;
        let w = 1usize << (packed & 3);
        let t = match ty {
            FBT_VECTOR | FBT_MAP => deref(buf, pos, pw)?,
            _ => return err("flexbuffers: locate step into a non-container"),
        };
        let n = sized(buf, t, w, w + 1)?;
        let idx = match (step, ty) {
            (PathElem::Index(i), FBT_VECTOR) => *i,
            (PathElem::Key(k), FBT_MAP) => {
                if t < 3 * w {
                    return err("flexbuffers: map header out of bounds");
                }
                let kt = deref(buf, t - 3 * w, w)?;
                let kw = read_uint(buf, t - 2 * w, w)?;
                if !matches!(kw, 1 | 2 | 4 | 8) {
                    return err("flexbuffers: invalid map key width");
                }
                let kw = kw as usize;
                check_region(buf, kt, n as u64, kw)?;
                let mut found = None;
                for i in 0..n {
                    let kp = deref(buf, kt + i * kw, kw)?;
                    let rest = &buf[kp..];
                    let end = rest.iter().position(|&b| b == 0);
                    if let Some(e) = end {
                        if &rest[..e] == k.as_bytes() {
                            found = Some(i);
                            break;
                        }
                    }
                }
                found.ok_or_else(|| crate::CodecError::new(format!("flexbuffers: key {k:?} not found")))?
            }
            _ => return err("flexbuffers: locate path does not match container type"),
        };
        if idx >= n {
            return err("flexbuffers: index out of range");
        }
        pos = t + idx * w;
        pw = w;
        packed = buf[t + n * w + idx];
    }
    match packed >> 2 {
        FBT_STRING | FBT_BLOB => {
            let t = deref(buf, pos, pw)?;
            let n = sized(buf, t, 1 << (packed & 3), 1)?;
            Ok(t..t + n)
        }
        _ => err("flexbuffers: located value is not a blob or string"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bigint::parse_decimal;
    use proptest::prelude::*;

    fn s(x: &str) -> Value {
        Value::String(x.into())
    }

    #[test]
    fn species_extension_bytes() {
        let b = encode_extension(&Value::species()).unwrap();
        assert_eq!(b, [8, b'#', b's', b'p', b'e', b'c', b'i', b'e', b's', 8, 0x30, 1]);
        assert_eq!(encode(&Value::species()).unwrap(), b);
        assert_eq!(decode(&b).unwrap(), Value::species());
        assert_eq!(decode_extension(&b).unwrap(), Value::species());
    }

    #[test]
    fn bigint_extensions() {
        let cases = [
            ("0", vec![b'x', 0]),
            ("1", vec![b'x', 1]),
            ("255", vec![b'x', 0xff]),
            ("256", vec![b'x', 1, 0]),
            ("-1", vec![b'~', 0]),
            ("-256", vec![b'~', 0xff]),
            ("-257", vec![b'~', 1, 0]),
        ];
        for (d, elems) in cases {
            let v = Value::BigInt(parse_decimal(d).unwrap());
            let b = encode_extension(&v).unwrap();
            assert_eq!(b[0] as usize, elems.len());
            assert_eq!(&b[1..1 + elems.len()], &elems[..]);
            assert_eq!(decode_extension(&b).unwrap(), v);
            assert_eq!(decode(&b).unwrap(), v);
        }
        let big = Value::BigInt(parse_decimal("-340282366920938463463374607431768211457").unwrap());
        assert_eq!(decode(&encode(&big).unwrap()).unwrap(), big);
    }

    #[test]
    fn extension_widening() {
        // 255 payload bytes + prefix = 256 elements -> two-byte elements
        let name = "a".repeat(255);
        let b = encode_extension(&Value::GlobalSymbol(name.clone())).unwrap();
        assert_eq!(b[b.len() - 2] & 3, 1);
        assert_eq!(&b[0..2], &[0, 1]); // size 256 as u16
        assert_eq!(decode_extension(&b).unwrap(), Value::GlobalSymbol(name));
        let name = "a".repeat(254);
        let b = encode_extension(&Value::GlobalSymbol(name.clone())).unwrap();
        assert_eq!(b[b.len() - 2] & 3, 0);
        let name = "b".repeat(65535);
        let b = encode_extension(&Value::GlobalSymbol(name.clone())).unwrap();
        assert_eq!(b[b.len() - 2] & 3, 2);
        assert_eq!(decode(&b).unwrap(), Value::GlobalSymbol(name));
    }

    #[test]
    fn extension_rejections() {
        // noncanonical magnitudes
        for payload in [vec![b'x'], vec![b'x', 0, 1], vec![b'~', 0, 0], vec![b'#', b'f', b'o', b'o'],
                        vec![b'$', 0xff], vec![b'?', 1]] {
            assert!(encode_extension_bytes(payload[0], &payload[1..]).is_err());
            let mut raw = vec![payload.len() as u8];
            raw.extend_from_slice(&payload);
            raw.extend_from_slice(&[payload.len() as u8 + 0, 0x30, 1]);
            assert!(decode_extension(&raw).is_err(), "{payload:?}");
            assert!(decode(&raw).is_err());
        }
        // empty VT_VECTOR_UINT
        assert!(decode(&[0, 0, 0x30, 1]).is_err());
        // element > 255 with 2-byte width
        let raw = [2, 0, b'x', 0, 0, 1, 6, 0x31, 1];
        assert!(decode_extension(&raw).is_err());
        // same value 1 with 2-byte width is accepted (any valid width)
        let raw = [2, 0, b'x', 0, 1, 0, 4, 0x31, 1];
        assert_eq!(decode_extension(&raw).unwrap(), Value::BigInt(1.into()));
    }

    #[test]
    fn commitflow_tuple() {
        let blob: Vec<u8> = (0..300u32).map(|i| i as u8).collect();
        let entry = Value::Map(vec![
            ("viewpoint".into(), s("0")),
            ("sha384".into(), s(&"ab".repeat(48))),
            ("blob".into(), Value::Blob(blob.clone())),
            ("context".into(), Value::Map(vec![("k".into(), s("v"))])),
            ("edges".into(), Value::Vector(vec![])),
        ]);
        let tuple = Value::Vector(vec![
            Value::species(),
            s("sync.memoremix.com"),
            s("sts/0/databasin/commitflow"),
            Value::Map(vec![("markers".into(), Value::Map(vec![("flow-a".into(), entry)]))]),
        ]);
        let bytes = encode(&tuple).unwrap();
        let back = decode(&bytes).unwrap();
        let v = back.as_vec().unwrap();
        assert!(v[0].is_well_known_symbol("species"));
        let e = v[3].get("markers").unwrap().get("flow-a").unwrap();
        assert_eq!(e.get("blob").unwrap().as_bytes().unwrap(), &blob[..]);
        assert_eq!(e.get("context").unwrap().get("k").unwrap().as_str(), Some("v"));
        // map keys come back sorted
        let keys: Vec<&str> = e.as_map().unwrap().iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(keys, ["blob", "context", "edges", "sha384", "viewpoint"]);
        let r = locate(&bytes, &[PathElem::Index(3), PathElem::Key("markers".into()),
                                 PathElem::Key("flow-a".into()), PathElem::Key("blob".into())]).unwrap();
        assert_eq!(&bytes[r], &blob[..]);
        let r = locate(&bytes, &[PathElem::Index(1)]).unwrap();
        assert_eq!(&bytes[r], b"sync.memoremix.com");
        assert!(locate(&bytes, &[PathElem::Index(0)]).is_err());
        assert!(locate(&bytes, &[PathElem::Index(9)]).is_err());
        assert!(locate(&bytes, &[PathElem::Key("x".into())]).is_err());
    }

    #[test]
    fn bulk_manifest_like() {
        let v = Value::Vector(vec![
            Value::species(),
            s("sync.memoremix.com"),
            s("sts/0/databasin/bulkmanifest"),
            Value::Map(vec![
                ("lowest".into(), Value::BigInt(42.into())),
                ("coordinate".into(), Value::BigInt(65536.into())),
                ("ends".into(), Value::IntVector(vec![488, 1360, 1976])),
            ]),
        ]);
        let b = encode(&v).unwrap();
        let mut sorted = v.clone();
        if let Value::Vector(items) = &mut sorted {
            if let Value::Map(m) = &mut items[3] {
                m.sort_by(|a, b| a.0.cmp(&b.0));
            }
        }
        assert_eq!(decode(&b).unwrap(), sorted);
    }

    #[test]
    fn scalars_layout() {
        assert_eq!(encode(&Value::Null).unwrap(), [0, 0, 1]);
        assert_eq!(encode(&Value::Bool(true)).unwrap(), [1, 26 << 2, 1]);
        assert_eq!(encode(&Value::Int(-1)).unwrap(), [0xff, 4, 1]);
        assert_eq!(encode(&Value::Int(300)).unwrap(), [44, 1, 5, 2]);
        assert_eq!(encode(&Value::UInt(255)).unwrap(), [255, 8, 1]);
        assert_eq!(encode(&Value::Float(1.5)).unwrap(), [0, 0, 0xc0, 0x3f, 14, 4]);
        // string "ab": len 2, 'a','b',0, offset 3, type STRING w8, width 1
        assert_eq!(encode(&s("ab")).unwrap(), [2, b'a', b'b', 0, 3, 20, 1]);
        // map {"a": 1}: key "a\0", keys vec [1, off], values: keys off, keys width, size, 1, type
        let m = encode(&Value::Map(vec![("a".into(), Value::Int(1))])).unwrap();
        assert_eq!(m, [b'a', 0, 1, 3, 1, 1, 1, 1, 4, 2, 36, 1]);
        assert_eq!(decode(&m).unwrap(), Value::Map(vec![("a".into(), Value::Int(1))]));
    }

    #[test]
    fn decode_other_layouts() {
        // indirect int: value 7 stored as 8-byte scalar, root INDIRECT_INT w64
        let mut b = 7u64.to_le_bytes().to_vec();
        b.extend_from_slice(&[8, FBT_INDIRECT_INT << 2 | 3, 1]);
        assert_eq!(decode(&b).unwrap(), Value::Int(7));
        // fixed VECTOR_INT2 with 2-byte elements [-2, 3]
        let b = [0xfe, 0xff, 3, 0, 4, FBT_VECTOR_INT2 << 2 | 1, 1];
        assert_eq!(decode(&b).unwrap(), Value::IntVector(vec![-2, 3]));
        // typed VECTOR_BOOL
        let b = [2, 1, 0, 2, FBT_VECTOR_BOOL << 2, 1];
        assert_eq!(decode(&b).unwrap(), Value::Vector(vec![Value::Bool(true), Value::Bool(false)]));
        // untyped vector stored with 4-byte root width
        let v = Value::Vector(vec![Value::Int(1), s("x")]);
        let e = encode(&v).unwrap();
        assert_eq!(decode(&e).unwrap(), v);
    }

    #[test]
    fn decoder_rejects() {
        for bad in [&[][..], &[1], &[0, 1], &[0, 0, 3], &[5, 20, 1], &[0xff, 20, 1], &[1, 100 << 2, 1]] {
            assert!(decode(bad).is_err(), "{bad:?}");
        }
        // self-referential vector (offset 0) recursion is bounded
        let b = [1, 0, 0x28, 1];
        assert!(decode(&b).is_err());
        assert!(encode(&Value::Map(vec![("a".into(), Value::Null), ("a".into(), Value::Null)])).is_err());
        assert!(encode(&Value::Map(vec![("a\0".into(), Value::Null)])).is_err());
        assert!(encode(&Value::WellKnownSymbol("Symbol.species".into())).is_err());
    }

    #[test]
    fn shared_blowup_bounded() {
        // A chain of vectors each containing the previous one twice: 2^40 leaves.
        let mut buf = vec![0u8, b'a', 0]; // placeholder string "a" at 1 (len 1 at 0)
        buf[0] = 1;
        let mut prev_loc = 1usize;
        let mut prev_ty = FBT_STRING << 2;
        for _ in 0..40 {
            while buf.len() % 8 != 0 {
                buf.push(0);
            }
            buf.extend_from_slice(&2u64.to_le_bytes());
            let loc = buf.len();
            for i in 0..2 {
                let off = (loc + i * 8 - prev_loc) as u64;
                buf.extend_from_slice(&off.to_le_bytes());
            }
            buf.push(prev_ty);
            buf.push(prev_ty);
            prev_loc = loc;
            prev_ty = FBT_VECTOR << 2 | 3;
        }
        while buf.len() % 8 != 0 {
            buf.push(0);
        }
        let off = (buf.len() - prev_loc) as u64;
        buf.extend_from_slice(&off.to_le_bytes());
        buf.push(prev_ty);
        buf.push(8);
        assert!(decode(&buf).is_err());
    }

    fn arb_value() -> impl Strategy<Value = Value> {
        let leaf = prop_oneof![
            Just(Value::Null),
            any::<bool>().prop_map(Value::Bool),
            any::<i64>().prop_map(Value::Int),
            any::<u64>().prop_map(Value::UInt),
            any::<f64>().prop_filter("nan", |f| !f.is_nan()).prop_map(Value::Float),
            ".{0,20}".prop_map(Value::String),
            proptest::collection::vec(any::<u8>(), 0..300).prop_map(Value::Blob),
            any::<i128>().prop_map(|i| Value::BigInt(i.into())),
            "[^\u{0}]{0,10}".prop_map(Value::GlobalSymbol),
            proptest::sample::select(WELL_KNOWN_SYMBOLS.to_vec()).prop_map(|s| Value::WellKnownSymbol(s.into())),
            proptest::collection::vec(any::<i64>(), 0..10).prop_map(Value::IntVector),
        ];
        leaf.prop_recursive(4, 64, 8, |inner| {
            prop_oneof![
                proptest::collection::vec(inner.clone(), 0..8).prop_map(Value::Vector),
                proptest::collection::btree_map("[a-z]{0,6}", inner, 0..6)
                    .prop_map(|m| Value::Map(m.into_iter().collect())),
            ]
        })
    }

    proptest! {
        #[test]
        fn roundtrip(v in arb_value()) {
            let b = encode(&v).unwrap();
            prop_assert_eq!(decode(&b).unwrap(), v);
        }

        /// The general decoder is too large for Kani (see KANI.md); fuzz it
        /// for panics on arbitrary bytes and on mutated valid encodings.
        #[test]
        fn decode_never_panics(bytes in proptest::collection::vec(any::<u8>(), 0..64)) {
            let _ = decode(&bytes);
            let _ = decode_extension(&bytes);
        }

        #[test]
        fn decode_mutated_never_panics(v in arb_value(), at in any::<usize>(), byte in any::<u8>()) {
            let mut b = encode(&v).unwrap();
            let i = at % b.len();
            b[i] = byte;
            let _ = decode(&b);
            b.truncate(i);
            let _ = decode(&b);
        }

        #[test]
        fn ext_direct_matches_builder(n in any::<i128>(), name in "[^\u{0}]{0,300}") {
            for v in [Value::BigInt(n.into()), Value::GlobalSymbol(name.clone())] {
                prop_assert_eq!(encode_extension(&v).unwrap(), encode(&v).unwrap());
            }
        }

        #[test]
        fn no_panic(buf in proptest::collection::vec(any::<u8>(), 0..64)) {
            let _ = decode(&buf);
            let _ = decode_extension(&buf);
            let _ = locate(&buf, &[PathElem::Index(0)]);
        }
    }
}

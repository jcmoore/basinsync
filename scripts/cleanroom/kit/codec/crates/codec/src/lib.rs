//! basinsync codecs: Lexidecimal, Diffuse16, EnvBuf, FlexBuffers (with the
//! basinsync `VT_VECTOR_UINT` extensions), JSON0, CRC32C, SHA-384 rendering,
//! identifiers and S3 object keys.
//!
//! Each codec is structured as a small, allocation-light core over digit or
//! byte slices (verified with Kani, see `kani_proofs.rs`) wrapped by the
//! `BigInt`-based public API.

pub use num_bigint::BigInt;

pub mod base64;
pub mod bigint;
pub mod crc32c;
pub mod diffuse16;
pub mod envbuf;
pub mod error;
pub mod flexbuffers;
pub mod ids;
pub mod json0;
pub mod keys;
pub mod lexidecimal;
pub mod sha384;

#[cfg(kani)]
mod kani_proofs;

pub use error::{CodecError, Result};

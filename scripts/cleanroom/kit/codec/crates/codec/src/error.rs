use std::fmt;

/// A codec rejection: invalid, noncanonical, reserved, or unsupported input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodecError(pub String);

impl CodecError {
    pub fn new(msg: impl Into<String>) -> Self {
        CodecError(msg.into())
    }
}

impl fmt::Display for CodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for CodecError {}

pub type Result<T> = std::result::Result<T, CodecError>;

/// Shorthand for building an `Err(CodecError)`.
pub(crate) fn err<T>(msg: impl Into<String>) -> Result<T> {
    Err(CodecError(msg.into()))
}

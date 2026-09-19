//! Library error types for discovery, decode, resize, and encode.

use thiserror::Error;

/// Recoverable pictures4096 failures.
#[derive(Debug, Error)]
pub enum Error {
    /// Filesystem or walk error.
    #[error("io error: {0}")]
    Io(String),

    /// Caller provided an unusable path or option.
    #[error("invalid input: {0}")]
    InvalidInput(String),

    /// Image could not be decoded or identified.
    #[error("decode failed: {0}")]
    Decode(String),

    /// Resize backend failed.
    #[error("resize failed: {0}")]
    Resize(String),

    /// Encoded bytes could not be written or validated.
    #[error("encode failed: {0}")]
    Encode(String),

    /// EXIF copy via fast-exif-rs failed.
    #[error("exif copy failed: {0}")]
    Exif(String),
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn displays_variants() {
        let error = Error::InvalidInput("bad".to_owned());
        assert!(error.to_string().contains("bad"));
    }
}

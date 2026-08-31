use thiserror::Error;

#[derive(Debug, Error)]
pub enum GtpError {
    #[error("buffer truncated")]
    Truncated,

    #[error("missing mandatory IE: type {0}")]
    MissingIe(u8),

    #[error("missing TEID in header")]
    MissingTeid,

    #[error("unsupported GTP version: {0}")]
    UnsupportedVersion(u8),

    #[error("invalid IE value for type {0}")]
    InvalidIeValue(u8),

    #[error("expected TEID {expected:?}, got {actual:?}")]
    InvalidTeid { expected: u32, actual: Option<u32> },

    #[error("invalid RAT type value: {0}")]
    InvalidRatType(u8),

    #[error("invalid PDN type value: {0}")]
    InvalidPdnType(u8),

    #[error("invalid selection mode value: {0}")]
    InvalidSelectionMode(u8),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

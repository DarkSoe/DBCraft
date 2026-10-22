use thiserror::Error;

/// Errors originating from pure core validation.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CoreError {
    #[error("{0}")]
    Message(String),
    #[error("invalid connection profile: {0}")]
    InvalidProfile(String),
}

pub type Result<T> = std::result::Result<T, CoreError>;

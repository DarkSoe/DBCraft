use thiserror::Error;

#[derive(Debug, Error)]
pub enum DriverError {
    #[error("{0}")]
    Message(String),
    #[error("invalid profile: {0}")]
    InvalidProfile(String),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("url error: {0}")]
    Url(String),
}

pub type Result<T> = std::result::Result<T, DriverError>;

impl From<dbcraft_core::CoreError> for DriverError {
    fn from(value: dbcraft_core::CoreError) -> Self {
        Self::InvalidProfile(value.to_string())
    }
}

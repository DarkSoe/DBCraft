use thiserror::Error;

#[derive(Debug, Error)]
pub enum EngineError {
    #[error("{0}")]
    Message(String),
    #[error("unknown command: {0}")]
    UnknownCommand(String),
    #[error("not connected")]
    NotConnected,
    #[error("profile not found: {0}")]
    ProfileNotFound(String),
    #[error("driver: {0}")]
    Driver(#[from] dbcraft_driver::DriverError),
    #[error("core: {0}")]
    Core(#[from] dbcraft_core::CoreError),
    #[error("settings: {0}")]
    Settings(String),
    #[error("keyring: {0}")]
    Keyring(String),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, EngineError>;

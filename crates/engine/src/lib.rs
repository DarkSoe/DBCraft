#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

//! Session façade: commands, jobs, settings and keyring secrets.

mod commands;
mod error;
mod jobs;
mod secrets;
mod session;
mod settings;

pub use commands::{CommandId, CommandParams, CommandResult};
pub use dbcraft_driver::ColumnDesignInfo;
pub use error::{EngineError, Result};
pub use jobs::{JobId, JobKind, JobStatus};
pub use session::Session;
pub use settings::{AppSettings, UiPrefs};

/// Preview SQL that `users.save` would run for a draft.
pub fn preview_user_save_sql(draft: &dbcraft_core::UserDraft) -> String {
    dbcraft_driver::preview_user_save_sql(draft)
}

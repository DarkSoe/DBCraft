#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

//! Pure data types for DBCraft. No I/O.

mod error;
mod object;
mod profile;
mod query;
mod schema;
mod user;

pub use error::{CoreError, Result};
pub use object::ObjectInfo;
pub use profile::{ConnectionKind, ConnectionProfile, SslMode};
pub use query::{ColumnInfo, QueryResult, Row, SqlValue};
pub use schema::{SchemaKind, SchemaNode};
pub use user::{DbGrant, UserDraft, UserInfo, db_privilege_names, server_privilege_names};

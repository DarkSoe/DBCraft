#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

//! Database drivers behind a single connection enum.

mod connect;
mod error;
mod query;
mod schema;
mod users;
mod value;

pub use connect::{connect, redacted_url, test_connection};
pub use error::{DriverError, Result};
pub use query::execute_sql;
pub use schema::{
    ColumnDesignInfo, drop_object, list_column_design, list_databases, list_objects,
    list_primary_keys, list_routines, list_table_columns, list_tables, object_definition,
    preview_table,
};
pub use users::{
    drop_user, list_users, load_user_detail, preview_user_save_sql, save_user,
};

use dbcraft_core::ConnectionKind;
use sqlx::{MySqlPool, PgPool, SqlitePool};

/// An open database connection (one pool per profile session).
#[derive(Debug, Clone)]
pub enum DbConnection {
    Mysql(MySqlPool),
    Postgres(PgPool),
    Sqlite(SqlitePool),
}

impl DbConnection {
    pub fn kind(&self) -> ConnectionKind {
        match self {
            Self::Mysql(_) => ConnectionKind::Mysql,
            Self::Postgres(_) => ConnectionKind::Postgres,
            Self::Sqlite(_) => ConnectionKind::Sqlite,
        }
    }

    pub async fn close(self) {
        match self {
            Self::Mysql(p) => p.close().await,
            Self::Postgres(p) => p.close().await,
            Self::Sqlite(p) => p.close().await,
        }
    }
}

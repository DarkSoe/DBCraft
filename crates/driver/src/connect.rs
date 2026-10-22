use dbcraft_core::{ConnectionKind, ConnectionProfile, SslMode};
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use sqlx::mysql::MySqlConnectOptions;
use sqlx::postgres::PgConnectOptions;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::{ConnectOptions, MySqlPool, PgPool, SqlitePool};
use std::str::FromStr;
use std::time::Duration;

use crate::{DbConnection, DriverError, Result};

fn encode(s: &str) -> String {
    utf8_percent_encode(s, NON_ALPHANUMERIC).to_string()
}

pub fn mysql_options(
    profile: &ConnectionProfile,
    password: &str,
) -> Result<MySqlConnectOptions> {
    profile.validate()?;
    let mut opts = MySqlConnectOptions::new()
        .host(&profile.host)
        .port(profile.port)
        .username(&profile.username)
        .password(password);
    if !profile.database.is_empty() {
        opts = opts.database(&profile.database);
    }
    match profile.ssl {
        SslMode::Disable => {
            opts = opts.ssl_mode(sqlx::mysql::MySqlSslMode::Disabled);
        }
        SslMode::Prefer => {
            opts = opts.ssl_mode(sqlx::mysql::MySqlSslMode::Preferred);
        }
        SslMode::Require => {
            opts = opts.ssl_mode(sqlx::mysql::MySqlSslMode::Required);
        }
    }
    opts = opts.log_statements(log::LevelFilter::Debug);
    Ok(opts)
}

pub fn postgres_options(
    profile: &ConnectionProfile,
    password: &str,
) -> Result<PgConnectOptions> {
    profile.validate()?;
    let mut opts = PgConnectOptions::new()
        .host(&profile.host)
        .port(profile.port)
        .username(&profile.username)
        .password(password);
    if !profile.database.is_empty() {
        opts = opts.database(&profile.database);
    } else {
        opts = opts.database("postgres");
    }
    match profile.ssl {
        SslMode::Disable => {
            opts = opts.ssl_mode(sqlx::postgres::PgSslMode::Disable);
        }
        SslMode::Prefer => {
            opts = opts.ssl_mode(sqlx::postgres::PgSslMode::Prefer);
        }
        SslMode::Require => {
            opts = opts.ssl_mode(sqlx::postgres::PgSslMode::Require);
        }
    }
    opts = opts.log_statements(log::LevelFilter::Debug);
    Ok(opts)
}

pub fn sqlite_options(profile: &ConnectionProfile) -> Result<SqliteConnectOptions> {
    profile.validate()?;
    let opts = SqliteConnectOptions::from_str(&profile.file_path)
        .map_err(|e| DriverError::Message(e.to_string()))?
        .create_if_missing(true)
        .log_statements(log::LevelFilter::Debug);
    Ok(opts)
}

/// Build a diagnostic URL (password redacted) for logs.
pub fn redacted_url(profile: &ConnectionProfile) -> String {
    match profile.kind {
        ConnectionKind::Sqlite => format!("sqlite:{}", profile.file_path),
        ConnectionKind::Mysql | ConnectionKind::Mariadb => format!(
            "mysql://{user}@{host}:{port}/{db}",
            user = encode(&profile.username),
            host = profile.host,
            port = profile.port,
            db = profile.database
        ),
        ConnectionKind::Postgres => format!(
            "postgres://{user}@{host}:{port}/{db}",
            user = encode(&profile.username),
            host = profile.host,
            port = profile.port,
            db = if profile.database.is_empty() {
                "postgres"
            } else {
                &profile.database
            }
        ),
    }
}

pub async fn connect(profile: &ConnectionProfile, password: &str) -> Result<DbConnection> {
    profile.validate()?;
    let pool = match profile.kind {
        ConnectionKind::Mysql | ConnectionKind::Mariadb => {
            let opts = mysql_options(profile, password)?;
            let pool = MySqlPool::connect_with(opts)
                .await
                .map_err(DriverError::from)?;
            DbConnection::Mysql(pool)
        }
        ConnectionKind::Postgres => {
            let opts = postgres_options(profile, password)?;
            let pool = PgPool::connect_with(opts)
                .await
                .map_err(DriverError::from)?;
            DbConnection::Postgres(pool)
        }
        ConnectionKind::Sqlite => {
            let opts = sqlite_options(profile)?;
            let pool = SqlitePool::connect_with(opts)
                .await
                .map_err(DriverError::from)?;
            DbConnection::Sqlite(pool)
        }
    };
    Ok(pool)
}

pub async fn test_connection(profile: &ConnectionProfile, password: &str) -> Result<()> {
    let conn = connect(profile, password).await?;
    // Soft timeout: if connect succeeded, run a trivial ping.
    let ping = async {
        match &conn {
            DbConnection::Mysql(p) => {
                sqlx::query("SELECT 1").execute(p).await?;
            }
            DbConnection::Postgres(p) => {
                sqlx::query("SELECT 1").execute(p).await?;
            }
            DbConnection::Sqlite(p) => {
                sqlx::query("SELECT 1").execute(p).await?;
            }
        }
        Ok::<(), DriverError>(())
    };
    match tokio::time::timeout(Duration::from_secs(15), ping).await {
        Ok(r) => {
            conn.close().await;
            r
        }
        Err(_) => {
            conn.close().await;
            Err(DriverError::Message("connection timed out".into()))
        }
    }
}

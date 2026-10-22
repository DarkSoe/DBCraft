use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{CoreError, Result};

/// Supported database engines for the MVP.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ConnectionKind {
    #[default]
    Mysql,
    Mariadb,
    Sqlite,
    Postgres,
}

impl ConnectionKind {
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Mysql => "MySQL",
            Self::Mariadb => "MariaDB",
            Self::Sqlite => "SQLite",
            Self::Postgres => "PostgreSQL",
        }
    }

    pub fn default_port(self) -> u16 {
        match self {
            Self::Mysql | Self::Mariadb => 3306,
            Self::Postgres => 5432,
            Self::Sqlite => 0,
        }
    }

    /// MySQL and MariaDB share the MySQL wire protocol.
    pub fn uses_mysql_protocol(self) -> bool {
        matches!(self, Self::Mysql | Self::Mariadb)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SslMode {
    #[default]
    Disable,
    Prefer,
    Require,
}

/// Saved connection profile (password lives in the OS keyring, not here).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ConnectionProfile {
    pub id: Uuid,
    pub name: String,
    pub kind: ConnectionKind,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub database: String,
    /// Absolute path for SQLite; ignored for other kinds.
    pub file_path: String,
    pub ssl: SslMode,
}

impl Default for ConnectionProfile {
    fn default() -> Self {
        Self {
            id: Uuid::nil(),
            name: String::new(),
            kind: ConnectionKind::Mysql,
            host: "127.0.0.1".into(),
            port: ConnectionKind::Mysql.default_port(),
            username: String::new(),
            database: String::new(),
            file_path: String::new(),
            ssl: SslMode::Disable,
        }
    }
}

impl ConnectionProfile {
    pub fn new(name: impl Into<String>, kind: ConnectionKind) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            kind,
            port: kind.default_port(),
            ..Self::default()
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.name.trim().is_empty() {
            return Err(CoreError::InvalidProfile("name is required".into()));
        }
        match self.kind {
            ConnectionKind::Sqlite => {
                if self.file_path.trim().is_empty() {
                    return Err(CoreError::InvalidProfile(
                        "SQLite file path is required".into(),
                    ));
                }
            }
            ConnectionKind::Mysql | ConnectionKind::Mariadb | ConnectionKind::Postgres => {
                if self.host.trim().is_empty() {
                    return Err(CoreError::InvalidProfile("host is required".into()));
                }
                if self.port == 0 {
                    return Err(CoreError::InvalidProfile("port must be non-zero".into()));
                }
            }
        }
        Ok(())
    }

    pub fn keyring_user(&self) -> String {
        self.id.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqlite_requires_path() {
        let mut p = ConnectionProfile::new("local", ConnectionKind::Sqlite);
        assert!(p.validate().is_err());
        p.file_path = ":memory:".into();
        assert!(p.validate().is_ok());
    }

    #[test]
    fn mysql_requires_host() {
        let mut p = ConnectionProfile::new("prod", ConnectionKind::Mysql);
        p.host.clear();
        assert!(p.validate().is_err());
        p.host = "localhost".into();
        assert!(p.validate().is_ok());
    }
}

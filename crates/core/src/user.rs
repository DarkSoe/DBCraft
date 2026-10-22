use std::collections::HashSet;

use serde::{Deserialize, Serialize};

/// Row in the server-wide user list (Navicat Objects › Benutzer).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserInfo {
    pub name: String,
    pub host: String,
    pub plugin: String,
    pub ssl_type: String,
    pub max_questions: u32,
    pub max_updates: u32,
    pub max_connections: u32,
    pub max_user_connections: u32,
    pub is_super: bool,
}

impl UserInfo {
    pub fn account(&self) -> String {
        format!("{}@{}", self.name, self.host)
    }
}

/// Database-level privilege row for a user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DbGrant {
    pub database: String,
    pub privileges: HashSet<String>,
}

/// Editable user draft used by the UI and `users.save`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserDraft {
    /// False for "Neuer Benutzer".
    pub is_existing: bool,
    pub name: String,
    pub host: String,
    pub plugin: String,
    pub password: String,
    pub password_repeat: String,
    /// Empty = do not change password on save (existing users).
    pub password_expiration: String,
    pub server_privs: HashSet<String>,
    pub db_grants: Vec<DbGrant>,
    /// Original account when editing existing user.
    pub original_name: String,
    pub original_host: String,
}

impl UserDraft {
    pub fn new_blank() -> Self {
        Self {
            is_existing: false,
            name: String::new(),
            host: "%".into(),
            plugin: "mysql_native_password".into(),
            password: String::new(),
            password_repeat: String::new(),
            password_expiration: "DEFAULT".into(),
            server_privs: HashSet::new(),
            db_grants: Vec::new(),
            original_name: String::new(),
            original_host: String::new(),
        }
    }

    pub fn account(&self) -> String {
        format!("{}@{}", self.name, self.host)
    }
}

/// Global / server privilege names shown in the editor (GRANT syntax).
pub fn server_privilege_names() -> &'static [&'static str] {
    &[
        "ALTER",
        "ALTER ROUTINE",
        "CREATE",
        "CREATE ROUTINE",
        "CREATE TEMPORARY TABLES",
        "CREATE USER",
        "CREATE VIEW",
        "DELETE",
        "DROP",
        "EVENT",
        "EXECUTE",
        "FILE",
        "GRANT OPTION",
        "INDEX",
        "INSERT",
        "LOCK TABLES",
        "PROCESS",
        "REFERENCES",
        "RELOAD",
        "REPLICATION CLIENT",
        "REPLICATION SLAVE",
        "SELECT",
        "SHOW DATABASES",
        "SHOW VIEW",
        "SHUTDOWN",
        "SUPER",
        "TRIGGER",
        "UPDATE",
    ]
}

/// Database-level privilege names for the Rechte grid.
pub fn db_privilege_names() -> &'static [&'static str] {
    &[
        "SELECT",
        "INSERT",
        "UPDATE",
        "DELETE",
        "CREATE",
        "DROP",
        "REFERENCES",
        "INDEX",
        "ALTER",
        "CREATE TEMPORARY TABLES",
        "LOCK TABLES",
        "CREATE VIEW",
        "SHOW VIEW",
        "CREATE ROUTINE",
        "ALTER ROUTINE",
        "EXECUTE",
        "EVENT",
        "TRIGGER",
        "GRANT OPTION",
    ]
}

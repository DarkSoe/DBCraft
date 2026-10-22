//! MySQL/MariaDB user management helpers.

use std::collections::HashSet;

use dbcraft_core::{DbGrant, UserDraft, UserInfo};
use sqlx::Row;

use crate::{DbConnection, DriverError, Result, execute_sql};

pub async fn list_users(conn: &DbConnection) -> Result<Vec<UserInfo>> {
    match conn {
        DbConnection::Mysql(pool) => {
            // CAST: mysql.user char columns often arrive as BINARY via sqlx.
            let rows = sqlx::query(
                "SELECT CONVERT(User USING utf8mb4), \
                        CONVERT(Host USING utf8mb4), \
                        CONVERT(IFNULL(plugin, '') USING utf8mb4), \
                        CONVERT(IFNULL(ssl_type, '') USING utf8mb4), \
                        IFNULL(max_questions, 0), \
                        IFNULL(max_updates, 0), \
                        IFNULL(max_connections, 0), \
                        IFNULL(max_user_connections, 0), \
                        CONVERT(IFNULL(Super_priv, 'N') USING utf8mb4) \
                 FROM mysql.user \
                 ORDER BY User, Host",
            )
            .fetch_all(pool)
            .await
            .map_err(|e| {
                DriverError::Message(format!(
                    "cannot list users (need SELECT on mysql.user): {e}"
                ))
            })?;
            let mut out = Vec::with_capacity(rows.len());
            for row in rows {
                let name = mysql_text(&row, 0)?;
                let host = mysql_text(&row, 1)?;
                let plugin = mysql_text(&row, 2).unwrap_or_default();
                let ssl_type = mysql_text(&row, 3).unwrap_or_default();
                let max_questions: u32 = row_u32(&row, 4);
                let max_updates: u32 = row_u32(&row, 5);
                let max_connections: u32 = row_u32(&row, 6);
                let max_user_connections: u32 = row_u32(&row, 7);
                let super_raw = mysql_text(&row, 8).unwrap_or_else(|_| "N".into());
                out.push(UserInfo {
                    name,
                    host,
                    plugin,
                    ssl_type,
                    max_questions,
                    max_updates,
                    max_connections,
                    max_user_connections,
                    is_super: super_raw.eq_ignore_ascii_case("Y"),
                });
            }
            Ok(out)
        }
        DbConnection::Postgres(_) | DbConnection::Sqlite(_) => Err(DriverError::Message(
            "user management is only supported for MySQL/MariaDB".into(),
        )),
    }
}

pub async fn load_user_detail(conn: &DbConnection, user: &str, host: &str) -> Result<UserDraft> {
    match conn {
        DbConnection::Mysql(pool) => {
            let row = sqlx::query(
                "SELECT CONVERT(User USING utf8mb4), CONVERT(Host USING utf8mb4), \
                        CONVERT(IFNULL(plugin, '') USING utf8mb4), \
                        CONVERT(IFNULL(Select_priv,'N') USING utf8mb4), \
                        CONVERT(IFNULL(Insert_priv,'N') USING utf8mb4), \
                        CONVERT(IFNULL(Update_priv,'N') USING utf8mb4), \
                        CONVERT(IFNULL(Delete_priv,'N') USING utf8mb4), \
                        CONVERT(IFNULL(Create_priv,'N') USING utf8mb4), \
                        CONVERT(IFNULL(Drop_priv,'N') USING utf8mb4), \
                        CONVERT(IFNULL(Reload_priv,'N') USING utf8mb4), \
                        CONVERT(IFNULL(Shutdown_priv,'N') USING utf8mb4), \
                        CONVERT(IFNULL(Process_priv,'N') USING utf8mb4), \
                        CONVERT(IFNULL(File_priv,'N') USING utf8mb4), \
                        CONVERT(IFNULL(Grant_priv,'N') USING utf8mb4), \
                        CONVERT(IFNULL(References_priv,'N') USING utf8mb4), \
                        CONVERT(IFNULL(Index_priv,'N') USING utf8mb4), \
                        CONVERT(IFNULL(Alter_priv,'N') USING utf8mb4), \
                        CONVERT(IFNULL(Show_db_priv,'N') USING utf8mb4), \
                        CONVERT(IFNULL(Super_priv,'N') USING utf8mb4), \
                        CONVERT(IFNULL(Create_tmp_table_priv,'N') USING utf8mb4), \
                        CONVERT(IFNULL(Lock_tables_priv,'N') USING utf8mb4), \
                        CONVERT(IFNULL(Execute_priv,'N') USING utf8mb4), \
                        CONVERT(IFNULL(Repl_slave_priv,'N') USING utf8mb4), \
                        CONVERT(IFNULL(Repl_client_priv,'N') USING utf8mb4), \
                        CONVERT(IFNULL(Create_view_priv,'N') USING utf8mb4), \
                        CONVERT(IFNULL(Show_view_priv,'N') USING utf8mb4), \
                        CONVERT(IFNULL(Create_routine_priv,'N') USING utf8mb4), \
                        CONVERT(IFNULL(Alter_routine_priv,'N') USING utf8mb4), \
                        CONVERT(IFNULL(Create_user_priv,'N') USING utf8mb4), \
                        CONVERT(IFNULL(Event_priv,'N') USING utf8mb4), \
                        CONVERT(IFNULL(Trigger_priv,'N') USING utf8mb4) \
                 FROM mysql.user WHERE User = ? AND Host = ?",
            )
            .bind(user)
            .bind(host)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| DriverError::Message(format!("user `{user}`@`{host}` not found")))?;

            let name = mysql_text(&row, 0)?;
            let host_s = mysql_text(&row, 1)?;
            let plugin = mysql_text(&row, 2).unwrap_or_default();

            let mut server_privs = HashSet::new();
            let col_map: &[(&str, usize)] = &[
                ("SELECT", 3),
                ("INSERT", 4),
                ("UPDATE", 5),
                ("DELETE", 6),
                ("CREATE", 7),
                ("DROP", 8),
                ("RELOAD", 9),
                ("SHUTDOWN", 10),
                ("PROCESS", 11),
                ("FILE", 12),
                ("GRANT OPTION", 13),
                ("REFERENCES", 14),
                ("INDEX", 15),
                ("ALTER", 16),
                ("SHOW DATABASES", 17),
                ("SUPER", 18),
                ("CREATE TEMPORARY TABLES", 19),
                ("LOCK TABLES", 20),
                ("EXECUTE", 21),
                ("REPLICATION SLAVE", 22),
                ("REPLICATION CLIENT", 23),
                ("CREATE VIEW", 24),
                ("SHOW VIEW", 25),
                ("CREATE ROUTINE", 26),
                ("ALTER ROUTINE", 27),
                ("CREATE USER", 28),
                ("EVENT", 29),
                ("TRIGGER", 30),
            ];
            for (priv_name, idx) in col_map {
                let v = mysql_text(&row, *idx).unwrap_or_else(|_| "N".into());
                if v.eq_ignore_ascii_case("Y") {
                    server_privs.insert((*priv_name).to_string());
                }
            }

            let db_rows = sqlx::query(
                "SELECT CONVERT(Db USING utf8mb4), \
                        CONVERT(IFNULL(Select_priv,'N') USING utf8mb4), \
                        CONVERT(IFNULL(Insert_priv,'N') USING utf8mb4), \
                        CONVERT(IFNULL(Update_priv,'N') USING utf8mb4), \
                        CONVERT(IFNULL(Delete_priv,'N') USING utf8mb4), \
                        CONVERT(IFNULL(Create_priv,'N') USING utf8mb4), \
                        CONVERT(IFNULL(Drop_priv,'N') USING utf8mb4), \
                        CONVERT(IFNULL(Grant_priv,'N') USING utf8mb4), \
                        CONVERT(IFNULL(References_priv,'N') USING utf8mb4), \
                        CONVERT(IFNULL(Index_priv,'N') USING utf8mb4), \
                        CONVERT(IFNULL(Alter_priv,'N') USING utf8mb4), \
                        CONVERT(IFNULL(Create_tmp_table_priv,'N') USING utf8mb4), \
                        CONVERT(IFNULL(Lock_tables_priv,'N') USING utf8mb4), \
                        CONVERT(IFNULL(Create_view_priv,'N') USING utf8mb4), \
                        CONVERT(IFNULL(Show_view_priv,'N') USING utf8mb4), \
                        CONVERT(IFNULL(Create_routine_priv,'N') USING utf8mb4), \
                        CONVERT(IFNULL(Alter_routine_priv,'N') USING utf8mb4), \
                        CONVERT(IFNULL(Execute_priv,'N') USING utf8mb4), \
                        CONVERT(IFNULL(Event_priv,'N') USING utf8mb4), \
                        CONVERT(IFNULL(Trigger_priv,'N') USING utf8mb4) \
                 FROM mysql.db WHERE User = ? AND Host = ? ORDER BY Db",
            )
            .bind(user)
            .bind(host)
            .fetch_all(pool)
            .await
            .unwrap_or_default();

            let db_map: &[(&str, usize)] = &[
                ("SELECT", 1),
                ("INSERT", 2),
                ("UPDATE", 3),
                ("DELETE", 4),
                ("CREATE", 5),
                ("DROP", 6),
                ("GRANT OPTION", 7),
                ("REFERENCES", 8),
                ("INDEX", 9),
                ("ALTER", 10),
                ("CREATE TEMPORARY TABLES", 11),
                ("LOCK TABLES", 12),
                ("CREATE VIEW", 13),
                ("SHOW VIEW", 14),
                ("CREATE ROUTINE", 15),
                ("ALTER ROUTINE", 16),
                ("EXECUTE", 17),
                ("EVENT", 18),
                ("TRIGGER", 19),
            ];
            let mut db_grants = Vec::new();
            for drow in db_rows {
                let database = mysql_text(&drow, 0).unwrap_or_default();
                let mut privileges = HashSet::new();
                for (priv_name, idx) in db_map {
                    let v = mysql_text(&drow, *idx).unwrap_or_else(|_| "N".into());
                    if v.eq_ignore_ascii_case("Y") {
                        privileges.insert((*priv_name).to_string());
                    }
                }
                if !privileges.is_empty() {
                    db_grants.push(DbGrant {
                        database,
                        privileges,
                    });
                }
            }

            Ok(UserDraft {
                is_existing: true,
                name,
                host: host_s.clone(),
                plugin,
                password: String::new(),
                password_repeat: String::new(),
                password_expiration: "DEFAULT".into(),
                server_privs,
                db_grants,
                original_name: user.to_string(),
                original_host: host.to_string(),
            })
        }
        DbConnection::Postgres(_) | DbConnection::Sqlite(_) => Err(DriverError::Message(
            "user management is only supported for MySQL/MariaDB".into(),
        )),
    }
}

/// Build SQL statements to create/update a user from a draft.
pub fn build_user_save_sql(draft: &UserDraft) -> Result<Vec<String>> {
    if draft.name.is_empty() {
        return Err(DriverError::Message("username required".into()));
    }
    if !draft.password.is_empty() && draft.password != draft.password_repeat {
        return Err(DriverError::Message("passwords do not match".into()));
    }

    let account = format!("'{}'@'{}'", escape_lit(&draft.name), escape_lit(&draft.host));
    let mut stmts = Vec::new();

    if draft.is_existing {
        let orig = format!(
            "'{}'@'{}'",
            escape_lit(&draft.original_name),
            escape_lit(&draft.original_host)
        );
        if !draft.password.is_empty() {
            stmts.push(format!(
                "ALTER USER {orig} {}",
                identified_clause(&draft.plugin, &draft.password)
            ));
        }
        // Revoke all global then re-grant (simple sync)
        stmts.push(format!("REVOKE ALL PRIVILEGES, GRANT OPTION FROM {orig}"));
        if !draft.server_privs.is_empty() {
            let (privs, with_grant) = format_priv_list(&draft.server_privs);
            let mut g = format!("GRANT {privs} ON *.* TO {orig}");
            if with_grant {
                g.push_str(" WITH GRANT OPTION");
            }
            stmts.push(g);
        }
        for grant in &draft.db_grants {
            if grant.privileges.is_empty() {
                continue;
            }
            let db = escape_ident(&grant.database);
            let (privs, with_grant) = format_priv_list(&grant.privileges);
            let mut g = format!("GRANT {privs} ON `{db}`.* TO {orig}");
            if with_grant {
                g.push_str(" WITH GRANT OPTION");
            }
            stmts.push(g);
        }
        stmts.push("FLUSH PRIVILEGES".into());
        let _ = account; // rename not supported in MVP
    } else {
        if draft.password.is_empty() {
            return Err(DriverError::Message(
                "password required for new users".into(),
            ));
        }
        stmts.push(format!(
            "CREATE USER {account} {}",
            identified_clause(&draft.plugin, &draft.password)
        ));
        if !draft.server_privs.is_empty() {
            let (privs, with_grant) = format_priv_list(&draft.server_privs);
            let mut g = format!("GRANT {privs} ON *.* TO {account}");
            if with_grant {
                g.push_str(" WITH GRANT OPTION");
            }
            stmts.push(g);
        }
        for grant in &draft.db_grants {
            if grant.privileges.is_empty() {
                continue;
            }
            let db = escape_ident(&grant.database);
            let (privs, with_grant) = format_priv_list(&grant.privileges);
            let mut g = format!("GRANT {privs} ON `{db}`.* TO {account}");
            if with_grant {
                g.push_str(" WITH GRANT OPTION");
            }
            stmts.push(g);
        }
        stmts.push("FLUSH PRIVILEGES".into());
    }

    Ok(stmts)
}

pub fn preview_user_save_sql(draft: &UserDraft) -> String {
    match build_user_save_sql(draft) {
        Ok(stmts) => stmts.join(";\n") + ";",
        Err(e) => format!("-- {e}"),
    }
}

pub async fn save_user(conn: &DbConnection, draft: &UserDraft) -> Result<()> {
    let stmts = build_user_save_sql(draft)?;
    for sql in stmts {
        execute_sql(conn, &sql).await?;
    }
    Ok(())
}

pub async fn drop_user(conn: &DbConnection, user: &str, host: &str) -> Result<()> {
    match conn {
        DbConnection::Mysql(_) => {
            let sql = format!(
                "DROP USER IF EXISTS '{}'@'{}'",
                escape_lit(user),
                escape_lit(host)
            );
            execute_sql(conn, &sql).await?;
            Ok(())
        }
        DbConnection::Postgres(_) | DbConnection::Sqlite(_) => Err(DriverError::Message(
            "user management is only supported for MySQL/MariaDB".into(),
        )),
    }
}

/// MariaDB-compatible auth clause (MySQL 8 `IDENTIFIED WITH x BY` is invalid there).
fn identified_clause(plugin: &str, password: &str) -> String {
    let pw = escape_lit(password);
    let plugin = plugin.trim();
    if plugin.is_empty()
        || plugin.eq_ignore_ascii_case("mysql_native_password")
        || plugin.eq_ignore_ascii_case("caching_sha2_password")
    {
        // Works on MariaDB and MySQL.
        format!("IDENTIFIED BY '{pw}'")
    } else if plugin.eq_ignore_ascii_case("unix_socket")
        || plugin.eq_ignore_ascii_case("auth_socket")
    {
        format!("IDENTIFIED VIA {plugin}")
    } else {
        // ed25519 and other MariaDB plugins
        format!("IDENTIFIED VIA {plugin} USING PASSWORD('{pw}')")
    }
}

fn format_priv_list(privs: &HashSet<String>) -> (String, bool) {
    let with_grant = privs.iter().any(|p| p.eq_ignore_ascii_case("GRANT OPTION"));
    let mut owned: Vec<String> = privs
        .iter()
        .filter_map(|p| {
            let upper = p.to_uppercase();
            if upper == "GRANT OPTION" {
                None
            } else {
                Some(upper)
            }
        })
        .collect();
    owned.sort();
    if owned.is_empty() {
        ("USAGE".into(), with_grant)
    } else {
        (owned.join(", "), with_grant)
    }
}

fn escape_lit(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\'', "\\'")
}

fn escape_ident(s: &str) -> String {
    s.replace('`', "``")
}

fn row_u32(row: &sqlx::mysql::MySqlRow, idx: usize) -> u32 {
    row.try_get::<i64, _>(idx)
        .ok()
        .map(|n| n.max(0) as u32)
        .or_else(|| row.try_get::<u32, _>(idx).ok())
        .unwrap_or(0)
}

/// Decode MySQL text that may arrive as VARCHAR or BINARY.
fn mysql_text(row: &sqlx::mysql::MySqlRow, idx: usize) -> Result<String> {
    if let Ok(s) = row.try_get::<String, _>(idx) {
        return Ok(s);
    }
    if let Ok(bytes) = row.try_get::<Vec<u8>, _>(idx) {
        return Ok(String::from_utf8_lossy(&bytes).into_owned());
    }
    Err(DriverError::Message(format!(
        "cannot decode text column {idx}"
    )))
}

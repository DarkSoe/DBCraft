use dbcraft_core::{ObjectInfo, SchemaKind, SchemaNode};
use sqlx::Row;

use crate::{DbConnection, DriverError, Result, execute_sql};

pub async fn list_databases(conn: &DbConnection) -> Result<Vec<SchemaNode>> {
    match conn {
        DbConnection::Mysql(pool) => {
            let rows = sqlx::query(
                "SELECT SCHEMA_NAME FROM information_schema.SCHEMATA ORDER BY SCHEMA_NAME",
            )
            .fetch_all(pool)
            .await?;
            let mut out = Vec::with_capacity(rows.len());
            for row in rows {
                let name: String = row.try_get(0).map_err(DriverError::from)?;
                out.push(SchemaNode::leaf(SchemaKind::Database, name));
            }
            Ok(out)
        }
        DbConnection::Postgres(pool) => {
            let rows = sqlx::query(
                "SELECT datname FROM pg_database WHERE datistemplate = false ORDER BY datname",
            )
            .fetch_all(pool)
            .await?;
            let mut out = Vec::with_capacity(rows.len());
            for row in rows {
                let name: String = row.try_get(0).map_err(DriverError::from)?;
                out.push(SchemaNode::leaf(SchemaKind::Database, name));
            }
            Ok(out)
        }
        DbConnection::Sqlite(_) => Ok(vec![SchemaNode::leaf(SchemaKind::Database, "main")]),
    }
}

pub async fn list_tables(conn: &DbConnection, database: &str) -> Result<Vec<SchemaNode>> {
    match conn {
        DbConnection::Mysql(pool) => {
            let rows = sqlx::query(
                "SELECT TABLE_NAME, TABLE_TYPE FROM information_schema.TABLES \
                 WHERE TABLE_SCHEMA = ? ORDER BY TABLE_NAME",
            )
            .bind(database)
            .fetch_all(pool)
            .await?;
            let mut out = Vec::with_capacity(rows.len());
            for row in rows {
                let name: String = row.try_get(0).map_err(DriverError::from)?;
                let ty: String = row.try_get::<String, _>(1).unwrap_or_else(|_| String::new());
                let kind = if ty.eq_ignore_ascii_case("VIEW") {
                    SchemaKind::View
                } else {
                    SchemaKind::Table
                };
                out.push(SchemaNode::leaf(kind, name));
            }
            Ok(out)
        }
        DbConnection::Postgres(pool) => {
            let rows = sqlx::query(
                "SELECT table_name, table_type FROM information_schema.tables \
                 WHERE table_catalog = $1 AND table_schema = 'public' \
                 ORDER BY table_name",
            )
            .bind(database)
            .fetch_all(pool)
            .await?;
            let mut out = Vec::with_capacity(rows.len());
            for row in rows {
                let name: String = row.try_get(0).map_err(DriverError::from)?;
                let ty: String = row.try_get::<String, _>(1).unwrap_or_else(|_| String::new());
                let kind = if ty.eq_ignore_ascii_case("VIEW") {
                    SchemaKind::View
                } else {
                    SchemaKind::Table
                };
                out.push(SchemaNode::leaf(kind, name));
            }
            Ok(out)
        }
        DbConnection::Sqlite(pool) => {
            let rows = sqlx::query(
                "SELECT name, type FROM sqlite_master \
                 WHERE type IN ('table','view') AND name NOT LIKE 'sqlite_%' \
                 ORDER BY name",
            )
            .fetch_all(pool)
            .await?;
            let mut out = Vec::with_capacity(rows.len());
            for row in rows {
                let name: String = row.try_get(0).map_err(DriverError::from)?;
                let ty: String = row.try_get::<String, _>(1).unwrap_or_else(|_| String::new());
                let kind = if ty.eq_ignore_ascii_case("view") {
                    SchemaKind::View
                } else {
                    SchemaKind::Table
                };
                out.push(SchemaNode::leaf(kind, name));
            }
            Ok(out)
        }
    }
}

/// Rich object list for the Navicat-style Objects main view.
pub async fn list_objects(conn: &DbConnection, database: &str) -> Result<Vec<ObjectInfo>> {
    match conn {
        DbConnection::Mysql(pool) => {
            let rows = sqlx::query(
                "SELECT TABLE_NAME, TABLE_TYPE, ENGINE, TABLE_ROWS, DATA_LENGTH, \
                        UPDATE_TIME, TABLE_COMMENT \
                 FROM information_schema.TABLES \
                 WHERE TABLE_SCHEMA = ? \
                 ORDER BY TABLE_TYPE, TABLE_NAME",
            )
            .bind(database)
            .fetch_all(pool)
            .await?;
            let mut out = Vec::with_capacity(rows.len());
            for row in rows {
                let name: String = row.try_get(0).map_err(DriverError::from)?;
                let ty: String = row.try_get::<String, _>(1).unwrap_or_else(|_| String::new());
                let kind = if ty.eq_ignore_ascii_case("VIEW") {
                    SchemaKind::View
                } else {
                    SchemaKind::Table
                };
                let engine: String = row.try_get::<String, _>(2).unwrap_or_else(|_| String::new());
                let rows_est: Option<u64> = row.try_get::<i64, _>(3).ok().map(|n| n as u64);
                let data_len: Option<u64> = row.try_get::<i64, _>(4).ok().map(|n| n as u64);
                let updated: String = row
                    .try_get::<chrono::NaiveDateTime, _>(5)
                    .map(|t| t.to_string())
                    .or_else(|_| row.try_get::<String, _>(5))
                    .unwrap_or_default();
                let comment: String = row.try_get::<String, _>(6).unwrap_or_else(|_| String::new());
                out.push(ObjectInfo {
                    name,
                    kind,
                    engine,
                    row_estimate: rows_est,
                    data_length: data_len,
                    updated_at: updated,
                    comment,
                });
            }
            Ok(out)
        }
        DbConnection::Postgres(pool) => {
            let rows = sqlx::query(
                "SELECT table_name, table_type \
                 FROM information_schema.tables \
                 WHERE table_catalog = $1 AND table_schema = 'public' \
                 ORDER BY table_type, table_name",
            )
            .bind(database)
            .fetch_all(pool)
            .await?;
            let mut out = Vec::with_capacity(rows.len());
            for row in rows {
                let name: String = row.try_get(0).map_err(DriverError::from)?;
                let ty: String = row.try_get::<String, _>(1).unwrap_or_else(|_| String::new());
                let kind = if ty.eq_ignore_ascii_case("VIEW") {
                    SchemaKind::View
                } else {
                    SchemaKind::Table
                };
                out.push(ObjectInfo {
                    name,
                    kind,
                    engine: String::new(),
                    row_estimate: None,
                    data_length: None,
                    updated_at: String::new(),
                    comment: String::new(),
                });
            }
            Ok(out)
        }
        DbConnection::Sqlite(pool) => {
            let rows = sqlx::query(
                "SELECT name, type FROM sqlite_master \
                 WHERE type IN ('table','view') AND name NOT LIKE 'sqlite_%' \
                 ORDER BY type, name",
            )
            .fetch_all(pool)
            .await?;
            let mut out = Vec::with_capacity(rows.len());
            for row in rows {
                let name: String = row.try_get(0).map_err(DriverError::from)?;
                let ty: String = row.try_get::<String, _>(1).unwrap_or_else(|_| String::new());
                let kind = if ty.eq_ignore_ascii_case("view") {
                    SchemaKind::View
                } else {
                    SchemaKind::Table
                };
                out.push(ObjectInfo {
                    name,
                    kind,
                    engine: "sqlite".into(),
                    row_estimate: None,
                    data_length: None,
                    updated_at: String::new(),
                    comment: String::new(),
                });
            }
            Ok(out)
        }
    }
}

/// Rich column metadata for the table designer.
#[derive(Debug, Clone)]
pub struct ColumnDesignInfo {
    pub name: String,
    pub data_type: String,
    pub length: Option<u64>,
    pub scale: Option<u64>,
    pub not_null: bool,
    pub primary_key: bool,
    pub auto_increment: bool,
    pub comment: String,
}

pub async fn list_column_design(
    conn: &DbConnection,
    database: &str,
    table: &str,
) -> Result<Vec<ColumnDesignInfo>> {
    match conn {
        DbConnection::Mysql(pool) => {
            // COLUMN_TYPE has display width / precision (e.g. int(11), decimal(10,2),
            // double(32,2)) — DATA_TYPE alone loses length/scale.
            let rows = sqlx::query(
                "SELECT CONVERT(COLUMN_NAME USING utf8mb4), \
                        CONVERT(COLUMN_TYPE USING utf8mb4), \
                        CONVERT(DATA_TYPE USING utf8mb4), \
                        CONVERT(IS_NULLABLE USING utf8mb4), \
                        CONVERT(COLUMN_KEY USING utf8mb4), \
                        CONVERT(EXTRA USING utf8mb4), \
                        CONVERT(IFNULL(COLUMN_COMMENT, '') USING utf8mb4) \
                 FROM information_schema.COLUMNS \
                 WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ? \
                 ORDER BY ORDINAL_POSITION",
            )
            .bind(database)
            .bind(table)
            .fetch_all(pool)
            .await?;
            let mut out = Vec::with_capacity(rows.len());
            for row in rows {
                let name: String = row.try_get(0).map_err(DriverError::from)?;
                let column_type: String = row.try_get(1).unwrap_or_default();
                let data_type: String = row.try_get(2).unwrap_or_default();
                let nullable: String = row.try_get(3).unwrap_or_else(|_| "YES".into());
                let key: String = row.try_get(4).unwrap_or_default();
                let extra: String = row.try_get(5).unwrap_or_default();
                let comment: String = row.try_get(6).unwrap_or_default();
                let (parsed_type, length, scale) = parse_mysql_column_type(&column_type, &data_type);
                out.push(ColumnDesignInfo {
                    name,
                    data_type: parsed_type,
                    length,
                    scale,
                    not_null: nullable.eq_ignore_ascii_case("NO"),
                    primary_key: key.eq_ignore_ascii_case("PRI"),
                    auto_increment: extra.to_ascii_lowercase().contains("auto_increment"),
                    comment,
                });
            }
            Ok(out)
        }
        DbConnection::Postgres(_) | DbConnection::Sqlite(_) => {
            // Fall back to simple column list
            let cols = list_table_columns(conn, database, table).await?;
            Ok(cols
                .into_iter()
                .map(|c| ColumnDesignInfo {
                    name: c.name,
                    data_type: c.detail.to_uppercase(),
                    length: None,
                    scale: None,
                    not_null: false,
                    primary_key: false,
                    auto_increment: false,
                    comment: String::new(),
                })
                .collect())
        }
    }
}

/// Parse MySQL `COLUMN_TYPE` like `int(11)`, `varchar(255)`, `decimal(10,2)`.
fn parse_mysql_column_type(column_type: &str, data_type: &str) -> (String, Option<u64>, Option<u64>) {
    let fallback = data_type.trim().to_uppercase();
    let ct = column_type.trim();
    if ct.is_empty() {
        return (fallback, None, None);
    }
    // First token may be `int(11)` or `enum('a','b')` before `unsigned`.
    let token = ct.split_whitespace().next().unwrap_or(ct);
    let lower = token.to_ascii_lowercase();
    if lower.starts_with("enum") || lower.starts_with("set") {
        return (fallback, None, None);
    }
    let Some(open) = token.find('(') else {
        return (token.to_uppercase(), None, None);
    };
    let type_name = token[..open].to_uppercase();
    let Some(close) = token[open + 1..].find(')') else {
        return (type_name, None, None);
    };
    let args = &token[open + 1..open + 1 + close];
    let mut parts = args.split(',').map(str::trim);
    let length = parts.next().and_then(|p| p.parse::<u64>().ok());
    let scale = parts.next().and_then(|p| p.parse::<u64>().ok());
    (type_name, length, scale)
}

pub async fn list_table_columns(
    conn: &DbConnection,
    database: &str,
    table: &str,
) -> Result<Vec<SchemaNode>> {
    match conn {
        DbConnection::Mysql(pool) => {
            let rows = sqlx::query(
                "SELECT COLUMN_NAME, COLUMN_TYPE FROM information_schema.COLUMNS \
                 WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ? ORDER BY ORDINAL_POSITION",
            )
            .bind(database)
            .bind(table)
            .fetch_all(pool)
            .await?;
            let mut out = Vec::with_capacity(rows.len());
            for row in rows {
                let name: String = row.try_get(0).map_err(DriverError::from)?;
                let ty: String = row.try_get::<String, _>(1).unwrap_or_else(|_| String::new());
                out.push(SchemaNode::leaf(SchemaKind::Column, name).with_detail(ty));
            }
            Ok(out)
        }
        DbConnection::Postgres(pool) => {
            let rows = sqlx::query(
                "SELECT column_name, data_type FROM information_schema.columns \
                 WHERE table_catalog = $1 AND table_schema = 'public' AND table_name = $2 \
                 ORDER BY ordinal_position",
            )
            .bind(database)
            .bind(table)
            .fetch_all(pool)
            .await?;
            let mut out = Vec::with_capacity(rows.len());
            for row in rows {
                let name: String = row.try_get(0).map_err(DriverError::from)?;
                let ty: String = row.try_get::<String, _>(1).unwrap_or_else(|_| String::new());
                out.push(SchemaNode::leaf(SchemaKind::Column, name).with_detail(ty));
            }
            Ok(out)
        }
        DbConnection::Sqlite(pool) => {
            // PRAGMA table_info — table name cannot be bound; validate identifier.
            if !is_safe_ident(table) {
                return Err(DriverError::Message(
                    "invalid table name for SQLite pragma".into(),
                ));
            }
            let sql = format!("PRAGMA table_info(\"{table}\")");
            let rows = sqlx::query(&sql).fetch_all(pool).await?;
            let mut out = Vec::with_capacity(rows.len());
            for row in rows {
                let name: String = row.try_get(1).map_err(DriverError::from)?;
                let ty: String = row.try_get::<String, _>(2).unwrap_or_else(|_| String::new());
                out.push(SchemaNode::leaf(SchemaKind::Column, name).with_detail(ty));
            }
            Ok(out)
        }
    }
}

pub async fn list_primary_keys(
    conn: &DbConnection,
    database: &str,
    table: &str,
) -> Result<Vec<String>> {
    match conn {
        DbConnection::Mysql(pool) => {
            let rows = sqlx::query(
                "SELECT COLUMN_NAME FROM information_schema.KEY_COLUMN_USAGE \
                 WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ? AND CONSTRAINT_NAME = 'PRIMARY' \
                 ORDER BY ORDINAL_POSITION",
            )
            .bind(database)
            .bind(table)
            .fetch_all(pool)
            .await?;
            let mut out = Vec::with_capacity(rows.len());
            for row in rows {
                out.push(row.try_get(0).map_err(DriverError::from)?);
            }
            Ok(out)
        }
        DbConnection::Postgres(pool) => {
            let rows = sqlx::query(
                "SELECT a.attname \
                 FROM pg_index i \
                 JOIN pg_attribute a ON a.attrelid = i.indrelid AND a.attnum = ANY(i.indkey) \
                 JOIN pg_class c ON c.oid = i.indrelid \
                 JOIN pg_namespace n ON n.oid = c.relnamespace \
                 WHERE i.indisprimary AND c.relname = $1 AND n.nspname = 'public' \
                 ORDER BY a.attnum",
            )
            .bind(table)
            .fetch_all(pool)
            .await?;
            let mut out = Vec::with_capacity(rows.len());
            for row in rows {
                out.push(row.try_get(0).map_err(DriverError::from)?);
            }
            Ok(out)
        }
        DbConnection::Sqlite(pool) => {
            if !is_safe_ident(table) {
                return Err(DriverError::Message("invalid table name".into()));
            }
            let sql = format!("PRAGMA table_info(\"{table}\")");
            let rows = sqlx::query(&sql).fetch_all(pool).await?;
            let mut out = Vec::new();
            for row in rows {
                let pk: i64 = row.try_get(5).unwrap_or(0);
                if pk > 0 {
                    let name: String = row.try_get(1).map_err(DriverError::from)?;
                    out.push((pk, name));
                }
            }
            out.sort_by_key(|(pk, _)| *pk);
            Ok(out.into_iter().map(|(_, n)| n).collect())
        }
    }
}

pub async fn preview_table(
    conn: &DbConnection,
    database: &str,
    table: &str,
    limit: u32,
) -> Result<dbcraft_core::QueryResult> {
    let limit = limit.clamp(1, 100_000);
    if !is_safe_ident(table) {
        return Err(DriverError::Message("invalid table name".into()));
    }
    let sql = match conn {
        DbConnection::Mysql(_) => {
            if !is_safe_ident(database) {
                return Err(DriverError::Message("invalid database name".into()));
            }
            format!("SELECT * FROM `{database}`.`{table}` LIMIT {limit}")
        }
        DbConnection::Postgres(_) => {
            format!("SELECT * FROM \"{table}\" LIMIT {limit}")
        }
        DbConnection::Sqlite(_) => {
            format!("SELECT * FROM \"{table}\" LIMIT {limit}")
        }
    };
    execute_sql(conn, &sql).await
}

/// Routines (functions + procedures) for the Functions category.
pub async fn list_routines(conn: &DbConnection, database: &str) -> Result<Vec<ObjectInfo>> {
    match conn {
        DbConnection::Mysql(pool) => {
            let rows = sqlx::query(
                "SELECT ROUTINE_NAME, ROUTINE_TYPE, IFNULL(ROUTINE_COMMENT, ''), \
                        IFNULL(CREATED, '') \
                 FROM information_schema.ROUTINES \
                 WHERE ROUTINE_SCHEMA = ? \
                 ORDER BY ROUTINE_TYPE, ROUTINE_NAME",
            )
            .bind(database)
            .fetch_all(pool)
            .await?;
            let mut out = Vec::with_capacity(rows.len());
            for row in rows {
                let name: String = row.try_get(0).map_err(DriverError::from)?;
                let ty: String = row.try_get(1).unwrap_or_default();
                let comment: String = row.try_get(2).unwrap_or_default();
                let created: String = row
                    .try_get::<chrono::NaiveDateTime, _>(3)
                    .map(|t| t.to_string())
                    .or_else(|_| row.try_get::<String, _>(3))
                    .unwrap_or_default();
                let kind = if ty.eq_ignore_ascii_case("PROCEDURE") {
                    SchemaKind::Procedure
                } else {
                    SchemaKind::Function
                };
                out.push(ObjectInfo {
                    name,
                    kind,
                    engine: ty,
                    row_estimate: None,
                    data_length: None,
                    updated_at: created,
                    comment,
                });
            }
            Ok(out)
        }
        DbConnection::Postgres(pool) => {
            let rows = sqlx::query(
                "SELECT p.proname, CASE WHEN p.prokind = 'p' THEN 'PROCEDURE' ELSE 'FUNCTION' END \
                 FROM pg_proc p \
                 JOIN pg_namespace n ON n.oid = p.pronamespace \
                 WHERE n.nspname = 'public' \
                 ORDER BY 2, 1",
            )
            .fetch_all(pool)
            .await?;
            let mut out = Vec::with_capacity(rows.len());
            for row in rows {
                let name: String = row.try_get(0).map_err(DriverError::from)?;
                let ty: String = row.try_get(1).unwrap_or_default();
                let kind = if ty.eq_ignore_ascii_case("PROCEDURE") {
                    SchemaKind::Procedure
                } else {
                    SchemaKind::Function
                };
                out.push(ObjectInfo {
                    name,
                    kind,
                    engine: ty,
                    row_estimate: None,
                    data_length: None,
                    updated_at: String::new(),
                    comment: String::new(),
                });
            }
            Ok(out)
        }
        DbConnection::Sqlite(_) => Ok(Vec::new()),
    }
}

/// Fetch CREATE definition for a table/view/function/procedure.
pub async fn object_definition(
    conn: &DbConnection,
    database: &str,
    name: &str,
    kind: SchemaKind,
) -> Result<String> {
    if !is_safe_ident(name) {
        return Err(DriverError::Message("invalid object name".into()));
    }
    match conn {
        DbConnection::Mysql(_) => {
            if !is_safe_ident(database) {
                return Err(DriverError::Message("invalid database name".into()));
            }
            let sql = match kind {
                SchemaKind::Table => format!("SHOW CREATE TABLE `{database}`.`{name}`"),
                SchemaKind::View => format!("SHOW CREATE VIEW `{database}`.`{name}`"),
                SchemaKind::Function => format!("SHOW CREATE FUNCTION `{database}`.`{name}`"),
                SchemaKind::Procedure => format!("SHOW CREATE PROCEDURE `{database}`.`{name}`"),
                _ => {
                    return Err(DriverError::Message(format!(
                        "definition not supported for {kind:?}"
                    )));
                }
            };
            let result = execute_sql(conn, &sql).await?;
            // SHOW CREATE returns definition in column 1 (index 1)
            let def = result
                .rows
                .first()
                .and_then(|r| r.values.get(1))
                .map(|v| v.display())
                .unwrap_or_default();
            Ok(def)
        }
        DbConnection::Postgres(_) => {
            let sql = match kind {
                SchemaKind::View => format!(
                    "SELECT pg_get_viewdef('{name}'::regclass, true)"
                ),
                SchemaKind::Function | SchemaKind::Procedure => format!(
                    "SELECT pg_get_functiondef(p.oid) FROM pg_proc p \
                     JOIN pg_namespace n ON n.oid = p.pronamespace \
                     WHERE p.proname = '{name}' AND n.nspname = 'public' LIMIT 1"
                ),
                SchemaKind::Table => {
                    return Err(DriverError::Message(
                        "SHOW CREATE TABLE not available on Postgres in MVP".into(),
                    ));
                }
                _ => {
                    return Err(DriverError::Message(format!(
                        "definition not supported for {kind:?}"
                    )));
                }
            };
            let result = execute_sql(conn, &sql).await?;
            Ok(result
                .rows
                .first()
                .and_then(|r| r.values.first())
                .map(|v| v.display())
                .unwrap_or_default())
        }
        DbConnection::Sqlite(pool) => {
            let rows = sqlx::query(
                "SELECT sql FROM sqlite_master WHERE name = ? AND sql IS NOT NULL LIMIT 1",
            )
            .bind(name)
            .fetch_all(pool)
            .await?;
            let sql: String = rows
                .first()
                .and_then(|r| r.try_get(0).ok())
                .unwrap_or_default();
            Ok(sql)
        }
    }
}

pub async fn drop_object(
    conn: &DbConnection,
    database: &str,
    name: &str,
    kind: SchemaKind,
) -> Result<()> {
    if !is_safe_ident(name) {
        return Err(DriverError::Message("invalid object name".into()));
    }
    let sql = match conn {
        DbConnection::Mysql(_) => {
            if !is_safe_ident(database) {
                return Err(DriverError::Message("invalid database name".into()));
            }
            match kind {
                SchemaKind::Table => format!("DROP TABLE IF EXISTS `{database}`.`{name}`"),
                SchemaKind::View => format!("DROP VIEW IF EXISTS `{database}`.`{name}`"),
                SchemaKind::Function => format!("DROP FUNCTION IF EXISTS `{database}`.`{name}`"),
                SchemaKind::Procedure => {
                    format!("DROP PROCEDURE IF EXISTS `{database}`.`{name}`")
                }
                _ => {
                    return Err(DriverError::Message(format!(
                        "drop not supported for {kind:?}"
                    )));
                }
            }
        }
        DbConnection::Postgres(_) => match kind {
            SchemaKind::Table => format!("DROP TABLE IF EXISTS \"{name}\""),
            SchemaKind::View => format!("DROP VIEW IF EXISTS \"{name}\""),
            SchemaKind::Function => format!("DROP FUNCTION IF EXISTS \"{name}\""),
            SchemaKind::Procedure => format!("DROP PROCEDURE IF EXISTS \"{name}\""),
            _ => {
                return Err(DriverError::Message(format!(
                    "drop not supported for {kind:?}"
                )));
            }
        },
        DbConnection::Sqlite(_) => match kind {
            SchemaKind::Table => format!("DROP TABLE IF EXISTS \"{name}\""),
            SchemaKind::View => format!("DROP VIEW IF EXISTS \"{name}\""),
            _ => {
                return Err(DriverError::Message(format!(
                    "drop not supported for {kind:?}"
                )));
            }
        },
    };
    execute_sql(conn, &sql).await?;
    Ok(())
}

fn is_safe_ident(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
}

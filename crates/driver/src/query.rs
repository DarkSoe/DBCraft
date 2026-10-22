use std::time::Instant;

use dbcraft_core::{ColumnInfo, QueryResult, Row as ResultRow};
use sqlx::Column;
use sqlx::Executor;
use sqlx::Row;
use sqlx::TypeInfo;

use crate::value::{
    column_type_name_mysql, column_type_name_pg, column_type_name_sqlite, mysql_value, pg_value,
    sqlite_value,
};
use crate::{DbConnection, DriverError, Result};

const MAX_ROWS: usize = 10_000;

pub async fn execute_sql(conn: &DbConnection, sql: &str) -> Result<QueryResult> {
    let sql = sql.trim();
    if sql.is_empty() {
        return Err(DriverError::Message("SQL is empty".into()));
    }
    let started = Instant::now();
    let statements: Vec<&str> = sql
        .split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if statements.is_empty() {
        return Err(DriverError::Message("SQL is empty".into()));
    }
    let mut last = PartialResult {
        columns: Vec::new(),
        rows: Vec::new(),
        rows_affected: Some(0),
    };
    for stmt in statements {
        last = match conn {
            DbConnection::Mysql(pool) => execute_mysql(pool, stmt).await?,
            DbConnection::Postgres(pool) => execute_postgres(pool, stmt).await?,
            DbConnection::Sqlite(pool) => execute_sqlite(pool, stmt).await?,
        };
    }
    let elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    Ok(QueryResult {
        columns: last.columns,
        rows: last.rows,
        rows_affected: last.rows_affected,
        elapsed_ms,
    })
}

struct PartialResult {
    columns: Vec<ColumnInfo>,
    rows: Vec<ResultRow>,
    rows_affected: Option<u64>,
}

fn looks_like_query(sql: &str) -> bool {
    let head = sql
        .split_whitespace()
        .next()
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();
    matches!(
        head.as_str(),
        "select" | "show" | "describe" | "desc" | "explain" | "with" | "pragma" | "values"
    )
}

async fn execute_mysql(pool: &sqlx::MySqlPool, sql: &str) -> Result<PartialResult> {
    if looks_like_query(sql) {
        let rows = pool.fetch_all(sqlx::query(sql)).await?;
        let columns = if let Some(first) = rows.first() {
            first
                .columns()
                .iter()
                .enumerate()
                .map(|(i, col)| ColumnInfo {
                    name: col.name().to_string(),
                    type_name: column_type_name_mysql(first, i),
                })
                .collect()
        } else {
            // Empty result set still has a schema — needed to insert rows in the grid.
            columns_from_describe_mysql(pool, sql).await?
        };
        let mut out_rows = Vec::new();
        for row in rows.into_iter().take(MAX_ROWS) {
            let mut values = Vec::with_capacity(columns.len());
            for i in 0..columns.len() {
                values.push(mysql_value(&row, i));
            }
            out_rows.push(ResultRow { values });
        }
        Ok(PartialResult {
            columns,
            rows: out_rows,
            rows_affected: None,
        })
    } else {
        let done = sqlx::query(sql).execute(pool).await?;
        Ok(PartialResult {
            columns: Vec::new(),
            rows: Vec::new(),
            rows_affected: Some(done.rows_affected()),
        })
    }
}

async fn columns_from_describe_mysql(
    pool: &sqlx::MySqlPool,
    sql: &str,
) -> Result<Vec<ColumnInfo>> {
    let desc = pool.describe(sql).await.map_err(DriverError::from)?;
    Ok(desc
        .columns()
        .iter()
        .map(|col| ColumnInfo {
            name: col.name().to_string(),
            type_name: col.type_info().name().to_string(),
        })
        .collect())
}

async fn execute_postgres(pool: &sqlx::PgPool, sql: &str) -> Result<PartialResult> {
    if looks_like_query(sql) {
        let rows = pool.fetch_all(sqlx::query(sql)).await?;
        let columns = if let Some(first) = rows.first() {
            first
                .columns()
                .iter()
                .enumerate()
                .map(|(i, col)| ColumnInfo {
                    name: col.name().to_string(),
                    type_name: column_type_name_pg(first, i),
                })
                .collect()
        } else {
            columns_from_describe_pg(pool, sql).await?
        };
        let mut out_rows = Vec::new();
        for row in rows.into_iter().take(MAX_ROWS) {
            let mut values = Vec::with_capacity(columns.len());
            for i in 0..columns.len() {
                values.push(pg_value(&row, i));
            }
            out_rows.push(ResultRow { values });
        }
        Ok(PartialResult {
            columns,
            rows: out_rows,
            rows_affected: None,
        })
    } else {
        let done = sqlx::query(sql).execute(pool).await?;
        Ok(PartialResult {
            columns: Vec::new(),
            rows: Vec::new(),
            rows_affected: Some(done.rows_affected()),
        })
    }
}

async fn columns_from_describe_pg(pool: &sqlx::PgPool, sql: &str) -> Result<Vec<ColumnInfo>> {
    let desc = pool.describe(sql).await.map_err(DriverError::from)?;
    Ok(desc
        .columns()
        .iter()
        .map(|col| ColumnInfo {
            name: col.name().to_string(),
            type_name: col.type_info().name().to_string(),
        })
        .collect())
}

async fn execute_sqlite(pool: &sqlx::SqlitePool, sql: &str) -> Result<PartialResult> {
    if looks_like_query(sql) {
        let rows = pool.fetch_all(sqlx::query(sql)).await?;
        let columns = if let Some(first) = rows.first() {
            first
                .columns()
                .iter()
                .enumerate()
                .map(|(i, col)| ColumnInfo {
                    name: col.name().to_string(),
                    type_name: column_type_name_sqlite(first, i),
                })
                .collect()
        } else {
            columns_from_describe_sqlite(pool, sql).await?
        };
        let mut out_rows = Vec::new();
        for row in rows.into_iter().take(MAX_ROWS) {
            let mut values = Vec::with_capacity(columns.len());
            for i in 0..columns.len() {
                values.push(sqlite_value(&row, i));
            }
            out_rows.push(ResultRow { values });
        }
        Ok(PartialResult {
            columns,
            rows: out_rows,
            rows_affected: None,
        })
    } else {
        let done = sqlx::query(sql).execute(pool).await?;
        Ok(PartialResult {
            columns: Vec::new(),
            rows: Vec::new(),
            rows_affected: Some(done.rows_affected()),
        })
    }
}

async fn columns_from_describe_sqlite(
    pool: &sqlx::SqlitePool,
    sql: &str,
) -> Result<Vec<ColumnInfo>> {
    let desc = pool.describe(sql).await.map_err(DriverError::from)?;
    Ok(desc
        .columns()
        .iter()
        .map(|col| ColumnInfo {
            name: col.name().to_string(),
            type_name: col.type_info().name().to_string(),
        })
        .collect())
}

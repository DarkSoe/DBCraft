use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use dbcraft_core::SqlValue;
use sqlx::Column;
use sqlx::TypeInfo;
use sqlx::ValueRef;
use sqlx::mysql::MySqlRow;
use sqlx::postgres::PgRow;
use sqlx::sqlite::SqliteRow;
use sqlx::Row;

pub fn column_type_name_mysql(row: &MySqlRow, idx: usize) -> String {
    row.column(idx).type_info().name().to_string()
}

pub fn column_type_name_pg(row: &PgRow, idx: usize) -> String {
    row.column(idx).type_info().name().to_string()
}

pub fn column_type_name_sqlite(row: &SqliteRow, idx: usize) -> String {
    row.column(idx).type_info().name().to_string()
}

pub fn mysql_value(row: &MySqlRow, idx: usize) -> SqlValue {
    let Ok(raw) = row.try_get_raw(idx) else {
        return SqlValue::Null;
    };
    if raw.is_null() {
        return SqlValue::Null;
    }
    // Unsigned integers first (MariaDB/MySQL INT UNSIGNED, BIGINT UNSIGNED, …).
    if let Ok(v) = row.try_get::<u64, _>(idx) {
        if v <= i64::MAX as u64 {
            return SqlValue::I64(v as i64);
        }
        return SqlValue::Text(v.to_string());
    }
    if let Ok(v) = row.try_get::<i64, _>(idx) {
        return SqlValue::I64(v);
    }
    if let Ok(v) = row.try_get::<f64, _>(idx) {
        return SqlValue::F64(v);
    }
    if let Ok(v) = row.try_get::<bool, _>(idx) {
        return SqlValue::Bool(v);
    }
    if let Ok(v) = row.try_get::<NaiveDateTime, _>(idx) {
        return SqlValue::Text(v.to_string());
    }
    if let Ok(v) = row.try_get::<DateTime<Utc>, _>(idx) {
        return SqlValue::Text(v.to_rfc3339());
    }
    if let Ok(v) = row.try_get::<NaiveDate, _>(idx) {
        return SqlValue::Text(v.to_string());
    }
    if let Ok(v) = row.try_get::<NaiveTime, _>(idx) {
        return SqlValue::Text(v.to_string());
    }
    if let Ok(v) = row.try_get::<String, _>(idx) {
        return SqlValue::Text(v);
    }
    if let Ok(v) = row.try_get::<Vec<u8>, _>(idx) {
        if looks_like_text(&v) {
            return SqlValue::Text(String::from_utf8_lossy(&v).into_owned());
        }
        return SqlValue::Bytes(v);
    }
    SqlValue::Text(format!("<{ty}>", ty = raw.type_info().name()))
}

pub fn pg_value(row: &PgRow, idx: usize) -> SqlValue {
    let Ok(raw) = row.try_get_raw(idx) else {
        return SqlValue::Null;
    };
    if raw.is_null() {
        return SqlValue::Null;
    }
    if let Ok(v) = row.try_get::<bool, _>(idx) {
        return SqlValue::Bool(v);
    }
    if let Ok(v) = row.try_get::<i64, _>(idx) {
        return SqlValue::I64(v);
    }
    if let Ok(v) = row.try_get::<f64, _>(idx) {
        return SqlValue::F64(v);
    }
    if let Ok(v) = row.try_get::<NaiveDateTime, _>(idx) {
        return SqlValue::Text(v.to_string());
    }
    if let Ok(v) = row.try_get::<DateTime<Utc>, _>(idx) {
        return SqlValue::Text(v.to_rfc3339());
    }
    if let Ok(v) = row.try_get::<NaiveDate, _>(idx) {
        return SqlValue::Text(v.to_string());
    }
    if let Ok(v) = row.try_get::<String, _>(idx) {
        return SqlValue::Text(v);
    }
    if let Ok(v) = row.try_get::<Vec<u8>, _>(idx) {
        return SqlValue::Bytes(v);
    }
    SqlValue::Text(format!("<{ty}>", ty = raw.type_info().name()))
}

pub fn sqlite_value(row: &SqliteRow, idx: usize) -> SqlValue {
    let Ok(raw) = row.try_get_raw(idx) else {
        return SqlValue::Null;
    };
    if raw.is_null() {
        return SqlValue::Null;
    }
    if let Ok(v) = row.try_get::<i64, _>(idx) {
        return SqlValue::I64(v);
    }
    if let Ok(v) = row.try_get::<f64, _>(idx) {
        return SqlValue::F64(v);
    }
    if let Ok(v) = row.try_get::<String, _>(idx) {
        return SqlValue::Text(v);
    }
    if let Ok(v) = row.try_get::<Vec<u8>, _>(idx) {
        if looks_like_text(&v) {
            return SqlValue::Text(String::from_utf8_lossy(&v).into_owned());
        }
        return SqlValue::Bytes(v);
    }
    SqlValue::Null
}

fn looks_like_text(bytes: &[u8]) -> bool {
    !bytes.is_empty()
        && bytes
            .iter()
            .all(|b| b.is_ascii_graphic() || b.is_ascii_whitespace())
}

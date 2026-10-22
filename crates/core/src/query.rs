use serde::{Deserialize, Serialize};

/// A single cell value, serializable for UI and CLI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SqlValue {
    Null,
    Bool(bool),
    I64(i64),
    F64(f64),
    Text(String),
    Bytes(Vec<u8>),
}

impl SqlValue {
    pub fn display(&self) -> String {
        match self {
            Self::Null => "NULL".into(),
            Self::Bool(b) => b.to_string(),
            Self::I64(n) => n.to_string(),
            Self::F64(n) => n.to_string(),
            Self::Text(s) => s.clone(),
            Self::Bytes(b) => format!("<{len} bytes>", len = b.len()),
        }
    }

    /// Value suitable for an editable cell (NULL stays as the word NULL).
    pub fn edit_string(&self) -> String {
        self.display()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColumnInfo {
    pub name: String,
    pub type_name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Row {
    pub values: Vec<SqlValue>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueryResult {
    pub columns: Vec<ColumnInfo>,
    pub rows: Vec<Row>,
    /// Rows affected for non-SELECT statements; `None` for result sets.
    pub rows_affected: Option<u64>,
    pub elapsed_ms: u64,
}

impl QueryResult {
    pub fn empty(elapsed_ms: u64) -> Self {
        Self {
            columns: Vec::new(),
            rows: Vec::new(),
            rows_affected: Some(0),
            elapsed_ms,
        }
    }

    pub fn row_count(&self) -> usize {
        self.rows.len()
    }
}

use serde::{Deserialize, Serialize};

use crate::SchemaKind;

/// A database object shown in the Navicat-style Objects main view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectInfo {
    pub name: String,
    pub kind: SchemaKind,
    pub engine: String,
    pub row_estimate: Option<u64>,
    pub data_length: Option<u64>,
    pub updated_at: String,
    pub comment: String,
}

impl ObjectInfo {
    pub fn table(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            kind: SchemaKind::Table,
            engine: String::new(),
            row_estimate: None,
            data_length: None,
            updated_at: String::new(),
            comment: String::new(),
        }
    }

    pub fn kind_label(&self) -> &'static str {
        match self.kind {
            SchemaKind::View => "View",
            SchemaKind::Table => "Table",
            SchemaKind::Function => "Function",
            SchemaKind::Procedure => "Procedure",
            SchemaKind::User => "User",
            _ => "Object",
        }
    }
}

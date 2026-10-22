use serde::{Deserialize, Serialize};

/// Kind of node in the schema browser tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SchemaKind {
    Connection,
    Database,
    Schema,
    Table,
    View,
    Function,
    Procedure,
    User,
    Column,
    Folder,
}

/// A node in the connection / schema browser.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaNode {
    pub kind: SchemaKind,
    pub name: String,
    /// Optional type / detail (e.g. column type).
    pub detail: String,
    pub children: Vec<SchemaNode>,
    /// When false, children are not fetched yet (expand on demand).
    #[serde(default = "default_loaded")]
    pub children_loaded: bool,
}

fn default_loaded() -> bool {
    true
}

impl SchemaNode {
    pub fn leaf(kind: SchemaKind, name: impl Into<String>) -> Self {
        Self {
            kind,
            name: name.into(),
            detail: String::new(),
            children: Vec::new(),
            children_loaded: true,
        }
    }

    /// Expandable node whose children load on first open.
    pub fn lazy(kind: SchemaKind, name: impl Into<String>) -> Self {
        Self {
            kind,
            name: name.into(),
            detail: String::new(),
            children: Vec::new(),
            children_loaded: false,
        }
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = detail.into();
        self
    }

    pub fn with_children(mut self, children: Vec<SchemaNode>) -> Self {
        self.children = children;
        self.children_loaded = true;
        self
    }

    pub fn is_expandable(&self) -> bool {
        if !self.children_loaded {
            return matches!(
                self.kind,
                SchemaKind::Connection
                    | SchemaKind::Database
                    | SchemaKind::Table
                    | SchemaKind::View
            );
        }
        !self.children.is_empty()
    }
}

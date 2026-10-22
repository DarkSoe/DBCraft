use dbcraft_core::{ConnectionKind, ConnectionProfile, SchemaKind, SslMode, UserDraft};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::jobs::JobId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandId {
    ConnectionCreate,
    ConnectionUpdate,
    ConnectionDelete,
    ConnectionTest,
    ConnectionConnect,
    ConnectionDisconnect,
    SchemaRefresh,
    SchemaExpand,
    DatabaseOpen,
    QueryExecute,
    TablePreview,
    TableDesignLoad,
    RoutinesList,
    UsersList,
    UsersLoad,
    UsersSave,
    UsersDrop,
    ObjectDefinition,
    ObjectDrop,
    SettingsSave,
    AppQuit,
}

impl CommandId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ConnectionCreate => "connection.create",
            Self::ConnectionUpdate => "connection.update",
            Self::ConnectionDelete => "connection.delete",
            Self::ConnectionTest => "connection.test",
            Self::ConnectionConnect => "connection.connect",
            Self::ConnectionDisconnect => "connection.disconnect",
            Self::SchemaRefresh => "schema.refresh",
            Self::SchemaExpand => "schema.expand",
            Self::DatabaseOpen => "database.open",
            Self::QueryExecute => "query.execute",
            Self::TablePreview => "table.preview",
            Self::TableDesignLoad => "table.design_load",
            Self::RoutinesList => "routines.list",
            Self::UsersList => "users.list",
            Self::UsersLoad => "users.load",
            Self::UsersSave => "users.save",
            Self::UsersDrop => "users.drop",
            Self::ObjectDefinition => "object.definition",
            Self::ObjectDrop => "object.drop",
            Self::SettingsSave => "settings.save",
            Self::AppQuit => "app.quit",
        }
    }

    pub fn parse(id: &str) -> Option<Self> {
        Some(match id {
            "connection.create" => Self::ConnectionCreate,
            "connection.update" => Self::ConnectionUpdate,
            "connection.delete" => Self::ConnectionDelete,
            "connection.test" => Self::ConnectionTest,
            "connection.connect" => Self::ConnectionConnect,
            "connection.disconnect" => Self::ConnectionDisconnect,
            "schema.refresh" => Self::SchemaRefresh,
            "schema.expand" => Self::SchemaExpand,
            "database.open" => Self::DatabaseOpen,
            "query.execute" => Self::QueryExecute,
            "table.preview" => Self::TablePreview,
            "table.design_load" => Self::TableDesignLoad,
            "routines.list" => Self::RoutinesList,
            "users.list" => Self::UsersList,
            "users.load" => Self::UsersLoad,
            "users.save" => Self::UsersSave,
            "users.drop" => Self::UsersDrop,
            "object.definition" => Self::ObjectDefinition,
            "object.drop" => Self::ObjectDrop,
            "settings.save" => Self::SettingsSave,
            "app.quit" => Self::AppQuit,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CommandParams {
    #[serde(default)]
    pub profile: Option<ConnectionProfile>,
    #[serde(default)]
    pub profile_id: Option<Uuid>,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub sql: Option<String>,
    #[serde(default)]
    pub database: Option<String>,
    #[serde(default)]
    pub table: Option<String>,
    #[serde(default)]
    pub limit: Option<u32>,
    #[serde(default)]
    pub kind: Option<ConnectionKind>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub database_name: Option<String>,
    #[serde(default)]
    pub file_path: Option<String>,
    #[serde(default)]
    pub ssl: Option<SslMode>,
    /// Schema object kind for definition/drop (table/view/function/…).
    #[serde(default)]
    pub object_kind: Option<SchemaKind>,
    #[serde(default)]
    pub user_draft: Option<UserDraft>,
}

impl CommandParams {
    pub fn from_json(value: &Value) -> Result<Self, serde_json::Error> {
        serde_json::from_value(value.clone())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CommandResult {
    Ok { message: String },
    JobStarted { job_id: JobId },
    Quit,
}

use craft_settings::{DocumentMut, MigrateError, Settings};
use dbcraft_core::ConnectionProfile;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiPrefs {
    pub theme: String,
    pub sidebar_ratio: f32,
    pub editor_ratio: f32,
    pub last_connection_id: Option<Uuid>,
}

impl Default for UiPrefs {
    fn default() -> Self {
        Self {
            theme: "dark".into(),
            sidebar_ratio: 0.22,
            editor_ratio: 0.45,
            last_connection_id: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AppSettings {
    pub connections: Vec<ConnectionProfile>,
    pub ui: UiPrefs,
}

impl Settings for AppSettings {
    const APP_ID: &'static str = "dbcraft";
    const SCHEMA_VERSION: u32 = 1;

    fn migrate(_from: u32, _doc: &mut DocumentMut) -> std::result::Result<(), MigrateError> {
        Ok(())
    }
}

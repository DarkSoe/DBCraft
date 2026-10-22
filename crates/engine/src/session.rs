use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use craft_settings::{Location, SettingsStore};
use dbcraft_core::{
    ConnectionProfile, ObjectInfo, QueryResult, SchemaKind, SchemaNode, UserDraft, UserInfo,
};
use dbcraft_driver::DbConnection;
use tokio::runtime::Runtime;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::commands::{CommandId, CommandParams, CommandResult};
use crate::jobs::{JobId, JobStatus};
use crate::secrets;
use crate::settings::AppSettings;
use crate::{EngineError, Result};

struct LiveConnection {
    profile_id: Uuid,
    conn: DbConnection,
}

/// Application session: profiles, live connection, async jobs.
pub struct Session {
    runtime: Runtime,
    store: SettingsStore<AppSettings>,
    /// Session-only passwords when keyring is unavailable.
    session_passwords: HashMap<Uuid, String>,
    live: Option<LiveConnection>,
    schema_tree: Vec<SchemaNode>,
    last_result: Option<QueryResult>,
    /// Last opened database object list (Navicat Objects main view).
    last_objects: Option<(String, Vec<ObjectInfo>)>,
    /// Last routines list for Functions category.
    last_routines: Option<(String, Vec<ObjectInfo>)>,
    /// Last server user list.
    last_users: Option<Vec<UserInfo>>,
    /// Last loaded user draft.
    last_user_draft: Option<UserDraft>,
    /// Last object definition SQL (SHOW CREATE …).
    last_definition: Option<String>,
    /// Primary key columns from the last table.preview.
    last_pk_columns: Vec<String>,
    /// Columns from the last table.design_load.
    last_column_design: Option<Vec<dbcraft_driver::ColumnDesignInfo>>,
    jobs: Arc<Mutex<HashMap<JobId, JobStatus>>>,
    job_rx: mpsc::UnboundedReceiver<JobStatus>,
    job_tx: mpsc::UnboundedSender<JobStatus>,
    status_message: String,
    quit_requested: bool,
}

impl Session {
    pub fn open() -> Result<Self> {
        let (store, report) = SettingsStore::<AppSettings>::open(Location::Auto);
        let status = if report.has_problems() {
            "settings loaded with warnings".into()
        } else {
            String::new()
        };
        Self::from_store(store, status)
    }

    /// In-memory settings for tests / agents.
    pub fn open_memory() -> Result<Self> {
        let (store, _) = SettingsStore::<AppSettings>::open(Location::Memory);
        Self::from_store(store, String::new())
    }

    fn from_store(store: SettingsStore<AppSettings>, status_message: String) -> Result<Self> {
        let runtime = Runtime::new().map_err(|e| EngineError::Message(e.to_string()))?;
        let (job_tx, job_rx) = mpsc::unbounded_channel();
        Ok(Self {
            runtime,
            store,
            session_passwords: HashMap::new(),
            live: None,
            schema_tree: Vec::new(),
            last_result: None,
            last_objects: None,
            last_routines: None,
            last_users: None,
            last_user_draft: None,
            last_definition: None,
            last_pk_columns: Vec::new(),
            last_column_design: None,
            jobs: Arc::new(Mutex::new(HashMap::new())),
            job_rx,
            job_tx,
            status_message,
            quit_requested: false,
        })
    }

    pub fn settings(&self) -> &AppSettings {
        self.store.get()
    }

    pub fn update_settings<F>(&mut self, f: F)
    where
        F: FnOnce(&mut AppSettings),
    {
        self.store.update(f);
    }

    pub fn tick(&mut self) {
        while let Ok(status) = self.job_rx.try_recv() {
            self.apply_job_status(status);
        }
        let _ = self.store.tick(std::time::Instant::now());
    }

    pub fn flush(&mut self) -> Result<()> {
        self.store
            .flush()
            .map_err(|e| EngineError::Settings(e.to_string()))
    }

    pub fn status_message(&self) -> &str {
        &self.status_message
    }

    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status_message = msg.into();
    }

    pub fn schema_tree(&self) -> &[SchemaNode] {
        &self.schema_tree
    }

    pub fn last_result(&self) -> Option<&QueryResult> {
        self.last_result.as_ref()
    }

    pub fn last_objects(&self) -> Option<&(String, Vec<ObjectInfo>)> {
        self.last_objects.as_ref()
    }

    pub fn last_pk_columns(&self) -> &[String] {
        &self.last_pk_columns
    }

    pub fn last_routines(&self) -> Option<&(String, Vec<ObjectInfo>)> {
        self.last_routines.as_ref()
    }

    pub fn last_users(&self) -> Option<&[UserInfo]> {
        self.last_users.as_deref()
    }

    pub fn last_user_draft(&self) -> Option<&UserDraft> {
        self.last_user_draft.as_ref()
    }

    pub fn last_definition(&self) -> Option<&str> {
        self.last_definition.as_deref()
    }

    pub fn last_column_design(&self) -> Option<&[dbcraft_driver::ColumnDesignInfo]> {
        self.last_column_design.as_deref()
    }

    pub fn active_profile_id(&self) -> Option<Uuid> {
        self.live.as_ref().map(|l| l.profile_id)
    }

    pub fn quit_requested(&self) -> bool {
        self.quit_requested
    }

    pub fn job(&self, id: JobId) -> Option<JobStatus> {
        self.jobs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&id)
            .cloned()
    }

    pub fn profiles(&self) -> &[ConnectionProfile] {
        &self.store.get().connections
    }

    pub fn dispatch(&mut self, id: CommandId, params: CommandParams) -> Result<CommandResult> {
        match id {
            CommandId::ConnectionCreate => self.cmd_connection_create(params),
            CommandId::ConnectionUpdate => self.cmd_connection_update(params),
            CommandId::ConnectionDelete => self.cmd_connection_delete(params),
            CommandId::ConnectionTest => self.cmd_connection_test(params),
            CommandId::ConnectionConnect => self.cmd_connection_connect(params),
            CommandId::ConnectionDisconnect => self.cmd_connection_disconnect(),
            CommandId::SchemaRefresh => self.cmd_schema_refresh(),
            CommandId::SchemaExpand => self.cmd_schema_expand(params),
            CommandId::DatabaseOpen => self.cmd_database_open(params),
            CommandId::QueryExecute => self.cmd_query_execute(params),
            CommandId::TablePreview => self.cmd_table_preview(params),
            CommandId::TableDesignLoad => self.cmd_table_design_load(params),
            CommandId::RoutinesList => self.cmd_routines_list(params),
            CommandId::UsersList => self.cmd_users_list(),
            CommandId::UsersLoad => self.cmd_users_load(params),
            CommandId::UsersSave => self.cmd_users_save(params),
            CommandId::UsersDrop => self.cmd_users_drop(params),
            CommandId::ObjectDefinition => self.cmd_object_definition(params),
            CommandId::ObjectDrop => self.cmd_object_drop(params),
            CommandId::SettingsSave => {
                self.flush()?;
                Ok(CommandResult::Ok {
                    message: "settings saved".into(),
                })
            }
            CommandId::AppQuit => {
                let _ = self.flush();
                self.quit_requested = true;
                Ok(CommandResult::Quit)
            }
        }
    }

    pub fn dispatch_str(&mut self, id: &str, params: CommandParams) -> Result<CommandResult> {
        let cmd = CommandId::parse(id).ok_or_else(|| EngineError::UnknownCommand(id.into()))?;
        self.dispatch(cmd, params)
    }

    fn remember_password(&mut self, id: Uuid, password: Option<String>) -> Result<()> {
        if let Some(pw) = password {
            match secrets::store_password(&id.to_string(), &pw) {
                Ok(()) => {
                    self.session_passwords.remove(&id);
                }
                Err(_) => {
                    self.session_passwords.insert(id, pw);
                }
            }
        }
        Ok(())
    }

    fn resolve_password(&self, id: Uuid, override_pw: Option<String>) -> Result<String> {
        if let Some(pw) = override_pw {
            return Ok(pw);
        }
        if let Some(pw) = self.session_passwords.get(&id) {
            return Ok(pw.clone());
        }
        match secrets::load_password(&id.to_string())? {
            Some(pw) => Ok(pw),
            None => Ok(String::new()),
        }
    }

    fn find_profile(&self, id: Uuid) -> Result<ConnectionProfile> {
        self.store
            .get()
            .connections
            .iter()
            .find(|p| p.id == id)
            .cloned()
            .ok_or_else(|| EngineError::ProfileNotFound(id.to_string()))
    }

    fn cmd_connection_create(&mut self, params: CommandParams) -> Result<CommandResult> {
        let mut profile = params.profile.clone().unwrap_or_else(|| {
            ConnectionProfile::new(
                params
                    .name
                    .clone()
                    .unwrap_or_else(|| "New connection".into()),
                params.kind.unwrap_or_default(),
            )
        });
        if profile.id.is_nil() {
            profile.id = Uuid::new_v4();
        }
        apply_profile_fields(&mut profile, &params);
        profile.validate()?;
        let id = profile.id;
        self.remember_password(id, params.password)?;
        self.store.update(|s| {
            s.connections.push(profile);
        });
        self.set_status(format!("created connection {id}"));
        Ok(CommandResult::Ok {
            message: format!("created {id}"),
        })
    }

    fn cmd_connection_update(&mut self, params: CommandParams) -> Result<CommandResult> {
        let id = params
            .profile_id
            .or_else(|| params.profile.as_ref().map(|p| p.id))
            .ok_or_else(|| EngineError::Message("profile_id required".into()))?;
        self.remember_password(id, params.password.clone())?;
        let mut found = false;
        self.store.update(|s| {
            if let Some(p) = s.connections.iter_mut().find(|p| p.id == id) {
                if let Some(profile) = &params.profile {
                    *p = profile.clone();
                    p.id = id;
                }
                apply_profile_fields(p, &params);
                found = true;
            }
        });
        if !found {
            return Err(EngineError::ProfileNotFound(id.to_string()));
        }
        self.find_profile(id)?.validate()?;
        Ok(CommandResult::Ok {
            message: format!("updated {id}"),
        })
    }

    fn cmd_connection_delete(&mut self, params: CommandParams) -> Result<CommandResult> {
        let id = params
            .profile_id
            .ok_or_else(|| EngineError::Message("profile_id required".into()))?;
        let mut removed = false;
        self.store.update(|s| {
            let before = s.connections.len();
            s.connections.retain(|p| p.id != id);
            removed = s.connections.len() != before;
        });
        if !removed {
            return Err(EngineError::ProfileNotFound(id.to_string()));
        }
        let _ = secrets::delete_password(&id.to_string());
        self.session_passwords.remove(&id);
        if self.active_profile_id() == Some(id) {
            let _ = self.cmd_connection_disconnect();
        }
        Ok(CommandResult::Ok {
            message: format!("deleted {id}"),
        })
    }

    fn cmd_connection_test(&mut self, params: CommandParams) -> Result<CommandResult> {
        let profile = if let Some(p) = params.profile.clone() {
            p
        } else {
            let id = params
                .profile_id
                .ok_or_else(|| EngineError::Message("profile_id required".into()))?;
            self.find_profile(id)?
        };
        let password = self.resolve_password(profile.id, params.password)?;
        self.runtime
            .block_on(dbcraft_driver::test_connection(&profile, &password))?;
        self.set_status("connection ok");
        Ok(CommandResult::Ok {
            message: "connection ok".into(),
        })
    }

    fn cmd_connection_connect(&mut self, params: CommandParams) -> Result<CommandResult> {
        let id = params
            .profile_id
            .ok_or_else(|| EngineError::Message("profile_id required".into()))?;
        let password = self.resolve_password(id, params.password)?;
        let profile = self.find_profile(id)?;
        if let Some(old) = self.live.take() {
            self.runtime.block_on(old.conn.close());
        }
        let conn = self
            .runtime
            .block_on(dbcraft_driver::connect(&profile, &password))?;
        self.live = Some(LiveConnection {
            profile_id: id,
            conn,
        });
        self.store.update(|s| {
            s.ui.last_connection_id = Some(id);
        });
        self.set_status(format!("connected to {}", profile.name));
        let _ = self.refresh_schema_blocking();
        Ok(CommandResult::Ok {
            message: format!("connected {id}"),
        })
    }

    fn cmd_connection_disconnect(&mut self) -> Result<CommandResult> {
        if let Some(old) = self.live.take() {
            self.runtime.block_on(old.conn.close());
            self.schema_tree.clear();
            self.last_objects = None;
            self.set_status("disconnected");
            Ok(CommandResult::Ok {
                message: "disconnected".into(),
            })
        } else {
            Ok(CommandResult::Ok {
                message: "not connected".into(),
            })
        }
    }

    fn cmd_schema_refresh(&mut self) -> Result<CommandResult> {
        self.refresh_schema_blocking()?;
        Ok(CommandResult::Ok {
            message: "schema refreshed".into(),
        })
    }

    fn refresh_schema_blocking(&mut self) -> Result<()> {
        let live = self.live.as_ref().ok_or(EngineError::NotConnected)?;
        let profile = self.find_profile(live.profile_id)?;
        let conn = &live.conn;
        // Only database names — tables/columns load on expand (schema.expand).
        let databases = self.runtime.block_on(dbcraft_driver::list_databases(conn))?;
        let children: Vec<SchemaNode> = databases
            .into_iter()
            .map(|db| SchemaNode::lazy(SchemaKind::Database, db.name))
            .collect();
        self.schema_tree = vec![SchemaNode::leaf(SchemaKind::Connection, profile.name)
            .with_children(children)];
        let db_count = self
            .schema_tree
            .first()
            .map(|n| n.children.len())
            .unwrap_or(0);
        self.set_status(format!("{db_count} database(s)"));
        Ok(())
    }

    fn cmd_schema_expand(&mut self, params: CommandParams) -> Result<CommandResult> {
        let database = params
            .database
            .ok_or_else(|| EngineError::Message("database required".into()))?;
        let live = self.live.as_ref().ok_or(EngineError::NotConnected)?;
        let conn = &live.conn;

        if let Some(table) = params.table.as_deref() {
            let cols = self.runtime.block_on(dbcraft_driver::list_table_columns(
                conn, &database, table,
            ))?;
            let updated = set_node_children(
                &mut self.schema_tree,
                &database,
                Some(table),
                cols,
            );
            if !updated {
                return Err(EngineError::Message(format!(
                    "table not found in schema tree: {database}.{table}"
                )));
            }
            self.set_status(format!("columns loaded for {database}.{table}"));
            Ok(CommandResult::Ok {
                message: "columns loaded".into(),
            })
        } else {
            let tables = self
                .runtime
                .block_on(dbcraft_driver::list_tables(conn, &database))?;
            let children: Vec<SchemaNode> = tables
                .into_iter()
                .map(|t| {
                    let kind = t.kind;
                    SchemaNode::lazy(kind, t.name)
                })
                .collect();
            let count = children.len();
            let updated = set_node_children(&mut self.schema_tree, &database, None, children);
            if !updated {
                return Err(EngineError::Message(format!(
                    "database not found in schema tree: {database}"
                )));
            }
            self.set_status(format!("{count} table(s) in {database}"));
            Ok(CommandResult::Ok {
                message: format!("{count} table(s)"),
            })
        }
    }

    fn cmd_database_open(&mut self, params: CommandParams) -> Result<CommandResult> {
        let database = params
            .database
            .ok_or_else(|| EngineError::Message("database required".into()))?;
        let live = self.live.as_ref().ok_or(EngineError::NotConnected)?;
        let objects = self
            .runtime
            .block_on(dbcraft_driver::list_objects(&live.conn, &database))?;
        let count = objects.len();
        self.last_objects = Some((database.clone(), objects));
        self.set_status(format!("{count} object(s) in {database}"));
        Ok(CommandResult::Ok {
            message: format!("opened {database}"),
        })
    }

    fn cmd_query_execute(&mut self, params: CommandParams) -> Result<CommandResult> {
        let sql = params
            .sql
            .ok_or_else(|| EngineError::Message("sql required".into()))?;
        let live = self.live.as_ref().ok_or(EngineError::NotConnected)?;
        let result = self
            .runtime
            .block_on(dbcraft_driver::execute_sql(&live.conn, &sql))?;
        let rows = result.row_count();
        let ms = result.elapsed_ms;
        self.last_result = Some(result);
        self.set_status(format!("{rows} row(s) in {ms} ms"));
        Ok(CommandResult::Ok {
            message: format!("{rows} row(s)"),
        })
    }

    fn cmd_table_preview(&mut self, params: CommandParams) -> Result<CommandResult> {
        let table = params
            .table
            .ok_or_else(|| EngineError::Message("table required".into()))?;
        let database = params.database.unwrap_or_default();
        let limit = params.limit.unwrap_or(100);
        let live = self.live.as_ref().ok_or(EngineError::NotConnected)?;
        let result = self.runtime.block_on(dbcraft_driver::preview_table(
            &live.conn,
            &database,
            &table,
            limit,
        ))?;
        let pks = self
            .runtime
            .block_on(dbcraft_driver::list_primary_keys(
                &live.conn, &database, &table,
            ))
            .unwrap_or_default();
        self.last_pk_columns = pks;
        let rows = result.row_count();
        self.last_result = Some(result);
        self.set_status(format!("preview {table}: {rows} row(s)"));
        Ok(CommandResult::Ok {
            message: format!("preview {rows} row(s)"),
        })
    }

    fn cmd_table_design_load(&mut self, params: CommandParams) -> Result<CommandResult> {
        let table = params
            .table
            .or(params.name)
            .ok_or_else(|| EngineError::Message("table required".into()))?;
        let database = params.database.unwrap_or_default();
        let live = self.live.as_ref().ok_or(EngineError::NotConnected)?;
        let cols = self.runtime.block_on(dbcraft_driver::list_column_design(
            &live.conn,
            &database,
            &table,
        ))?;
        let count = cols.len();
        self.last_column_design = Some(cols);
        self.set_status(format!("design loaded for {table}: {count} column(s)"));
        Ok(CommandResult::Ok {
            message: format!("{count} column(s)"),
        })
    }

    fn cmd_routines_list(&mut self, params: CommandParams) -> Result<CommandResult> {
        let database = params
            .database
            .ok_or_else(|| EngineError::Message("database required".into()))?;
        let live = self.live.as_ref().ok_or(EngineError::NotConnected)?;
        let routines = self
            .runtime
            .block_on(dbcraft_driver::list_routines(&live.conn, &database))?;
        let count = routines.len();
        self.last_routines = Some((database.clone(), routines));
        self.set_status(format!("{count} routine(s) in {database}"));
        Ok(CommandResult::Ok {
            message: format!("{count} routine(s)"),
        })
    }

    fn cmd_users_list(&mut self) -> Result<CommandResult> {
        let live = self.live.as_ref().ok_or(EngineError::NotConnected)?;
        let users = self
            .runtime
            .block_on(dbcraft_driver::list_users(&live.conn))?;
        let count = users.len();
        self.last_users = Some(users);
        self.set_status(format!("{count} user(s)"));
        Ok(CommandResult::Ok {
            message: format!("{count} user(s)"),
        })
    }

    fn cmd_users_load(&mut self, params: CommandParams) -> Result<CommandResult> {
        let name = params
            .name
            .or(params.username)
            .ok_or_else(|| EngineError::Message("user name required".into()))?;
        let host = params
            .host
            .unwrap_or_else(|| "localhost".into());
        let live = self.live.as_ref().ok_or(EngineError::NotConnected)?;
        let draft = self
            .runtime
            .block_on(dbcraft_driver::load_user_detail(&live.conn, &name, &host))?;
        let account = draft.account();
        self.last_user_draft = Some(draft);
        self.set_status(format!("loaded {account}"));
        Ok(CommandResult::Ok {
            message: format!("loaded {account}"),
        })
    }

    fn cmd_users_save(&mut self, params: CommandParams) -> Result<CommandResult> {
        let draft = params
            .user_draft
            .ok_or_else(|| EngineError::Message("user_draft required".into()))?;
        let live = self.live.as_ref().ok_or(EngineError::NotConnected)?;
        self.runtime
            .block_on(dbcraft_driver::save_user(&live.conn, &draft))?;
        let account = draft.account();
        self.last_user_draft = Some(draft);
        // Refresh list
        let users = self
            .runtime
            .block_on(dbcraft_driver::list_users(&live.conn))?;
        self.last_users = Some(users);
        self.set_status(format!("saved {account}"));
        Ok(CommandResult::Ok {
            message: format!("saved {account}"),
        })
    }

    fn cmd_users_drop(&mut self, params: CommandParams) -> Result<CommandResult> {
        let name = params
            .name
            .or(params.username)
            .ok_or_else(|| EngineError::Message("user name required".into()))?;
        let host = params
            .host
            .unwrap_or_else(|| "localhost".into());
        let live = self.live.as_ref().ok_or(EngineError::NotConnected)?;
        self.runtime
            .block_on(dbcraft_driver::drop_user(&live.conn, &name, &host))?;
        let users = self
            .runtime
            .block_on(dbcraft_driver::list_users(&live.conn))?;
        self.last_users = Some(users);
        self.set_status(format!("dropped {name}@{host}"));
        Ok(CommandResult::Ok {
            message: format!("dropped {name}@{host}"),
        })
    }

    fn cmd_object_definition(&mut self, params: CommandParams) -> Result<CommandResult> {
        let name = params
            .name
            .or(params.table)
            .ok_or_else(|| EngineError::Message("object name required".into()))?;
        let database = params.database.unwrap_or_default();
        let kind = params
            .object_kind
            .ok_or_else(|| EngineError::Message("object_kind required".into()))?;
        let live = self.live.as_ref().ok_or(EngineError::NotConnected)?;
        let def = self.runtime.block_on(dbcraft_driver::object_definition(
            &live.conn,
            &database,
            &name,
            kind,
        ))?;
        self.last_definition = Some(def);
        self.set_status(format!("definition loaded for {name}"));
        Ok(CommandResult::Ok {
            message: "definition loaded".into(),
        })
    }

    fn cmd_object_drop(&mut self, params: CommandParams) -> Result<CommandResult> {
        let name = params
            .name
            .or(params.table)
            .ok_or_else(|| EngineError::Message("object name required".into()))?;
        let database = params.database.unwrap_or_default();
        let kind = params
            .object_kind
            .ok_or_else(|| EngineError::Message("object_kind required".into()))?;
        let live = self.live.as_ref().ok_or(EngineError::NotConnected)?;
        self.runtime.block_on(dbcraft_driver::drop_object(
            &live.conn,
            &database,
            &name,
            kind,
        ))?;
        // Refresh objects if we have a database context
        if !database.is_empty() {
            let objects = self
                .runtime
                .block_on(dbcraft_driver::list_objects(&live.conn, &database))?;
            self.last_objects = Some((database.clone(), objects));
            let routines = self
                .runtime
                .block_on(dbcraft_driver::list_routines(&live.conn, &database))?;
            self.last_routines = Some((database, routines));
        }
        self.set_status(format!("dropped {name}"));
        Ok(CommandResult::Ok {
            message: format!("dropped {name}"),
        })
    }

    fn apply_job_status(&mut self, status: JobStatus) {
        if let JobStatus::Done { message, .. } = &status {
            self.status_message = message.clone();
        }
        if let JobStatus::Failed { error, .. } = &status {
            self.status_message = error.clone();
        }
        let id = status.id();
        self.jobs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(id, status);
    }
}

/// Find a database (and optional table) under the connection root and replace its children.
fn set_node_children(
    tree: &mut [SchemaNode],
    database: &str,
    table: Option<&str>,
    children: Vec<SchemaNode>,
) -> bool {
    for root in tree.iter_mut() {
        for db in root.children.iter_mut() {
            if db.kind != SchemaKind::Database || db.name != database {
                continue;
            }
            if let Some(table_name) = table {
                for t in db.children.iter_mut() {
                    if matches!(t.kind, SchemaKind::Table | SchemaKind::View)
                        && t.name == table_name
                    {
                        t.children = children;
                        t.children_loaded = true;
                        return true;
                    }
                }
                return false;
            }
            db.children = children;
            db.children_loaded = true;
            return true;
        }
    }
    false
}

fn apply_profile_fields(profile: &mut ConnectionProfile, params: &CommandParams) {
    if let Some(name) = &params.name {
        profile.name = name.clone();
    }
    if let Some(kind) = params.kind {
        profile.kind = kind;
    }
    if let Some(host) = &params.host {
        profile.host = host.clone();
    }
    if let Some(port) = params.port {
        profile.port = port;
    }
    if let Some(username) = &params.username {
        profile.username = username.clone();
    }
    if let Some(database) = &params.database_name {
        profile.database = database.clone();
    }
    if let Some(file_path) = &params.file_path {
        profile.file_path = file_path.clone();
    }
    if let Some(ssl) = params.ssl {
        profile.ssl = ssl;
    }
}

// Keep job_tx referenced so the channel stays open for future async jobs.
#[allow(dead_code)]
fn _job_tx(session: &Session) -> &mpsc::UnboundedSender<JobStatus> {
    &session.job_tx
}

#[cfg(test)]
mod tests {
    use super::*;
    use dbcraft_core::ConnectionKind;

    #[test]
    fn create_and_delete_profile() {
        let mut session = Session::open_memory().expect("session");
        let res = session
            .dispatch(
                CommandId::ConnectionCreate,
                CommandParams {
                    name: Some("demo".into()),
                    kind: Some(ConnectionKind::Sqlite),
                    file_path: Some(":memory:".into()),
                    ..Default::default()
                },
            )
            .expect("create");
        assert!(matches!(res, CommandResult::Ok { .. }));
        assert_eq!(session.profiles().len(), 1);
        let id = session.profiles()[0].id;
        session
            .dispatch(
                CommandId::ConnectionDelete,
                CommandParams {
                    profile_id: Some(id),
                    ..Default::default()
                },
            )
            .expect("delete");
        assert!(session.profiles().is_empty());
    }

    #[test]
    fn unknown_command_errors() {
        let mut session = Session::open_memory().expect("session");
        let err = session
            .dispatch_str("nope", CommandParams::default())
            .expect_err("should fail");
        assert!(matches!(err, EngineError::UnknownCommand(_)));
    }

    #[test]
    fn query_without_connection_errors() {
        let mut session = Session::open_memory().expect("session");
        let err = session
            .dispatch(
                CommandId::QueryExecute,
                CommandParams {
                    sql: Some("SELECT 1".into()),
                    ..Default::default()
                },
            )
            .expect_err("not connected");
        assert!(matches!(err, EngineError::NotConnected));
    }

    #[test]
    fn sqlite_connect_query() {
        let mut session = Session::open_memory().expect("session");
        session
            .dispatch(
                CommandId::ConnectionCreate,
                CommandParams {
                    name: Some("mem".into()),
                    kind: Some(ConnectionKind::Sqlite),
                    file_path: Some(":memory:".into()),
                    ..Default::default()
                },
            )
            .expect("create");
        let id = session.profiles()[0].id;
        session
            .dispatch(
                CommandId::ConnectionConnect,
                CommandParams {
                    profile_id: Some(id),
                    ..Default::default()
                },
            )
            .expect("connect");
        session
            .dispatch(
                CommandId::QueryExecute,
                CommandParams {
                    sql: Some("SELECT 1 AS n".into()),
                    ..Default::default()
                },
            )
            .expect("query");
        let result = session.last_result().expect("result");
        assert_eq!(result.row_count(), 1);
    }

    #[test]
    fn schema_loads_lazily() {
        let mut session = Session::open_memory().expect("session");
        session
            .dispatch(
                CommandId::ConnectionCreate,
                CommandParams {
                    name: Some("mem".into()),
                    kind: Some(ConnectionKind::Sqlite),
                    file_path: Some(":memory:".into()),
                    ..Default::default()
                },
            )
            .expect("create");
        let id = session.profiles()[0].id;
        session
            .dispatch(
                CommandId::ConnectionConnect,
                CommandParams {
                    profile_id: Some(id),
                    ..Default::default()
                },
            )
            .expect("connect");
        session
            .dispatch(
                CommandId::QueryExecute,
                CommandParams {
                    sql: Some("CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT)".into()),
                    ..Default::default()
                },
            )
            .expect("create table");

        let root = &session.schema_tree()[0];
        assert_eq!(root.children.len(), 1);
        assert!(!root.children[0].children_loaded);

        session
            .dispatch(
                CommandId::SchemaExpand,
                CommandParams {
                    database: Some("main".into()),
                    ..Default::default()
                },
            )
            .expect("expand db");
        let db = &session.schema_tree()[0].children[0];
        assert!(db.children_loaded);
        assert_eq!(db.children.len(), 1);
        assert_eq!(db.children[0].name, "items");
        assert!(!db.children[0].children_loaded);

        session
            .dispatch(
                CommandId::SchemaExpand,
                CommandParams {
                    database: Some("main".into()),
                    table: Some("items".into()),
                    ..Default::default()
                },
            )
            .expect("expand table");
        let table = &session.schema_tree()[0].children[0].children[0];
        assert!(table.children_loaded);
        assert!(table.children.len() >= 2);
    }
}

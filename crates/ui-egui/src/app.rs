use craft_ui::layout::{SplitAxis, SplitSize, split_rect};
use dbcraft_core::{ConnectionKind, ConnectionProfile, SchemaKind, SslMode, UserDraft};
use dbcraft_engine::{CommandId, CommandParams, Session};
use egui::{Align, Color32, Layout, RichText, ScrollArea, Sense, TextEdit, Ui};
use uuid::Uuid;

use crate::grid_view::{
    draw_table_workspace as draw_table_grid, refresh_table_tab, save_table_edits,
};
use crate::icons::{self, glyph};
use crate::mysql_charset::{collations_for, default_collation, mysql_charsets};
use crate::table_design::{TableDesignTab, draw_table_design, save_table_design};
use crate::theme::{ThemeMode, Tokens};
use crate::user_view::{draw_user_editor, save_user_tab};
use crate::workspace::{
    ObjectCategory, ObjectsAction, Workspace, WorkspaceTab, draw_data_grid, draw_objects_view,
    draw_query_editor, draw_tab_bar, open_database_objects, run_query_tab,
};

pub struct DbCraftApp {
    session: Session,
    workspace: Workspace,
    selected_profile: Option<Uuid>,
    /// Force-expand this connection once after connect (Navicat-style).
    force_expand: Option<Uuid>,
    show_connection_dialog: bool,
    draft: ConnectionProfile,
    draft_password: String,
    show_new_database_dialog: bool,
    new_db_name: String,
    new_db_charset: String,
    new_db_collation: String,
    new_db_sql_tab: bool,
    error: String,
    tokens: Tokens,
    dark: bool,
}

impl DbCraftApp {
    pub fn new(session: Session) -> Self {
        let dark = session.settings().ui.theme != "light";
        let tokens = Tokens::for_mode(if dark {
            ThemeMode::Dark
        } else {
            ThemeMode::Light
        });
        let selected = session.settings().ui.last_connection_id;
        Self {
            session,
            workspace: Workspace::default(),
            selected_profile: selected,
            force_expand: None,
            show_connection_dialog: false,
            draft: ConnectionProfile::new("New connection", ConnectionKind::Mysql),
            draft_password: String::new(),
            show_new_database_dialog: false,
            new_db_name: String::new(),
            new_db_charset: "utf8mb4".into(),
            new_db_collation: String::new(),
            new_db_sql_tab: false,
            error: String::new(),
            tokens,
            dark,
        }
    }

    fn run_cmd(&mut self, id: CommandId, params: CommandParams) {
        match self.session.dispatch(id, params) {
            Ok(_) => self.error.clear(),
            Err(e) => {
                self.error = e.to_string();
                self.session.set_status(self.error.clone());
            }
        }
    }

    fn connect_profile(&mut self, id: Uuid) {
        self.selected_profile = Some(id);
        self.run_cmd(
            CommandId::ConnectionConnect,
            CommandParams {
                profile_id: Some(id),
                ..Default::default()
            },
        );
        if self.session.active_profile_id() == Some(id) {
            self.force_expand = Some(id);
            let label = self.connection_label();
            self.workspace.ensure_server_objects(label);
        }
    }

    fn open_database(&mut self, database: String) {
        let label = self
            .session
            .active_profile_id()
            .and_then(|id| {
                self.session
                    .profiles()
                    .iter()
                    .find(|p| p.id == id)
                    .map(|p| p.name.clone())
            })
            .unwrap_or_else(|| "Connection".into());

        match open_database_objects(&mut self.session, &database) {
            Ok((_, objects)) => {
                self.error.clear();
                self.workspace.open_objects(database, label, objects);
            }
            Err(e) => {
                self.error = e.clone();
                self.session.set_status(e);
            }
        }
    }

    fn open_table_data(&mut self, database: String, table: String) {
        let idx = self.workspace.open_table(database, table);
        if let Some(WorkspaceTab::Table(tab)) = self.workspace.tabs.get_mut(idx) {
            match refresh_table_tab(&mut self.session, tab) {
                Ok(()) => self.error.clear(),
                Err(e) => {
                    self.error = e.clone();
                    self.session.set_status(e);
                }
            }
        }
    }

    fn connection_label(&self) -> String {
        self.session
            .active_profile_id()
            .and_then(|id| {
                self.session
                    .profiles()
                    .iter()
                    .find(|p| p.id == id)
                    .map(|p| p.name.clone())
            })
            .unwrap_or_else(|| "Connection".into())
    }

    fn reload_objects_category(&mut self, category: ObjectCategory) {
        let active = self.workspace.active;
        let database = self
            .workspace
            .tabs
            .get(active)
            .and_then(|t| match t {
                WorkspaceTab::Objects(o) => Some(o.database.clone()),
                _ => None,
            })
            .unwrap_or_default();

        match category {
            ObjectCategory::Tables | ObjectCategory::Views => {
                if database.is_empty() {
                    self.error = "Open a database first".into();
                    return;
                }
                match open_database_objects(&mut self.session, &database) {
                    Ok((_, objects)) => {
                        if let Some(WorkspaceTab::Objects(tab)) =
                            self.workspace.tabs.get_mut(active)
                        {
                            tab.objects = objects;
                            tab.category = category;
                            tab.selected = None;
                        }
                        self.error.clear();
                    }
                    Err(e) => {
                        self.error = e;
                    }
                }
            }
            ObjectCategory::Functions => {
                if database.is_empty() {
                    self.error = "Open a database first".into();
                    return;
                }
                match self.session.dispatch(
                    CommandId::RoutinesList,
                    CommandParams {
                        database: Some(database.clone()),
                        ..Default::default()
                    },
                ) {
                    Ok(_) => {
                        let routines = self
                            .session
                            .last_routines()
                            .map(|(_, r)| r.clone())
                            .unwrap_or_default();
                        if let Some(WorkspaceTab::Objects(tab)) =
                            self.workspace.tabs.get_mut(active)
                        {
                            tab.routines = routines;
                            tab.category = category;
                            tab.selected = None;
                        }
                        self.error.clear();
                    }
                    Err(e) => self.error = e.to_string(),
                }
            }
            ObjectCategory::Users => match self.session.dispatch(
                CommandId::UsersList,
                CommandParams::default(),
            ) {
                Ok(_) => {
                    let users = self
                        .session
                        .last_users()
                        .map(|u| u.to_vec())
                        .unwrap_or_default();
                    if let Some(WorkspaceTab::Objects(tab)) = self.workspace.tabs.get_mut(active)
                    {
                        tab.users = users;
                        tab.category = category;
                        tab.selected = None;
                    }
                    self.error.clear();
                }
                Err(e) => self.error = e.to_string(),
            },
        }
    }

    fn open_definition(&mut self, database: String, name: String, kind: SchemaKind) {
        match self.session.dispatch(
            CommandId::ObjectDefinition,
            CommandParams {
                database: Some(database.clone()),
                name: Some(name.clone()),
                object_kind: Some(kind),
                ..Default::default()
            },
        ) {
            Ok(_) => {
                let sql = self
                    .session
                    .last_definition()
                    .unwrap_or("")
                    .to_string();
                self.workspace
                    .open_definition(database, name, kind, sql);
                self.error.clear();
            }
            Err(e) => {
                self.error = e.to_string();
                self.session.set_status(self.error.clone());
            }
        }
    }

    fn drop_object(&mut self, database: String, name: String, kind: SchemaKind) {
        match self.session.dispatch(
            CommandId::ObjectDrop,
            CommandParams {
                database: Some(database.clone()),
                name: Some(name),
                object_kind: Some(kind),
                ..Default::default()
            },
        ) {
            Ok(_) => {
                self.error.clear();
                let active = self.workspace.active;
                if let Some(WorkspaceTab::Objects(tab)) = self.workspace.tabs.get_mut(active) {
                    if let Some((_, objects)) = self.session.last_objects().cloned() {
                        tab.objects = objects;
                    }
                    if let Some((_, routines)) = self.session.last_routines().cloned() {
                        tab.routines = routines;
                    }
                    tab.selected = None;
                }
            }
            Err(e) => {
                self.error = e.to_string();
                self.session.set_status(self.error.clone());
            }
        }
    }

    fn open_user_editor(&mut self, name: String, host: String) {
        match self.session.dispatch(
            CommandId::UsersLoad,
            CommandParams {
                name: Some(name),
                host: Some(host),
                ..Default::default()
            },
        ) {
            Ok(_) => {
                if let Some(draft) = self.session.last_user_draft().cloned() {
                    let label = self.connection_label();
                    let databases: Vec<String> = self
                        .session
                        .schema_tree()
                        .first()
                        .map(|root| root.children.iter().map(|c| c.name.clone()).collect())
                        .unwrap_or_default();
                    let idx = self.workspace.open_user(label, draft);
                    if let Some(WorkspaceTab::User(tab)) = self.workspace.tabs.get_mut(idx) {
                        tab.add_priv_databases = databases;
                    }
                    self.error.clear();
                }
            }
            Err(e) => {
                self.error = e.to_string();
                self.session.set_status(self.error.clone());
            }
        }
    }

    fn open_new_user(&mut self) {
        let label = self.connection_label();
        let databases: Vec<String> = self
            .session
            .schema_tree()
            .first()
            .map(|root| root.children.iter().map(|c| c.name.clone()).collect())
            .unwrap_or_default();
        let idx = self.workspace.open_user(label, UserDraft::new_blank());
        if let Some(WorkspaceTab::User(tab)) = self.workspace.tabs.get_mut(idx) {
            tab.add_priv_databases = databases;
        }
    }

    fn drop_user(&mut self, name: String, host: String) {
        match self.session.dispatch(
            CommandId::UsersDrop,
            CommandParams {
                name: Some(name),
                host: Some(host),
                ..Default::default()
            },
        ) {
            Ok(_) => {
                let users = self
                    .session
                    .last_users()
                    .map(|u| u.to_vec())
                    .unwrap_or_default();
                let active = self.workspace.active;
                if let Some(WorkspaceTab::Objects(tab)) = self.workspace.tabs.get_mut(active) {
                    tab.users = users;
                    tab.selected = None;
                }
                self.error.clear();
            }
            Err(e) => {
                self.error = e.to_string();
                self.session.set_status(self.error.clone());
            }
        }
    }

    fn handle_objects_action(&mut self, action: ObjectsAction) {
        let database = self
            .workspace
            .tabs
            .get(self.workspace.active)
            .and_then(|t| match t {
                WorkspaceTab::Objects(o) => Some(o.database.clone()),
                _ => None,
            })
            .unwrap_or_default();

        match action {
            ObjectsAction::OpenTable(name) => self.open_table_data(database, name),
            ObjectsAction::DesignTable(name) => self.open_table_designer(database, name),
            ObjectsAction::OpenDefinition { name, kind } => {
                self.open_definition(database, name, kind);
            }
            ObjectsAction::DropObject { name, kind } => {
                self.drop_object(database, name, kind);
            }
            ObjectsAction::TruncateTable(name) => {
                self.truncate_table(database, name);
            }
            ObjectsAction::NewTable => {
                let label = self.connection_label();
                self.workspace.open_table_design(database, label);
            }
            ObjectsAction::Refresh => {
                let cat = self
                    .workspace
                    .tabs
                    .get(self.workspace.active)
                    .and_then(|t| match t {
                        WorkspaceTab::Objects(o) => Some(o.category),
                        _ => None,
                    })
                    .unwrap_or(ObjectCategory::Tables);
                self.reload_objects_category(cat);
            }
            ObjectsAction::EditUser { name, host } => self.open_user_editor(name, host),
            ObjectsAction::NewUser => self.open_new_user(),
            ObjectsAction::DropUser { name, host } => self.drop_user(name, host),
            ObjectsAction::CategoryChanged(cat) => self.reload_objects_category(cat),
        }
    }

    fn open_table_designer(&mut self, database: String, table: String) {
        match self.session.dispatch(
            CommandId::TableDesignLoad,
            CommandParams {
                database: Some(database.clone()),
                table: Some(table.clone()),
                ..Default::default()
            },
        ) {
            Ok(_) => {
                let cols = self
                    .session
                    .last_column_design()
                    .map(|c| c.to_vec())
                    .unwrap_or_default();
                let label = self.connection_label();
                let tab = TableDesignTab::from_existing(database, label, table, &cols);
                self.workspace.open_table_design_existing(tab);
                self.error.clear();
            }
            Err(e) => {
                self.error = e.to_string();
                self.session.set_status(self.error.clone());
            }
        }
    }

    fn truncate_table(&mut self, database: String, table: String) {
        let sql = if database.is_empty() {
            format!("TRUNCATE TABLE `{table}`")
        } else {
            format!("TRUNCATE TABLE `{database}`.`{table}`")
        };
        match self.session.dispatch(
            CommandId::QueryExecute,
            CommandParams {
                sql: Some(sql),
                ..Default::default()
            },
        ) {
            Ok(_) => {
                self.error.clear();
                self.session.set_status(format!("truncated `{table}`"));
                self.reload_objects_category(ObjectCategory::Tables);
            }
            Err(e) => {
                self.error = e.to_string();
                self.session.set_status(self.error.clone());
            }
        }
    }
}

impl eframe::App for DbCraftApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.session.tick();
        self.tokens.apply(ctx, self.dark);
        if self.session.quit_requested() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        egui::Panel::top("menu").show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui
                        .button(icons::label(glyph::ADD, "New connection…"))
                        .clicked()
                    {
                        self.draft =
                            ConnectionProfile::new("New connection", ConnectionKind::Mysql);
                        self.draft_password.clear();
                        self.show_connection_dialog = true;
                        ui.close();
                    }
                    if ui
                        .button(icons::label(glyph::QUERY, "New query tab"))
                        .clicked()
                    {
                        self.workspace.ensure_query_tab();
                        ui.close();
                    }
                    if ui
                        .button(icons::label(glyph::SAVE, "Save settings"))
                        .clicked()
                    {
                        self.run_cmd(CommandId::SettingsSave, CommandParams::default());
                        ui.close();
                    }
                    ui.separator();
                    if ui
                        .button(icons::label(glyph::CLOSE, "Quit"))
                        .clicked()
                    {
                        self.run_cmd(CommandId::AppQuit, CommandParams::default());
                        ui.close();
                    }
                });
                ui.menu_button("Connection", |ui| {
                    if ui
                        .button(icons::label(glyph::CONNECTED, "Connect"))
                        .clicked()
                    {
                        if let Some(id) = self.selected_profile {
                            self.connect_profile(id);
                        }
                        ui.close();
                    }
                    if ui
                        .button(icons::label(glyph::DISCONNECTED, "Disconnect"))
                        .clicked()
                    {
                        self.run_cmd(CommandId::ConnectionDisconnect, CommandParams::default());
                        ui.close();
                    }
                    if ui
                        .button(icons::label(glyph::REFRESH, "Refresh databases"))
                        .clicked()
                    {
                        self.run_cmd(CommandId::SchemaRefresh, CommandParams::default());
                        ui.close();
                    }
                });
                ui.menu_button("Query", |ui| {
                    if ui.button(icons::label(glyph::RUN, "Run")).clicked() {
                        self.workspace.ensure_query_tab();
                        if let Some(WorkspaceTab::Query(tab)) = self.workspace.active_mut() {
                            match run_query_tab(&mut self.session, tab) {
                                Ok(()) => self.error.clear(),
                                Err(e) => {
                                    self.error = e.clone();
                                    self.session.set_status(e);
                                }
                            }
                        }
                        ui.close();
                    }
                });
                ui.menu_button("View", |ui| {
                    if ui.checkbox(&mut self.dark, "Dark theme").changed() {
                        self.tokens = Tokens::for_mode(if self.dark {
                            ThemeMode::Dark
                        } else {
                            ThemeMode::Light
                        });
                        let theme = if self.dark { "dark" } else { "light" };
                        self.session.update_settings(|s| {
                            s.ui.theme = theme.into();
                        });
                    }
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(RichText::new("DBCraft").strong().color(self.tokens.accent));
                });
            });
        });

        egui::Panel::bottom("status")
            .exact_size(22.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if !self.error.is_empty() {
                        ui.colored_label(self.tokens.danger, &self.error);
                    } else {
                        ui.colored_label(self.tokens.muted, self.session.status_message());
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.colored_label(
                            self.tokens.muted,
                            format!("DBCraft V{}", env!("CARGO_PKG_VERSION")),
                        );
                    });
                });
            });

        egui::CentralPanel::default().show(ui, |ui| {
            let full = ui.available_rect_before_wrap();
            let sidebar_ratio = self.session.settings().ui.sidebar_ratio.clamp(0.12, 0.45);
            let Some(split) = split_rect(
                full,
                SplitAxis::Horizontal,
                SplitSize::Ratio(sidebar_ratio),
                4.0,
                120.0,
            ) else {
                return;
            };

            ui.scope_builder(
                egui::UiBuilder::new()
                    .id_salt("sidebar_pane")
                    .max_rect(split.first),
                |ui| {
                    ui.set_clip_rect(split.first);
                    self.draw_sidebar(ui);
                },
            );

            let divider = ui.interact(split.divider, ui.id().with("sidebar_div"), Sense::drag());
            if divider.dragged() {
                let delta = divider.drag_delta().x;
                let width = full.width().max(1.0);
                let new_ratio = (sidebar_ratio + delta / width).clamp(0.12, 0.45);
                self.session.update_settings(|s| {
                    s.ui.sidebar_ratio = new_ratio;
                });
            }
            ui.painter()
                .rect_filled(split.divider, 0.0, self.tokens.border);

            ui.scope_builder(
                egui::UiBuilder::new()
                    .id_salt("workspace_pane")
                    .max_rect(split.second),
                |ui| {
                    ui.set_clip_rect(split.second);
                    self.draw_workspace(ui);
                },
            );
        });

        if self.show_connection_dialog {
            self.draw_connection_dialog(&ctx);
        }
        if self.show_new_database_dialog {
            self.draw_new_database_dialog(&ctx);
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        let _ = self.session.flush();
    }
}

impl DbCraftApp {
    fn draw_sidebar(&mut self, ui: &mut Ui) {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.heading(icons::label(glyph::SERVER, "Connections"));
                if ui
                    .small_button(glyph::ADD)
                    .on_hover_text("Neue Verbindung")
                    .clicked()
                {
                    self.draft = ConnectionProfile::new("New connection", ConnectionKind::Mysql);
                    self.draft_password.clear();
                    self.show_connection_dialog = true;
                }
            });
            ui.colored_label(
                self.tokens.muted,
                "Doppelklick: verbinden › DB › Tabelle",
            );
            ui.separator();

            ScrollArea::vertical()
                .id_salt("sidebar_scroll")
                .show(ui, |ui| {
                    let profiles: Vec<ConnectionProfile> = self.session.profiles().to_vec();
                    let active_id = self.session.active_profile_id();
                    let databases: Vec<String> = self
                        .session
                        .schema_tree()
                        .first()
                        .map(|root| {
                            root.children
                                .iter()
                                .map(|c| c.name.clone())
                                .collect()
                        })
                        .unwrap_or_default();

                    let force = self.force_expand;
                    for profile in profiles {
                        let connected = active_id == Some(profile.id);
                        let label = format!(
                            "{}  {} ({})",
                            if connected {
                                glyph::CONNECTED
                            } else {
                                glyph::DISCONNECTED
                            },
                            profile.name,
                            profile.kind.display_name()
                        );

                        let id_salt = format!("conn/{}", profile.id);
                        let open = if force == Some(profile.id) {
                            if self.force_expand == Some(profile.id) {
                                self.force_expand = None;
                            }
                            Some(true)
                        } else {
                            None
                        };
                        let response = egui::CollapsingHeader::new(&label)
                            .id_salt(&id_salt)
                            .default_open(connected)
                            .open(open)
                            .show(ui, |ui| {
                                if connected {
                                    if databases.is_empty() {
                                        ui.colored_label(self.tokens.muted, "No databases");
                                    }
                                    for db in &databases {
                                        let db_resp = ui.selectable_label(
                                            false,
                                            format!("{}  {db}", glyph::DATABASE),
                                        );
                                        if db_resp.double_clicked() {
                                            self.open_database(db.clone());
                                        }
                                        db_resp.context_menu(|ui| {
                                            if ui
                                                .button(icons::label(glyph::OPEN, "Öffnen"))
                                                .clicked()
                                            {
                                                self.open_database(db.clone());
                                                ui.close();
                                            }
                                            if ui
                                                .button(icons::label(glyph::QUERY, "Neue Abfrage"))
                                                .clicked()
                                            {
                                                if !connected {
                                                    self.connect_profile(profile.id);
                                                }
                                                let idx = self.workspace.ensure_query_tab();
                                                if let Some(WorkspaceTab::Query(tab)) =
                                                    self.workspace.tabs.get_mut(idx)
                                                {
                                                    tab.sql = format!("USE `{db}`;\n");
                                                }
                                                ui.close();
                                            }
                                        });
                                    }
                                } else {
                                    ui.colored_label(
                                        self.tokens.muted,
                                        "Double-click connection to connect",
                                    );
                                }
                            });

                        if response.header_response.clicked() {
                            self.selected_profile = Some(profile.id);
                        }
                        if response.header_response.double_clicked() {
                            self.connect_profile(profile.id);
                        }
                        self.draw_server_context_menu(
                            &response.header_response,
                            &profile,
                            connected,
                        );
                    }
                });
        });
    }

    fn draw_server_context_menu(
        &mut self,
        header: &egui::Response,
        profile: &ConnectionProfile,
        connected: bool,
    ) {
        let id = profile.id;
        header.context_menu(|ui| {
            if connected {
                if ui
                    .button(icons::label(glyph::DISCONNECTED, "Verbindung schließen"))
                    .clicked()
                {
                    self.run_cmd(CommandId::ConnectionDisconnect, CommandParams::default());
                    ui.close();
                }
            } else if ui
                .button(icons::label(glyph::CONNECTED, "Verbinden"))
                .clicked()
            {
                self.connect_profile(id);
                ui.close();
            }
            ui.separator();
            if ui
                .button(icons::label(glyph::EDIT, "Verbindung bearbeiten…"))
                .clicked()
            {
                self.draft = profile.clone();
                self.draft_password.clear();
                self.show_connection_dialog = true;
                ui.close();
            }
            if ui
                .button(icons::label(glyph::ADD, "Neue Verbindung…"))
                .clicked()
            {
                self.draft = ConnectionProfile::new("New connection", ConnectionKind::Mysql);
                self.draft_password.clear();
                self.show_connection_dialog = true;
                ui.close();
            }
            if ui
                .button(icons::label(glyph::DELETE, "Verbindung löschen"))
                .clicked()
            {
                if connected {
                    self.run_cmd(CommandId::ConnectionDisconnect, CommandParams::default());
                }
                self.run_cmd(
                    CommandId::ConnectionDelete,
                    CommandParams {
                        profile_id: Some(id),
                        ..Default::default()
                    },
                );
                if self.selected_profile == Some(id) {
                    self.selected_profile = None;
                }
                ui.close();
            }
            if ui
                .button(icons::label(glyph::ADD, "Verbindung duplizieren…"))
                .clicked()
            {
                let mut copy = profile.clone();
                copy.id = Uuid::new_v4();
                copy.name = format!("{} (Kopie)", profile.name);
                self.draft = copy;
                self.draft_password.clear();
                self.show_connection_dialog = true;
                ui.close();
            }
            ui.separator();
            let can_create_db = !matches!(profile.kind, ConnectionKind::Sqlite);
            ui.add_enabled_ui(can_create_db, |ui| {
                if ui
                    .button(icons::label(glyph::DATABASE, "Neue Datenbank…"))
                    .on_hover_text(if can_create_db {
                        "CREATE DATABASE"
                    } else {
                        "Nicht für SQLite"
                    })
                    .clicked()
                {
                    if !connected {
                        self.connect_profile(id);
                    }
                    self.selected_profile = Some(id);
                    self.new_db_name.clear();
                    self.new_db_charset = "utf8mb4".into();
                    self.new_db_collation = default_collation("utf8mb4").into();
                    self.new_db_sql_tab = false;
                    self.show_new_database_dialog = true;
                    ui.close();
                }
            });
            if ui
                .button(icons::label(glyph::QUERY, "Neue Abfrage"))
                .clicked()
            {
                if !connected {
                    self.connect_profile(id);
                }
                self.workspace.ensure_query_tab();
                ui.close();
            }
            ui.separator();
            if ui
                .button(icons::label(glyph::REFRESH, "Aktualisieren"))
                .clicked()
            {
                if !connected {
                    self.connect_profile(id);
                } else {
                    self.run_cmd(CommandId::SchemaRefresh, CommandParams::default());
                }
                ui.close();
            }
        });
    }

    fn new_database_sql(&self) -> String {
        let name = self.new_db_name.trim();
        let kind = self
            .selected_profile
            .and_then(|id| {
                self.session
                    .profiles()
                    .iter()
                    .find(|p| p.id == id)
                    .map(|p| p.kind)
            })
            .unwrap_or(ConnectionKind::Mysql);
        match kind {
            ConnectionKind::Mysql | ConnectionKind::Mariadb => {
                let mut sql = format!("CREATE DATABASE `{name}`");
                if !self.new_db_charset.trim().is_empty() {
                    sql.push_str(&format!(
                        " CHARACTER SET {}",
                        self.new_db_charset.trim()
                    ));
                }
                if !self.new_db_collation.trim().is_empty() {
                    sql.push_str(&format!(" COLLATE {}", self.new_db_collation.trim()));
                }
                sql.push(';');
                sql
            }
            ConnectionKind::Postgres => {
                format!("CREATE DATABASE \"{name}\";")
            }
            ConnectionKind::Sqlite => {
                "-- SQLite hat keine CREATE DATABASE; lege eine neue Datei-Verbindung an.".into()
            }
        }
    }

    fn create_database_from_dialog(&mut self) {
        let name = self.new_db_name.trim().to_string();
        if name.is_empty() {
            self.error = "Datenbankname erforderlich".into();
            return;
        }
        let kind = self
            .selected_profile
            .and_then(|id| {
                self.session
                    .profiles()
                    .iter()
                    .find(|p| p.id == id)
                    .map(|p| p.kind)
            })
            .unwrap_or(ConnectionKind::Mysql);
        if kind == ConnectionKind::Sqlite {
            self.error = "SQLite unterstützt keine CREATE DATABASE".into();
            return;
        }
        if self.session.active_profile_id() != self.selected_profile {
            if let Some(id) = self.selected_profile {
                self.connect_profile(id);
            }
        }
        let sql = self.new_database_sql();
        match self.session.dispatch(
            CommandId::QueryExecute,
            CommandParams {
                sql: Some(sql),
                ..Default::default()
            },
        ) {
            Ok(_) => {
                self.run_cmd(CommandId::SchemaRefresh, CommandParams::default());
                self.show_new_database_dialog = false;
                self.error.clear();
                self.session
                    .set_status(format!("database `{name}` created"));
            }
            Err(e) => {
                self.error = e.to_string();
                self.session.set_status(self.error.clone());
            }
        }
    }

    fn draw_new_database_dialog(&mut self, ctx: &egui::Context) {
        let mut open = self.show_new_database_dialog;
        egui::Window::new("Neue Datenbank")
            .open(&mut open)
            .resizable(true)
            .default_width(400.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui
                        .selectable_label(!self.new_db_sql_tab, "Allgemein")
                        .clicked()
                    {
                        self.new_db_sql_tab = false;
                    }
                    if ui
                        .selectable_label(self.new_db_sql_tab, "SQL-Vorschau")
                        .clicked()
                    {
                        self.new_db_sql_tab = true;
                    }
                });
                ui.separator();

                if self.new_db_sql_tab {
                    let sql = self.new_database_sql();
                    ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
                        ui.label(RichText::new(&sql).monospace());
                    });
                } else {
                    egui::Grid::new("new_db_grid")
                        .num_columns(2)
                        .spacing([12.0, 8.0])
                        .show(ui, |ui| {
                            ui.label("Datenbankname");
                            ui.add(
                                TextEdit::singleline(&mut self.new_db_name).desired_width(240.0),
                            );
                            ui.end_row();

                            ui.label("Zeichensatz");
                            let charset_before = self.new_db_charset.clone();
                            egui::ComboBox::from_id_salt("new_db_charset")
                                .selected_text(&self.new_db_charset)
                                .width(240.0)
                                .height(280.0)
                                .show_ui(ui, |ui| {
                                    ScrollArea::vertical()
                                        .max_height(260.0)
                                        .show(ui, |ui| {
                                            for cs in mysql_charsets() {
                                                ui.selectable_value(
                                                    &mut self.new_db_charset,
                                                    (*cs).to_string(),
                                                    *cs,
                                                );
                                            }
                                        });
                                });
                            if self.new_db_charset != charset_before {
                                self.new_db_collation =
                                    default_collation(&self.new_db_charset).into();
                            }
                            ui.end_row();

                            ui.label("Sortierfolge");
                            let collations = collations_for(&self.new_db_charset);
                            let coll_label = if self.new_db_collation.is_empty() {
                                "(Server-Standard)"
                            } else {
                                self.new_db_collation.as_str()
                            };
                            egui::ComboBox::from_id_salt("new_db_collation")
                                .selected_text(coll_label)
                                .width(240.0)
                                .height(220.0)
                                .show_ui(ui, |ui| {
                                    if ui
                                        .selectable_label(
                                            self.new_db_collation.is_empty(),
                                            "(Server-Standard)",
                                        )
                                        .clicked()
                                    {
                                        self.new_db_collation.clear();
                                    }
                                    ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                                        for c in collations {
                                            ui.selectable_value(
                                                &mut self.new_db_collation,
                                                (*c).to_string(),
                                                *c,
                                            );
                                        }
                                    });
                                });
                            ui.end_row();
                        });
                }

                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("OK").clicked() {
                        self.create_database_from_dialog();
                    }
                    if ui.button("Abbrechen").clicked() {
                        self.show_new_database_dialog = false;
                    }
                });
            });
        if !open {
            self.show_new_database_dialog = false;
        }
    }

    fn draw_workspace(&mut self, ui: &mut Ui) {
        ui.vertical(|ui| {
            if let Some(close) = draw_tab_bar(ui, &mut self.workspace, &self.tokens) {
                self.workspace.close_tab(close);
            }
            ui.separator();

            if self.workspace.tabs.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.label(RichText::new("DBCraft").heading().color(self.tokens.accent));
                    ui.add_space(8.0);
                    ui.label("1. Double-click a connection to connect");
                    ui.label("2. Double-click a database to list tables");
                    ui.label("3. Double-click a table to open its data");
                });
                return;
            }

            let kind = self.workspace.tabs.get(self.workspace.active).map(|t| {
                match t {
                    WorkspaceTab::Objects(_) => 0,
                    WorkspaceTab::Query(_) => 1,
                    WorkspaceTab::Table(_) => 2,
                    WorkspaceTab::Definition(_) => 3,
                    WorkspaceTab::User(_) => 4,
                    WorkspaceTab::TableDesign(_) => 5,
                }
            });

            match kind {
                Some(0) => self.draw_objects_workspace(ui),
                Some(1) => self.draw_query_workspace(ui),
                Some(2) => self.draw_table_workspace(ui),
                Some(3) => self.draw_definition_workspace(ui),
                Some(4) => self.draw_user_workspace(ui),
                Some(5) => self.draw_table_design_workspace(ui),
                _ => {}
            }
        });
    }

    fn draw_objects_workspace(&mut self, ui: &mut Ui) {
        let active = self.workspace.active;
        let mut action = None;
        if let Some(WorkspaceTab::Objects(tab)) = self.workspace.tabs.get_mut(active) {
            action = draw_objects_view(ui, tab, &self.tokens);
        }
        if let Some(action) = action {
            self.handle_objects_action(action);
        }
    }

    fn draw_definition_workspace(&mut self, ui: &mut Ui) {
        let active = self.workspace.active;
        let mut save = false;
        if let Some(WorkspaceTab::Definition(tab)) = self.workspace.tabs.get_mut(active) {
            ui.horizontal(|ui| {
                ui.label(RichText::new(tab.title()).strong());
                if ui.button("Als Query ausführen / speichern").clicked() {
                    save = true;
                }
            });
            ui.separator();
            let editor = TextEdit::multiline(&mut tab.sql)
                .code_editor()
                .desired_width(f32::INFINITY);
            if ui
                .add_sized([ui.available_width(), ui.available_height().max(120.0)], editor)
                .changed()
            {
                tab.dirty = true;
            }
        }
        if save && let Some(WorkspaceTab::Definition(tab)) = self.workspace.tabs.get_mut(active)
        {
            match self.session.dispatch(
                CommandId::QueryExecute,
                CommandParams {
                    sql: Some(tab.sql.clone()),
                    ..Default::default()
                },
            ) {
                Ok(_) => {
                    tab.dirty = false;
                    self.error.clear();
                }
                Err(e) => {
                    self.error = e.to_string();
                    self.session.set_status(self.error.clone());
                }
            }
        }
    }

    fn draw_user_workspace(&mut self, ui: &mut Ui) {
        let active = self.workspace.active;
        let mut save = false;
        if let Some(WorkspaceTab::User(tab)) = self.workspace.tabs.get_mut(active) {
            draw_user_editor(ui, &self.tokens, tab, &mut || save = true);
        }
        if save && let Some(WorkspaceTab::User(tab)) = self.workspace.tabs.get_mut(active) {
            match save_user_tab(&mut self.session, tab) {
                Ok(()) => self.error.clear(),
                Err(e) => {
                    tab.message = e.clone();
                    self.error = e.clone();
                    self.session.set_status(e);
                }
            }
        }
    }

    fn draw_table_design_workspace(&mut self, ui: &mut Ui) {
        let active = self.workspace.active;
        let mut save = false;
        if let Some(WorkspaceTab::TableDesign(tab)) = self.workspace.tabs.get_mut(active) {
            draw_table_design(ui, &self.tokens, tab, &mut || save = true);
        }
        if save && let Some(WorkspaceTab::TableDesign(tab)) = self.workspace.tabs.get_mut(active)
        {
            match save_table_design(&mut self.session, tab) {
                Ok(()) => {
                    self.error.clear();
                    self.reload_objects_category(ObjectCategory::Tables);
                }
                Err(e) => {
                    tab.message = e.clone();
                    self.error = e.clone();
                    self.session.set_status(e);
                }
            }
        }
    }

    fn draw_query_workspace(&mut self, ui: &mut Ui) {
        let full = ui.available_rect_before_wrap();
        let editor_ratio = self.session.settings().ui.editor_ratio.clamp(0.2, 0.8);
        let Some(split) = split_rect(
            full,
            SplitAxis::Vertical,
            SplitSize::Ratio(editor_ratio),
            4.0,
            80.0,
        ) else {
            return;
        };

        ui.scope_builder(
            egui::UiBuilder::new()
                .id_salt("editor_pane")
                .max_rect(split.first),
            |ui| {
                ui.set_clip_rect(split.first);
                let active = self.workspace.active;
                if let Some(WorkspaceTab::Query(tab)) = self.workspace.tabs.get_mut(active) {
                    let mut run = false;
                    draw_query_editor(ui, tab, &mut || run = true);
                    if run {
                        match run_query_tab(&mut self.session, tab) {
                            Ok(()) => self.error.clear(),
                            Err(e) => {
                                self.error = e.clone();
                                self.session.set_status(e);
                            }
                        }
                    }
                }
            },
        );

        let divider = ui.interact(split.divider, ui.id().with("editor_div"), Sense::drag());
        if divider.dragged() {
            let delta = divider.drag_delta().y;
            let height = full.height().max(1.0);
            let new_ratio = (editor_ratio + delta / height).clamp(0.2, 0.8);
            self.session.update_settings(|s| {
                s.ui.editor_ratio = new_ratio;
            });
        }
        ui.painter()
            .rect_filled(split.divider, 0.0, self.tokens.border);

        ui.scope_builder(
            egui::UiBuilder::new()
                .id_salt("results_pane")
                .max_rect(split.second),
            |ui| {
                ui.set_clip_rect(split.second);
                ui.label(RichText::new("Results").strong());
                ui.separator();
                let active = self.workspace.active;
                if let Some(WorkspaceTab::Query(tab)) = self.workspace.tabs.get(active) {
                    if let Some(result) = &tab.result {
                        draw_data_grid(ui, &self.tokens, result, "query_result_scroll");
                    } else {
                        ui.colored_label(self.tokens.muted, "No results yet. Run a query.");
                    }
                }
            },
        );
    }

    fn draw_table_workspace(&mut self, ui: &mut Ui) {
        let active = self.workspace.active;
        let mut refresh = false;
        let mut save = false;
        if let Some(WorkspaceTab::Table(tab)) = self.workspace.tabs.get_mut(active) {
            draw_table_grid(ui, &self.tokens, tab, &mut || refresh = true, &mut || {
                save = true
            });
        }
        if refresh
            && let Some(WorkspaceTab::Table(tab)) = self.workspace.tabs.get_mut(active)
        {
            match refresh_table_tab(&mut self.session, tab) {
                Ok(()) => self.error.clear(),
                Err(e) => {
                    self.error = e.clone();
                    self.session.set_status(e);
                }
            }
        }
        if save
            && let Some(WorkspaceTab::Table(tab)) = self.workspace.tabs.get_mut(active)
        {
            match save_table_edits(&mut self.session, tab) {
                Ok(()) => self.error.clear(),
                Err(e) => {
                    tab.message = e.clone();
                    self.error = e.clone();
                    self.session.set_status(e);
                }
            }
        }
    }

    fn draw_connection_dialog(&mut self, ctx: &egui::Context) {
        let mut open = self.show_connection_dialog;
        egui::Window::new("Connection")
            .open(&mut open)
            .resizable(true)
            .default_width(420.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Name");
                    ui.text_edit_singleline(&mut self.draft.name);
                });
                egui::ComboBox::from_label("Type")
                    .selected_text(self.draft.kind.display_name())
                    .show_ui(ui, |ui| {
                        for kind in [
                            ConnectionKind::Mysql,
                            ConnectionKind::Mariadb,
                            ConnectionKind::Postgres,
                            ConnectionKind::Sqlite,
                        ] {
                            ui.selectable_value(&mut self.draft.kind, kind, kind.display_name());
                        }
                    });
                if self.draft.kind == ConnectionKind::Sqlite {
                    ui.horizontal(|ui| {
                        ui.label("File");
                        ui.text_edit_singleline(&mut self.draft.file_path);
                    });
                } else {
                    ui.horizontal(|ui| {
                        ui.label("Host");
                        ui.text_edit_singleline(&mut self.draft.host);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Port");
                        ui.add(egui::DragValue::new(&mut self.draft.port).range(1..=65535));
                    });
                    ui.horizontal(|ui| {
                        ui.label("User");
                        ui.text_edit_singleline(&mut self.draft.username);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Password");
                        ui.add(TextEdit::singleline(&mut self.draft_password).password(true));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Database");
                        ui.text_edit_singleline(&mut self.draft.database);
                    });
                    egui::ComboBox::from_label("SSL")
                        .selected_text(format!("{:?}", self.draft.ssl))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.draft.ssl, SslMode::Disable, "disable");
                            ui.selectable_value(&mut self.draft.ssl, SslMode::Prefer, "prefer");
                            ui.selectable_value(&mut self.draft.ssl, SslMode::Require, "require");
                        });
                }
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button(icons::label(glyph::CONNECTED, "Test")).clicked() {
                        let mut params = CommandParams {
                            profile: Some(self.draft.clone()),
                            password: Some(self.draft_password.clone()),
                            ..Default::default()
                        };
                        if self.draft.id.is_nil() {
                            self.draft.id = Uuid::new_v4();
                            params.profile = Some(self.draft.clone());
                        }
                        self.run_cmd(CommandId::ConnectionTest, params);
                    }
                    if ui.button(icons::label(glyph::SAVE, "Save")).clicked() {
                        let password = if self.draft_password.is_empty() {
                            None
                        } else {
                            Some(self.draft_password.clone())
                        };
                        if self
                            .session
                            .profiles()
                            .iter()
                            .any(|p| p.id == self.draft.id)
                        {
                            self.run_cmd(
                                CommandId::ConnectionUpdate,
                                CommandParams {
                                    profile_id: Some(self.draft.id),
                                    profile: Some(self.draft.clone()),
                                    password,
                                    ..Default::default()
                                },
                            );
                        } else {
                            if self.draft.id.is_nil() {
                                self.draft.id = Uuid::new_v4();
                            }
                            self.run_cmd(
                                CommandId::ConnectionCreate,
                                CommandParams {
                                    profile: Some(self.draft.clone()),
                                    password,
                                    ..Default::default()
                                },
                            );
                        }
                        self.selected_profile = Some(self.draft.id);
                        self.show_connection_dialog = false;
                    }
                    if ui
                        .button(icons::label(glyph::CLOSE, "Cancel"))
                        .clicked()
                    {
                        self.show_connection_dialog = false;
                    }
                });
            });
        self.show_connection_dialog = open;
    }
}

fn app_icon() -> Option<egui::IconData> {
    let png: &[u8] = include_bytes!(
        "../../../assets/app-icon/hicolor/256x256/apps/ai.storyteller.dbcraft.png"
    );
    eframe::icon_data::from_png_bytes(png)
        .map_err(|e| log::warn!("app icon: {e}"))
        .ok()
}

pub fn run_native() -> eframe::Result<()> {
    let session = Session::open().map_err(|e| {
        eframe::Error::AppCreation(Box::new(std::io::Error::other(e.to_string())))
    })?;
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("DBCraft")
        .with_app_id("ai.storyteller.dbcraft")
        .with_inner_size([1280.0, 800.0])
        .with_min_inner_size([800.0, 500.0]);
    if let Some(icon) = app_icon() {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "DBCraft",
        options,
        Box::new(|_cc| Ok(Box::new(DbCraftApp::new(session)))),
    )
}

#[allow(dead_code)]
fn _color(c: Color32) -> Color32 {
    c
}

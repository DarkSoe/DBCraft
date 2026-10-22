use std::collections::{HashMap, HashSet};

use dbcraft_core::{ObjectInfo, QueryResult, SchemaKind, UserDraft, UserInfo};
use dbcraft_engine::{CommandId, CommandParams, Session};
use egui::{RichText, ScrollArea, TextEdit, Ui};

use crate::icons::{self, glyph};
use crate::table_design::TableDesignTab;
use crate::theme::Tokens;

/// Navicat-style object category in the Objects main view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ObjectCategory {
    #[default]
    Tables,
    Views,
    Functions,
    Users,
}

impl ObjectCategory {
    pub fn label(self) -> &'static str {
        match self {
            Self::Tables => "Tabelle",
            Self::Views => "Ansicht",
            Self::Functions => "Funktion",
            Self::Users => "Benutzer",
        }
    }

    pub const ALL: [Self; 4] = [Self::Tables, Self::Views, Self::Functions, Self::Users];
}

/// Action requested from the Objects view.
#[derive(Debug, Clone)]
pub enum ObjectsAction {
    OpenTable(String),
    DesignTable(String),
    OpenDefinition { name: String, kind: SchemaKind },
    DropObject { name: String, kind: SchemaKind },
    TruncateTable(String),
    NewTable,
    Refresh,
    EditUser { name: String, host: String },
    NewUser,
    DropUser { name: String, host: String },
    CategoryChanged(ObjectCategory),
}

#[derive(Debug, Clone)]
pub struct QueryTab {
    pub sql: String,
    pub result: Option<QueryResult>,
}

impl Default for QueryTab {
    fn default() -> Self {
        Self {
            sql: "SELECT 1;\n".into(),
            result: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TableTab {
    pub database: String,
    pub table: String,
    pub limit: u32,
    pub result: Option<QueryResult>,
    pub pk_columns: Vec<String>,
    /// Edited cell values keyed by (row, column).
    pub edits: HashMap<(usize, usize), String>,
    pub dirty_rows: HashSet<usize>,
    /// Rows appended locally — saved as INSERT.
    pub new_rows: HashSet<usize>,
    /// Existing rows marked for deletion — saved as DELETE.
    pub pending_deletes: HashSet<usize>,
    pub selected_row: Option<usize>,
    pub editing: Option<(usize, usize)>,
    pub message: String,
}

impl TableTab {
    pub fn title(&self) -> String {
        if self.database.is_empty() {
            self.table.clone()
        } else {
            format!("{}@{}", self.table, self.database)
        }
    }
}

/// Navicat-style Objects main view: tables/views/functions/users.
#[derive(Debug, Clone)]
pub struct ObjectsTab {
    pub database: String,
    pub connection_label: String,
    pub category: ObjectCategory,
    pub objects: Vec<ObjectInfo>,
    pub routines: Vec<ObjectInfo>,
    pub users: Vec<UserInfo>,
    pub selected: Option<usize>,
}

impl ObjectsTab {
    pub fn title(&self) -> String {
        if self.database.is_empty() {
            format!("Objekte · {}", self.connection_label)
        } else {
            format!("Objekte · {}", self.database)
        }
    }

    pub fn filtered_objects(&self) -> Vec<(usize, &ObjectInfo)> {
        let kind_ok = |k: SchemaKind| match self.category {
            ObjectCategory::Tables => matches!(k, SchemaKind::Table),
            ObjectCategory::Views => matches!(k, SchemaKind::View),
            ObjectCategory::Functions => {
                matches!(k, SchemaKind::Function | SchemaKind::Procedure)
            }
            ObjectCategory::Users => false,
        };
        let source: &[ObjectInfo] = match self.category {
            ObjectCategory::Functions => &self.routines,
            _ => &self.objects,
        };
        source
            .iter()
            .enumerate()
            .filter(|(_, o)| kind_ok(o.kind))
            .collect()
    }
}

/// SQL definition editor for views / functions / procedures.
#[derive(Debug, Clone)]
pub struct DefinitionTab {
    pub database: String,
    pub name: String,
    pub kind: SchemaKind,
    pub sql: String,
    pub dirty: bool,
}

impl DefinitionTab {
    pub fn title(&self) -> String {
        let kind = match self.kind {
            SchemaKind::View => "Ansicht",
            SchemaKind::Function => "Funktion",
            SchemaKind::Procedure => "Prozedur",
            SchemaKind::Table => "Tabelle",
            _ => "Objekt",
        };
        format!("{} · {kind}", self.name)
    }
}

/// User editor tab (Navicat-style).
#[derive(Debug, Clone)]
pub struct UserTab {
    pub connection_label: String,
    pub draft: UserDraft,
    pub subtab: UserSubTab,
    pub message: String,
    pub add_priv_open: bool,
    pub add_priv_databases: Vec<String>,
    pub add_priv_selected: HashSet<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UserSubTab {
    #[default]
    General,
    ServerPrivs,
    Rights,
    SqlPreview,
}

impl UserTab {
    pub fn title(&self) -> String {
        if self.draft.is_existing {
            format!(
                "{} ({}) · Benutzer",
                self.draft.account(),
                self.connection_label
            )
        } else {
            format!("Ohne Titel ({}) · Benutzer", self.connection_label)
        }
    }
}

#[derive(Debug, Clone)]
pub enum WorkspaceTab {
    Objects(ObjectsTab),
    Query(QueryTab),
    Table(TableTab),
    Definition(DefinitionTab),
    User(UserTab),
    TableDesign(TableDesignTab),
}

impl WorkspaceTab {
    pub fn title(&self) -> String {
        match self {
            Self::Objects(t) => t.title(),
            Self::Query(_) => "Query".into(),
            Self::Table(t) => t.title(),
            Self::Definition(t) => t.title(),
            Self::User(t) => t.title(),
            Self::TableDesign(t) => t.title(),
        }
    }
}

#[derive(Default)]
pub struct Workspace {
    pub tabs: Vec<WorkspaceTab>,
    pub active: usize,
}

impl Workspace {
    /// One Objects tab per connection; updates when a database is opened.
    pub fn open_objects(
        &mut self,
        database: String,
        connection_label: String,
        objects: Vec<ObjectInfo>,
    ) -> usize {
        if let Some(idx) = self.tabs.iter().position(|t| {
            matches!(
                t,
                WorkspaceTab::Objects(o) if o.connection_label == connection_label
            )
        }) {
            if let Some(WorkspaceTab::Objects(tab)) = self.tabs.get_mut(idx) {
                tab.objects = objects;
                tab.connection_label = connection_label;
                tab.database = database;
                tab.selected = None;
                // Keep current category (Users stays server-wide; Tables/Views refresh for DB).
            }
            self.active = idx;
            return idx;
        }
        self.tabs.push(WorkspaceTab::Objects(ObjectsTab {
            database,
            connection_label,
            category: ObjectCategory::Tables,
            objects,
            routines: Vec::new(),
            users: Vec::new(),
            selected: None,
        }));
        self.active = self.tabs.len() - 1;
        self.active
    }

    /// Ensure an Objects tab exists right after connect (category bar visible).
    pub fn ensure_server_objects(&mut self, connection_label: String) -> usize {
        self.open_objects(String::new(), connection_label, Vec::new())
    }

    pub fn open_table(&mut self, database: String, table: String) -> usize {
        if let Some(idx) = self.tabs.iter().position(|t| {
            matches!(
                t,
                WorkspaceTab::Table(tt)
                    if tt.database == database && tt.table == table
            )
        }) {
            self.active = idx;
            return idx;
        }
        self.tabs.push(WorkspaceTab::Table(TableTab {
            database,
            table,
            limit: 1_000,
            result: None,
            pk_columns: Vec::new(),
            edits: HashMap::new(),
            dirty_rows: HashSet::new(),
            new_rows: HashSet::new(),
            pending_deletes: HashSet::new(),
            selected_row: None,
            editing: None,
            message: String::new(),
        }));
        self.active = self.tabs.len() - 1;
        self.active
    }

    pub fn ensure_query_tab(&mut self) -> usize {
        if let Some(idx) = self
            .tabs
            .iter()
            .position(|t| matches!(t, WorkspaceTab::Query(_)))
        {
            self.active = idx;
            return idx;
        }
        self.tabs.push(WorkspaceTab::Query(QueryTab::default()));
        self.active = self.tabs.len() - 1;
        self.active
    }

    pub fn open_definition(
        &mut self,
        database: String,
        name: String,
        kind: SchemaKind,
        sql: String,
    ) -> usize {
        if let Some(idx) = self.tabs.iter().position(|t| {
            matches!(
                t,
                WorkspaceTab::Definition(d)
                    if d.database == database && d.name == name && d.kind == kind
            )
        }) {
            if let Some(WorkspaceTab::Definition(tab)) = self.tabs.get_mut(idx) {
                tab.sql = sql;
                tab.dirty = false;
            }
            self.active = idx;
            return idx;
        }
        self.tabs.push(WorkspaceTab::Definition(DefinitionTab {
            database,
            name,
            kind,
            sql,
            dirty: false,
        }));
        self.active = self.tabs.len() - 1;
        self.active
    }

    pub fn open_table_design(
        &mut self,
        database: String,
        connection_label: String,
    ) -> usize {
        self.tabs.push(WorkspaceTab::TableDesign(TableDesignTab::new(
            database,
            connection_label,
        )));
        self.active = self.tabs.len() - 1;
        self.active
    }

    pub fn open_table_design_existing(&mut self, tab: TableDesignTab) -> usize {
        if let Some(idx) = self.tabs.iter().position(|t| {
            matches!(
                t,
                WorkspaceTab::TableDesign(d)
                    if d.is_existing
                        && d.database == tab.database
                        && d.table_name == tab.table_name
            )
        }) {
            if let Some(WorkspaceTab::TableDesign(existing)) = self.tabs.get_mut(idx) {
                *existing = tab;
            }
            self.active = idx;
            return idx;
        }
        self.tabs.push(WorkspaceTab::TableDesign(tab));
        self.active = self.tabs.len() - 1;
        self.active
    }

    pub fn open_user(&mut self, connection_label: String, draft: UserDraft) -> usize {
        let key_name = draft.original_name.clone();
        let key_host = draft.original_host.clone();
        if draft.is_existing
            && let Some(idx) = self.tabs.iter().position(|t| {
                matches!(
                    t,
                    WorkspaceTab::User(u)
                        if u.draft.original_name == key_name
                            && u.draft.original_host == key_host
                )
            })
        {
            if let Some(WorkspaceTab::User(tab)) = self.tabs.get_mut(idx) {
                tab.draft = draft;
                tab.message.clear();
            }
            self.active = idx;
            return idx;
        }
        self.tabs.push(WorkspaceTab::User(UserTab {
            connection_label,
            draft,
            subtab: UserSubTab::General,
            message: String::new(),
            add_priv_open: false,
            add_priv_databases: Vec::new(),
            add_priv_selected: HashSet::new(),
        }));
        self.active = self.tabs.len() - 1;
        self.active
    }

    pub fn close_tab(&mut self, idx: usize) {
        if idx >= self.tabs.len() {
            return;
        }
        self.tabs.remove(idx);
        if self.tabs.is_empty() {
            self.active = 0;
            return;
        }
        if self.active >= self.tabs.len() {
            self.active = self.tabs.len() - 1;
        }
    }

    pub fn active_mut(&mut self) -> Option<&mut WorkspaceTab> {
        self.tabs.get_mut(self.active)
    }
}

pub fn draw_data_grid(ui: &mut Ui, tokens: &Tokens, result: &QueryResult, scroll_id: &str) {
    ScrollArea::both()
        .id_salt(scroll_id)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            egui::Grid::new(scroll_id)
                .striped(true)
                .min_col_width(72.0)
                .show(ui, |ui| {
                    for col in &result.columns {
                        ui.label(RichText::new(&col.name).strong().color(tokens.accent));
                    }
                    ui.end_row();
                    for row in &result.rows {
                        for cell in &row.values {
                            ui.label(cell.display());
                        }
                        ui.end_row();
                    }
                });
        });
}

pub fn run_query_tab(session: &mut Session, tab: &mut QueryTab) -> Result<(), String> {
    session
        .dispatch(
            CommandId::QueryExecute,
            CommandParams {
                sql: Some(tab.sql.clone()),
                ..Default::default()
            },
        )
        .map_err(|e| e.to_string())?;
    tab.result = session.last_result().cloned();
    Ok(())
}

pub fn open_database_objects(
    session: &mut Session,
    database: &str,
) -> Result<(String, Vec<ObjectInfo>), String> {
    session
        .dispatch(
            CommandId::DatabaseOpen,
            CommandParams {
                database: Some(database.to_string()),
                ..Default::default()
            },
        )
        .map_err(|e| e.to_string())?;
    session
        .last_objects()
        .cloned()
        .ok_or_else(|| "no objects returned".into())
}

pub fn draw_query_editor(ui: &mut Ui, tab: &mut QueryTab, on_run: &mut dyn FnMut()) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(icons::label(glyph::SQL, "SQL")).strong());
        if ui.button(icons::label(glyph::RUN, "Run")).clicked() {
            on_run();
        }
    });
    let editor = TextEdit::multiline(&mut tab.sql)
        .code_editor()
        .desired_width(f32::INFINITY)
        .desired_rows(12);
    ui.add_sized(
        [ui.available_width(), ui.available_height().max(80.0)],
        editor,
    );
}

/// Draw Objects main view; returns requested action (if any).
pub fn draw_objects_view(
    ui: &mut Ui,
    tab: &mut ObjectsTab,
    tokens: &Tokens,
) -> Option<ObjectsAction> {
    let mut action = None;

    // Category toolbar (Navicat-style chips)
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        for cat in ObjectCategory::ALL {
            let icon = match cat {
                ObjectCategory::Tables => glyph::TABLE,
                ObjectCategory::Views => glyph::VIEW,
                ObjectCategory::Functions => glyph::FUNCTION,
                ObjectCategory::Users => glyph::USER,
            };
            let selected = tab.category == cat;
            if icons::category_chip(ui, selected, icon, cat.label(), tokens.accent, tokens.muted)
                .clicked()
                && tab.category != cat
            {
                tab.category = cat;
                tab.selected = None;
                action = Some(ObjectsAction::CategoryChanged(cat));
            }
        }
        ui.add_space(12.0);
        ui.vertical(|ui| {
            ui.add_space(8.0);
            let path = if tab.database.is_empty() {
                format!("{}  {} › (Server)", glyph::SERVER, tab.connection_label)
            } else {
                format!(
                    "{}  {} › {}",
                    glyph::DATABASE,
                    tab.connection_label,
                    tab.database
                )
            };
            ui.label(RichText::new(path).color(tokens.muted));
        });
    });
    ui.separator();

    // Action bar
    ui.horizontal(|ui| {
        match tab.category {
            ObjectCategory::Tables => {
                if ui
                    .button(icons::label(glyph::OPEN, "Öffnen"))
                    .clicked()
                {
                    if let Some(name) = selected_object_name(tab) {
                        action = Some(ObjectsAction::OpenTable(name));
                    }
                }
                if ui
                    .button(icons::label(glyph::DESIGN, "Entwerfen"))
                    .clicked()
                {
                    if let Some(name) = selected_object_name(tab) {
                        action = Some(ObjectsAction::DesignTable(name));
                    }
                }
                if ui
                    .button(icons::label(glyph::ADD, "Neue Tabelle"))
                    .clicked()
                {
                    action = Some(ObjectsAction::NewTable);
                }
                if ui
                    .button(icons::label(glyph::DELETE, "Löschen"))
                    .clicked()
                {
                    if let Some((name, kind)) = selected_object(tab) {
                        action = Some(ObjectsAction::DropObject { name, kind });
                    }
                }
                if ui
                    .button(icons::label(glyph::REFRESH, "Aktualisieren"))
                    .clicked()
                {
                    action = Some(ObjectsAction::Refresh);
                }
            }
            ObjectCategory::Views | ObjectCategory::Functions => {
                if ui
                    .button(icons::label(glyph::OPEN, "Öffnen"))
                    .clicked()
                {
                    if let Some((name, kind)) = selected_object(tab) {
                        action = Some(ObjectsAction::OpenDefinition { name, kind });
                    }
                }
                if ui
                    .button(icons::label(glyph::DELETE, "Löschen"))
                    .clicked()
                {
                    if let Some((name, kind)) = selected_object(tab) {
                        action = Some(ObjectsAction::DropObject { name, kind });
                    }
                }
            }
            ObjectCategory::Users => {
                if ui
                    .button(icons::label(glyph::EDIT, "Bearbeiten"))
                    .clicked()
                {
                    if let Some((name, host)) = selected_user(tab) {
                        action = Some(ObjectsAction::EditUser { name, host });
                    }
                }
                if ui
                    .button(icons::label(glyph::ADD, "Neuer Benutzer"))
                    .clicked()
                {
                    action = Some(ObjectsAction::NewUser);
                }
                if ui
                    .button(icons::label(glyph::DELETE, "Löschen"))
                    .clicked()
                {
                    if let Some((name, host)) = selected_user(tab) {
                        action = Some(ObjectsAction::DropUser { name, host });
                    }
                }
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let count = match tab.category {
                ObjectCategory::Users => tab.users.len(),
                _ => tab.filtered_objects().len(),
            };
            ui.colored_label(tokens.muted, format!("{count} Objekt(e)"));
        });
    });
    ui.separator();

    if tab.category == ObjectCategory::Users {
        draw_users_list(ui, tab, tokens, &mut action);
    } else if tab.database.is_empty() {
        ui.colored_label(
            tokens.muted,
            "Doppelklick auf eine Datenbank in der Sidebar, oder wechsle zu Benutzer.",
        );
    } else {
        draw_schema_objects_list(ui, tab, tokens, &mut action);
    }

    action
}

fn selected_object_name(tab: &ObjectsTab) -> Option<String> {
    selected_object(tab).map(|(n, _)| n)
}

fn selected_object(tab: &ObjectsTab) -> Option<(String, SchemaKind)> {
    let i = tab.selected?;
    let list = tab.filtered_objects();
    let (_, obj) = list.into_iter().find(|(idx, _)| *idx == i)?;
    Some((obj.name.clone(), obj.kind))
}

fn selected_user(tab: &ObjectsTab) -> Option<(String, String)> {
    let i = tab.selected?;
    let u = tab.users.get(i)?;
    Some((u.name.clone(), u.host.clone()))
}

fn draw_schema_objects_list(
    ui: &mut Ui,
    tab: &mut ObjectsTab,
    tokens: &Tokens,
    action: &mut Option<ObjectsAction>,
) {
    ScrollArea::both()
        .id_salt("objects_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            egui::Grid::new("objects_grid")
                .striped(true)
                .min_col_width(90.0)
                .show(ui, |ui| {
                    for h in [
                        "Name",
                        "Type",
                        "Rows",
                        "Data length",
                        "Engine",
                        "Updated",
                        "Comment",
                    ] {
                        ui.label(RichText::new(h).strong().color(tokens.accent));
                    }
                    ui.end_row();

                    let rows: Vec<(usize, ObjectInfo)> = tab
                        .filtered_objects()
                        .into_iter()
                        .map(|(i, o)| (i, o.clone()))
                        .collect();

                    for (idx, obj) in rows {
                        let selected = tab.selected == Some(idx);
                        let row_icon = match obj.kind {
                            SchemaKind::View => glyph::VIEW,
                            SchemaKind::Function | SchemaKind::Procedure => glyph::FUNCTION,
                            _ => glyph::TABLE,
                        };
                        let name_resp =
                            ui.selectable_label(selected, format!("{row_icon}  {}", obj.name));
                        if name_resp.clicked() {
                            tab.selected = Some(idx);
                        }
                        if name_resp.double_clicked() {
                            tab.selected = Some(idx);
                            match tab.category {
                                ObjectCategory::Tables => {
                                    *action = Some(ObjectsAction::OpenTable(obj.name.clone()));
                                }
                                ObjectCategory::Views | ObjectCategory::Functions => {
                                    *action = Some(ObjectsAction::OpenDefinition {
                                        name: obj.name.clone(),
                                        kind: obj.kind,
                                    });
                                }
                                ObjectCategory::Users => {}
                            }
                        }
                        name_resp.context_menu(|ui| {
                            tab.selected = Some(idx);
                            match tab.category {
                                ObjectCategory::Tables => {
                                    if ui
                                        .button(icons::label(glyph::OPEN, "Tabelle öffnen"))
                                        .clicked()
                                    {
                                        *action =
                                            Some(ObjectsAction::OpenTable(obj.name.clone()));
                                        ui.close();
                                    }
                                    if ui
                                        .button(icons::label(glyph::DESIGN, "Tabelle entwerfen"))
                                        .clicked()
                                    {
                                        *action =
                                            Some(ObjectsAction::DesignTable(obj.name.clone()));
                                        ui.close();
                                    }
                                    if ui
                                        .button(icons::label(glyph::ADD, "Neue Tabelle"))
                                        .clicked()
                                    {
                                        *action = Some(ObjectsAction::NewTable);
                                        ui.close();
                                    }
                                    if ui
                                        .button(icons::label(glyph::DELETE, "Tabelle löschen"))
                                        .clicked()
                                    {
                                        *action = Some(ObjectsAction::DropObject {
                                            name: obj.name.clone(),
                                            kind: obj.kind,
                                        });
                                        ui.close();
                                    }
                                    if ui
                                        .button(icons::label(glyph::REMOVE, "Tabelle leeren"))
                                        .clicked()
                                    {
                                        *action =
                                            Some(ObjectsAction::TruncateTable(obj.name.clone()));
                                        ui.close();
                                    }
                                    ui.separator();
                                    if ui
                                        .button(icons::label(glyph::REFRESH, "Aktualisieren"))
                                        .clicked()
                                    {
                                        *action = Some(ObjectsAction::Refresh);
                                        ui.close();
                                    }
                                }
                                ObjectCategory::Views | ObjectCategory::Functions => {
                                    if ui
                                        .button(icons::label(glyph::OPEN, "Öffnen"))
                                        .clicked()
                                    {
                                        *action = Some(ObjectsAction::OpenDefinition {
                                            name: obj.name.clone(),
                                            kind: obj.kind,
                                        });
                                        ui.close();
                                    }
                                    if ui
                                        .button(icons::label(glyph::DELETE, "Löschen"))
                                        .clicked()
                                    {
                                        *action = Some(ObjectsAction::DropObject {
                                            name: obj.name.clone(),
                                            kind: obj.kind,
                                        });
                                        ui.close();
                                    }
                                    ui.separator();
                                    if ui
                                        .button(icons::label(glyph::REFRESH, "Aktualisieren"))
                                        .clicked()
                                    {
                                        *action = Some(ObjectsAction::Refresh);
                                        ui.close();
                                    }
                                }
                                ObjectCategory::Users => {}
                            }
                        });

                        ui.label(obj.kind_label());
                        ui.label(
                            obj.row_estimate
                                .map(|n| n.to_string())
                                .unwrap_or_else(|| "—".into()),
                        );
                        ui.label(
                            obj.data_length
                                .map(format_bytes)
                                .unwrap_or_else(|| "—".into()),
                        );
                        ui.label(if obj.engine.is_empty() {
                            "—"
                        } else {
                            &obj.engine
                        });
                        ui.label(if obj.updated_at.is_empty() {
                            "—"
                        } else {
                            &obj.updated_at
                        });
                        ui.label(&obj.comment);
                        ui.end_row();
                    }
                });

            // Empty space below list — MainView right-click (Navicat)
            let fill = ui.available_size().max(egui::vec2(200.0, 120.0));
            let (_rect, empty) = ui.allocate_exact_size(fill, egui::Sense::click());
            empty.context_menu(|ui| {
                objects_empty_context_menu(ui, tab.category, action);
            });
        });
}

fn objects_empty_context_menu(
    ui: &mut Ui,
    category: ObjectCategory,
    action: &mut Option<ObjectsAction>,
) {
    match category {
        ObjectCategory::Tables => {
            if ui
                .button(icons::label(glyph::ADD, "Neue Tabelle"))
                .clicked()
            {
                *action = Some(ObjectsAction::NewTable);
                ui.close();
            }
            ui.separator();
            if ui
                .button(icons::label(glyph::REFRESH, "Aktualisieren"))
                .clicked()
            {
                *action = Some(ObjectsAction::Refresh);
                ui.close();
            }
        }
        ObjectCategory::Views | ObjectCategory::Functions => {
            if ui
                .button(icons::label(glyph::REFRESH, "Aktualisieren"))
                .clicked()
            {
                *action = Some(ObjectsAction::Refresh);
                ui.close();
            }
        }
        ObjectCategory::Users => {
            if ui
                .button(icons::label(glyph::ADD, "Neuer Benutzer"))
                .clicked()
            {
                *action = Some(ObjectsAction::NewUser);
                ui.close();
            }
            ui.separator();
            if ui
                .button(icons::label(glyph::REFRESH, "Aktualisieren"))
                .clicked()
            {
                *action = Some(ObjectsAction::Refresh);
                ui.close();
            }
        }
    }
}

fn draw_users_list(
    ui: &mut Ui,
    tab: &mut ObjectsTab,
    tokens: &Tokens,
    action: &mut Option<ObjectsAction>,
) {
    ScrollArea::both()
        .id_salt("users_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            egui::Grid::new("users_grid")
                .striped(true)
                .min_col_width(80.0)
                .show(ui, |ui| {
                    for h in [
                        "Name",
                        "SSL-Typ",
                        "Max. Abfragen",
                        "Max. Updates",
                        "Max. Verbindungen",
                        "Max. User-Verb.",
                        "Superuser",
                    ] {
                        ui.label(RichText::new(h).strong().color(tokens.accent));
                    }
                    ui.end_row();

                    let users = tab.users.clone();
                    for (idx, user) in users.iter().enumerate() {
                        let selected = tab.selected == Some(idx);
                        let label = format!("{}  {}", glyph::USER, user.account());
                        let name_resp = ui.selectable_label(selected, &label);
                        if name_resp.clicked() {
                            tab.selected = Some(idx);
                        }
                        if name_resp.double_clicked() {
                            tab.selected = Some(idx);
                            *action = Some(ObjectsAction::EditUser {
                                name: user.name.clone(),
                                host: user.host.clone(),
                            });
                        }
                        ui.label(if user.ssl_type.is_empty() {
                            "—"
                        } else {
                            &user.ssl_type
                        });
                        ui.label(user.max_questions.to_string());
                        ui.label(user.max_updates.to_string());
                        ui.label(user.max_connections.to_string());
                        ui.label(user.max_user_connections.to_string());
                        ui.label(if user.is_super { "Ja" } else { "Nein" });
                        ui.end_row();
                    }
                });
        });
}

fn format_bytes(n: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    let f = n as f64;
    if f >= MB {
        format!("{:.1} MB", f / MB)
    } else if f >= KB {
        format!("{:.1} KB", f / KB)
    } else {
        format!("{n} B")
    }
}

/// Tab strip; returns close request index.
pub fn draw_tab_bar(ui: &mut Ui, workspace: &mut Workspace, tokens: &Tokens) -> Option<usize> {
    let mut close_idx = None;
    ui.horizontal(|ui| {
        if workspace.tabs.is_empty() {
            ui.colored_label(
                tokens.muted,
                "Double-click a connection, then a database",
            );
        }
        let titles: Vec<String> = workspace
            .tabs
            .iter()
            .map(|t| {
                let icon = match t {
                    WorkspaceTab::Objects(_) => glyph::DATABASE,
                    WorkspaceTab::Query(_) => glyph::QUERY,
                    WorkspaceTab::Table(_) => glyph::TABLE,
                    WorkspaceTab::Definition(_) => glyph::SQL,
                    WorkspaceTab::User(_) => glyph::USER,
                    WorkspaceTab::TableDesign(_) => glyph::DESIGN,
                };
                format!("{icon}  {}", t.title())
            })
            .collect();
        for (i, title) in titles.iter().enumerate() {
            let selected = workspace.active == i;
            let response = ui.selectable_label(selected, title);
            if response.clicked() {
                workspace.active = i;
            }
            response.context_menu(|ui| {
                if ui.button("Close").clicked() {
                    close_idx = Some(i);
                    ui.close();
                }
            });
            if response.middle_clicked() {
                close_idx = Some(i);
            }
            let can_close = !matches!(workspace.tabs.get(i), Some(WorkspaceTab::Objects(_)))
                || workspace
                    .tabs
                    .iter()
                    .filter(|t| matches!(t, WorkspaceTab::Objects(_)))
                    .count()
                    > 0;
            if can_close
                && ui
                    .add(egui::Button::new("×").frame(false))
                    .on_hover_text("Close")
                    .clicked()
            {
                close_idx = Some(i);
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add(
                    egui::Button::new(icons::label(glyph::QUERY, "Query")).fill(tokens.panel),
                )
                .clicked()
            {
                workspace.ensure_query_tab();
            }
        });
    });
    close_idx
}

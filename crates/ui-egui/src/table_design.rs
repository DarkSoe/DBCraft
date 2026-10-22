//! Navicat-style "Neue Tabelle" designer tab.

use std::collections::HashSet;

use dbcraft_engine::{CommandId, CommandParams, Session};
use egui::{Key, RichText, ScrollArea, TextEdit, Ui};

use crate::icons::{self, glyph};
use crate::theme::Tokens;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesignField {
    pub name: String,
    /// Name as loaded from DB; empty = newly added column.
    pub orig_name: String,
    pub type_name: String,
    pub length: String,
    pub scale: String,
    pub not_null: bool,
    pub primary_key: bool,
    pub auto_increment: bool,
    pub comment: String,
}

impl DesignField {
    pub fn blank() -> Self {
        Self {
            name: String::new(),
            orig_name: String::new(),
            type_name: "INT".into(),
            length: String::new(),
            scale: String::new(),
            not_null: false,
            primary_key: false,
            auto_increment: false,
            comment: String::new(),
        }
    }

    pub fn id_pk() -> Self {
        Self {
            name: "id".into(),
            orig_name: String::new(),
            type_name: "INT".into(),
            length: "11".into(),
            scale: String::new(),
            not_null: true,
            primary_key: true,
            auto_increment: true,
            comment: String::new(),
        }
    }

}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DesignSubTab {
    #[default]
    Fields,
    SqlPreview,
}

#[derive(Debug, Clone)]
pub struct TableDesignTab {
    pub database: String,
    pub connection_label: String,
    pub table_name: String,
    pub fields: Vec<DesignField>,
    pub selected: Option<usize>,
    pub subtab: DesignSubTab,
    pub message: String,
    pub dirty: bool,
    /// True when opened via "Tabelle entwerfen" on an existing table.
    pub is_existing: bool,
    /// Snapshot at load time for ALTER diff.
    pub original_fields: Vec<DesignField>,
    /// Request keyboard focus on the Name cell of `selected` (after ↑/↓).
    pub focus_selected_name: bool,
}

impl TableDesignTab {
    pub fn new(database: String, connection_label: String) -> Self {
        Self {
            database,
            connection_label,
            table_name: String::new(),
            fields: vec![DesignField::id_pk()],
            selected: Some(0),
            subtab: DesignSubTab::Fields,
            message: String::new(),
            dirty: true,
            is_existing: false,
            original_fields: Vec::new(),
            focus_selected_name: true,
        }
    }

    pub fn from_existing(
        database: String,
        connection_label: String,
        table_name: String,
        cols: &[dbcraft_engine::ColumnDesignInfo],
    ) -> Self {
        let fields: Vec<DesignField> = cols
            .iter()
            .map(|c| DesignField {
                name: c.name.clone(),
                orig_name: c.name.clone(),
                type_name: c.data_type.clone(),
                length: c.length.map(|n| n.to_string()).unwrap_or_default(),
                scale: c.scale.map(|n| n.to_string()).unwrap_or_default(),
                not_null: c.not_null,
                primary_key: c.primary_key,
                auto_increment: c.auto_increment,
                comment: c.comment.clone(),
            })
            .collect();
        let selected = if fields.is_empty() { None } else { Some(0) };
        let original_fields = fields.clone();
        Self {
            database,
            connection_label,
            table_name,
            fields,
            selected,
            subtab: DesignSubTab::Fields,
            message: String::new(),
            dirty: false,
            is_existing: true,
            original_fields,
            focus_selected_name: true,
        }
    }

    pub fn title(&self) -> String {
        let name = if self.table_name.trim().is_empty() {
            "Ohne Titel"
        } else {
            self.table_name.trim()
        };
        if self.database.is_empty() {
            format!("{name} ({}) · Tabelle", self.connection_label)
        } else {
            format!(
                "{name} @{} ({}) · Tabelle",
                self.database, self.connection_label
            )
        }
    }

    pub fn build_create_sql(&self) -> Result<String, String> {
        let table = self.table_name.trim();
        if table.is_empty() {
            return Err("Tabellenname erforderlich".into());
        }
        if self.fields.is_empty() || self.fields.iter().all(|f| f.name.trim().is_empty()) {
            return Err("Mindestens ein Feld erforderlich".into());
        }

        let mut cols = Vec::new();
        let mut pks = Vec::new();
        for f in &self.fields {
            let name = f.name.trim();
            if name.is_empty() {
                continue;
            }
            let mut def = format!("`{name}` {}", format_type(f));
            if f.not_null || f.primary_key {
                def.push_str(" NOT NULL");
            }
            if f.auto_increment {
                def.push_str(" AUTO_INCREMENT");
            }
            if !f.comment.trim().is_empty() {
                def.push_str(&format!(
                    " COMMENT '{}'",
                    f.comment.replace('\'', "''")
                ));
            }
            cols.push(def);
            if f.primary_key {
                pks.push(format!("`{name}`"));
            }
        }
        if cols.is_empty() {
            return Err("Mindestens ein Feld erforderlich".into());
        }
        if !pks.is_empty() {
            cols.push(format!("PRIMARY KEY ({})", pks.join(", ")));
        }

        let table_ref = if self.database.is_empty() {
            format!("`{table}`")
        } else {
            format!("`{}`.`{table}`", self.database)
        };
        Ok(format!(
            "CREATE TABLE {table_ref} (\n  {}\n);",
            cols.join(",\n  ")
        ))
    }

    pub fn table_ref(&self) -> Result<String, String> {
        let table = self.table_name.trim();
        if table.is_empty() {
            return Err("Tabellenname erforderlich".into());
        }
        if self.database.is_empty() {
            Ok(format!("`{table}`"))
        } else {
            Ok(format!("`{}`.`{table}`", self.database))
        }
    }

    /// Build ALTER TABLE SQL for an existing table (diff vs `original_fields`).
    pub fn build_alter_sql(&self) -> Result<String, String> {
        let table_ref = self.table_ref()?;
        let current: Vec<&DesignField> = self
            .fields
            .iter()
            .filter(|f| !f.name.trim().is_empty())
            .collect();
        if current.is_empty() {
            return Err("Mindestens ein Feld erforderlich".into());
        }

        let mut clauses: Vec<String> = Vec::new();

        let current_orig_names: HashSet<&str> = current
            .iter()
            .map(|f| f.orig_name.trim())
            .filter(|n| !n.is_empty())
            .collect();

        // DROP removed columns
        for orig in &self.original_fields {
            let on = orig.orig_name.trim();
            if on.is_empty() {
                continue;
            }
            if !current_orig_names.contains(on) {
                clauses.push(format!("DROP COLUMN `{on}`"));
            }
        }

        // ADD / MODIFY / CHANGE
        for f in &current {
            let name = f.name.trim();
            let orig = f.orig_name.trim();
            if orig.is_empty() {
                clauses.push(format!("ADD COLUMN {}", column_clause(f, name)));
                continue;
            }
            let Some(old) = self
                .original_fields
                .iter()
                .find(|o| o.orig_name.trim() == orig)
            else {
                clauses.push(format!("ADD COLUMN {}", column_clause(f, name)));
                continue;
            };
            let renamed = name != orig;
            let def_changed = format_type(f) != format_type(old)
                || f.not_null != old.not_null
                || f.auto_increment != old.auto_increment
                || f.comment.trim() != old.comment.trim();
            if renamed {
                clauses.push(format!(
                    "CHANGE COLUMN `{orig}` {}",
                    column_clause(f, name)
                ));
            } else if def_changed {
                clauses.push(format!("MODIFY COLUMN {}", column_clause(f, name)));
            }
        }

        // Primary key changes
        let mut old_pks: Vec<String> = self
            .original_fields
            .iter()
            .filter(|f| f.primary_key)
            .map(|f| f.orig_name.trim().to_string())
            .filter(|n| !n.is_empty())
            .collect();
        old_pks.sort();
        let mut new_pks: Vec<String> = current
            .iter()
            .filter(|f| f.primary_key)
            .map(|f| f.name.trim().to_string())
            .collect();
        new_pks.sort();

        if old_pks != new_pks {
            if !old_pks.is_empty() {
                clauses.insert(0, "DROP PRIMARY KEY".into());
            }
            if !new_pks.is_empty() {
                let pk_list = new_pks
                    .iter()
                    .map(|n| format!("`{n}`"))
                    .collect::<Vec<_>>()
                    .join(", ");
                clauses.push(format!("ADD PRIMARY KEY ({pk_list})"));
            }
        }

        if clauses.is_empty() {
            return Err("Keine Änderungen".into());
        }

        Ok(format!(
            "ALTER TABLE {table_ref}\n  {};",
            clauses.join(",\n  ")
        ))
    }

    pub fn preview_sql(&self) -> String {
        if self.is_existing {
            self.build_alter_sql()
                .unwrap_or_else(|e| format!("-- {e}"))
        } else {
            self.build_create_sql()
                .unwrap_or_else(|e| format!("-- {e}"))
        }
    }

    fn sync_original_after_save(&mut self) {
        for f in &mut self.fields {
            let n = f.name.trim().to_string();
            f.orig_name = n;
        }
        self.fields.retain(|f| !f.name.trim().is_empty());
        self.original_fields = self.fields.clone();
        self.is_existing = true;
        self.dirty = false;
    }
}

fn column_clause(f: &DesignField, name: &str) -> String {
    let mut def = format!("`{name}` {}", format_type(f));
    if f.not_null || f.primary_key {
        def.push_str(" NOT NULL");
    } else {
        def.push_str(" NULL");
    }
    if f.auto_increment {
        def.push_str(" AUTO_INCREMENT");
    }
    if !f.comment.trim().is_empty() {
        def.push_str(&format!(
            " COMMENT '{}'",
            f.comment.replace('\'', "''")
        ));
    }
    def
}

fn format_type(f: &DesignField) -> String {
    let ty = f.type_name.trim().to_uppercase();
    let len = f.length.trim();
    let scale = f.scale.trim();
    match ty.as_str() {
        "VARCHAR" | "CHAR" | "VARBINARY" | "BINARY" => {
            if len.is_empty() {
                format!("{ty}(255)")
            } else {
                format!("{ty}({len})")
            }
        }
        "DECIMAL" | "NUMERIC" => {
            if len.is_empty() {
                "DECIMAL(10,2)".into()
            } else if scale.is_empty() {
                format!("{ty}({len})")
            } else {
                format!("{ty}({len},{scale})")
            }
        }
        "FLOAT" | "DOUBLE" | "INT" | "INTEGER" | "BIGINT" | "SMALLINT" | "TINYINT"
        | "MEDIUMINT" => {
            if len.is_empty() {
                ty
            } else {
                format!("{ty}({len})")
            }
        }
        _ => ty,
    }
}

const MYSQL_TYPES: &[&str] = &[
    "INT",
    "BIGINT",
    "SMALLINT",
    "TINYINT",
    "MEDIUMINT",
    "VARCHAR",
    "CHAR",
    "TEXT",
    "MEDIUMTEXT",
    "LONGTEXT",
    "DECIMAL",
    "FLOAT",
    "DOUBLE",
    "DATE",
    "DATETIME",
    "TIMESTAMP",
    "TIME",
    "YEAR",
    "BOOLEAN",
    "JSON",
    "BLOB",
    "LONGBLOB",
    "BINARY",
    "VARBINARY",
    "ENUM",
    "SET",
];

pub fn save_table_design(session: &mut Session, tab: &mut TableDesignTab) -> Result<(), String> {
    let sql = if tab.is_existing {
        tab.build_alter_sql()?
    } else {
        tab.build_create_sql()?
    };
    session
        .dispatch(
            CommandId::QueryExecute,
            CommandParams {
                sql: Some(sql),
                ..Default::default()
            },
        )
        .map_err(|e| e.to_string())?;
    let msg = if tab.is_existing {
        "Tabelle geändert (ALTER)"
    } else {
        "Tabelle erstellt"
    };
    tab.sync_original_after_save();
    tab.message = msg.into();
    Ok(())
}

pub fn draw_table_design(
    ui: &mut Ui,
    tokens: &Tokens,
    tab: &mut TableDesignTab,
    on_save: &mut dyn FnMut(),
) {
    ui.horizontal(|ui| {
        if ui
            .button(icons::label(glyph::SAVE, "Speichern"))
            .on_hover_text(if tab.is_existing {
                "ALTER TABLE aus Diff ausführen"
            } else {
                "CREATE TABLE ausführen"
            })
            .clicked()
        {
            on_save();
        }
        ui.separator();
        if ui
            .button(icons::label(glyph::ADD, "Feld hinzufügen"))
            .clicked()
        {
            tab.fields.push(DesignField::blank());
            tab.selected = Some(tab.fields.len() - 1);
            tab.dirty = true;
        }
        if ui
            .button(icons::label(glyph::ADD, "Feld einfügen"))
            .clicked()
        {
            let at = tab.selected.unwrap_or(0).min(tab.fields.len());
            tab.fields.insert(at, DesignField::blank());
            tab.selected = Some(at);
            tab.dirty = true;
        }
        if ui
            .button(icons::label(glyph::REMOVE, "Feld löschen"))
            .clicked()
        {
            if let Some(i) = tab.selected
                && i < tab.fields.len()
            {
                tab.fields.remove(i);
                tab.selected = if tab.fields.is_empty() {
                    None
                } else {
                    Some(i.min(tab.fields.len() - 1))
                };
                tab.dirty = true;
            }
        }
        if ui
            .button(icons::label(glyph::KEY, "Primärschlüssel"))
            .clicked()
        {
            if let Some(i) = tab.selected
                && let Some(f) = tab.fields.get_mut(i)
            {
                f.primary_key = !f.primary_key;
                if f.primary_key {
                    f.not_null = true;
                }
                tab.dirty = true;
            }
        }
        if ui.button(icons::label(glyph::UP, "Nach oben")).clicked() {
            if let Some(i) = tab.selected
                && i > 0
            {
                tab.fields.swap(i, i - 1);
                tab.selected = Some(i - 1);
                tab.dirty = true;
            }
        }
        if ui.button(icons::label(glyph::DOWN, "Nach unten")).clicked() {
            if let Some(i) = tab.selected
                && i + 1 < tab.fields.len()
            {
                tab.fields.swap(i, i + 1);
                tab.selected = Some(i + 1);
                tab.dirty = true;
            }
        }
        if !tab.message.is_empty() {
            ui.colored_label(tokens.muted, &tab.message);
        }
    });
    ui.separator();

    ui.horizontal(|ui| {
        ui.label("Tabellenname");
        if ui
            .add(TextEdit::singleline(&mut tab.table_name).desired_width(200.0))
            .changed()
        {
            tab.dirty = true;
        }
        ui.separator();
        for (sub, label) in [
            (DesignSubTab::Fields, "Felder"),
            (DesignSubTab::SqlPreview, "SQL-Vorschau"),
        ] {
            if ui.selectable_label(tab.subtab == sub, label).clicked() {
                tab.subtab = sub;
            }
        }
        ui.colored_label(tokens.muted, "Indizes · Fremdschlüssel · … (später)");
    });
    ui.separator();

    match tab.subtab {
        DesignSubTab::Fields => draw_fields_grid(ui, tokens, tab),
        DesignSubTab::SqlPreview => {
            let sql = tab.preview_sql();
            ScrollArea::both()
                .id_salt("table_design_sql")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.label(RichText::new(sql).monospace());
                });
        }
    }

    // Context menu on the design area
    ui.allocate_response(ui.available_size(), egui::Sense::click())
        .context_menu(|ui| {
            if ui
                .button(icons::label(glyph::ADD, "Feld hinzufügen"))
                .clicked()
            {
                tab.fields.push(DesignField::blank());
                tab.selected = Some(tab.fields.len() - 1);
                tab.dirty = true;
                ui.close();
            }
            if ui
                .button(icons::label(glyph::REMOVE, "Feld löschen"))
                .clicked()
            {
                if let Some(i) = tab.selected
                    && i < tab.fields.len()
                {
                    tab.fields.remove(i);
                    tab.dirty = true;
                }
                ui.close();
            }
            if ui
                .button(icons::label(glyph::KEY, "Primärschlüssel umschalten"))
                .clicked()
            {
                if let Some(i) = tab.selected
                    && let Some(f) = tab.fields.get_mut(i)
                {
                    f.primary_key = !f.primary_key;
                    tab.dirty = true;
                }
                ui.close();
            }
            ui.separator();
            if ui
                .button(icons::label(glyph::SAVE, "Speichern"))
                .clicked()
            {
                on_save();
                ui.close();
            }
        });
}

fn handle_arrow_row_nav(ui: &mut Ui, tab: &mut TableDesignTab) {
    let down = ui.input(|i| i.key_pressed(Key::ArrowDown));
    let up = ui.input(|i| i.key_pressed(Key::ArrowUp));
    if !down && !up {
        return;
    }

    if tab.fields.is_empty() {
        if down {
            tab.fields.push(DesignField::blank());
            tab.selected = Some(0);
            tab.dirty = true;
            tab.focus_selected_name = true;
        }
        return;
    }

    let cur = tab.selected.unwrap_or(0).min(tab.fields.len() - 1);
    let last = tab.fields.len() - 1;

    if down {
        if cur >= last {
            tab.fields.push(DesignField::blank());
            tab.selected = Some(tab.fields.len() - 1);
            tab.dirty = true;
        } else {
            tab.selected = Some(cur + 1);
        }
        tab.focus_selected_name = true;
        ui.ctx().request_repaint();
    } else if up && cur > 0 {
        tab.selected = Some(cur - 1);
        tab.focus_selected_name = true;
        ui.ctx().request_repaint();
    }
}

fn draw_fields_grid(ui: &mut Ui, tokens: &Tokens, tab: &mut TableDesignTab) {
    handle_arrow_row_nav(ui, tab);

    ScrollArea::both()
        .id_salt("table_design_fields")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            egui::Grid::new("design_fields_grid")
                .striped(true)
                .min_col_width(60.0)
                .show(ui, |ui| {
                    for h in [
                        "Name",
                        "Typ",
                        "Länge",
                        "Dezimal",
                        "Nicht NULL",
                        "AI",
                        "Schlüssel",
                        "Kommentar",
                    ] {
                        ui.label(RichText::new(h).strong().color(tokens.accent));
                    }
                    ui.end_row();

                    let mut dirty = false;
                    let want_focus = tab.focus_selected_name;
                    let n = tab.fields.len();
                    for i in 0..n {
                        let selected = tab.selected == Some(i);
                        let f = &mut tab.fields[i];
                        let name_id = ui.id().with(("design_field_name", i));

                        let name_resp = ui.add(
                            TextEdit::singleline(&mut f.name)
                                .id(name_id)
                                .desired_width(120.0)
                                .text_color(if selected {
                                    tokens.accent
                                } else {
                                    tokens.text
                                }),
                        );
                        if name_resp.clicked() || name_resp.gained_focus() {
                            tab.selected = Some(i);
                        }
                        if selected && want_focus {
                            name_resp.request_focus();
                            tab.focus_selected_name = false;
                        }
                        if name_resp.changed() {
                            dirty = true;
                        }
                        name_resp.context_menu(|ui| {
                            tab.selected = Some(i);
                            if ui.button("Feld löschen").clicked() {
                                // mark for delete after loop — use selected
                                ui.close();
                            }
                            if ui.button("Primärschlüssel").clicked() {
                                f.primary_key = !f.primary_key;
                                if f.primary_key {
                                    f.not_null = true;
                                }
                                dirty = true;
                                ui.close();
                            }
                        });

                        egui::ComboBox::from_id_salt(("field_type", i))
                            .selected_text(&f.type_name)
                            .width(100.0)
                            .show_ui(ui, |ui| {
                                for t in MYSQL_TYPES {
                                    if ui
                                        .selectable_value(&mut f.type_name, (*t).to_string(), *t)
                                        .changed()
                                    {
                                        dirty = true;
                                    }
                                }
                            });

                        if ui
                            .add(TextEdit::singleline(&mut f.length).desired_width(50.0))
                            .changed()
                        {
                            dirty = true;
                        }
                        if ui
                            .add(TextEdit::singleline(&mut f.scale).desired_width(40.0))
                            .changed()
                        {
                            dirty = true;
                        }
                        if ui.checkbox(&mut f.not_null, "").changed() {
                            dirty = true;
                        }
                        if ui.checkbox(&mut f.auto_increment, "").changed() {
                            dirty = true;
                        }
                        let key_label = if f.primary_key { "PK" } else { "" };
                        if ui.selectable_label(f.primary_key, key_label).clicked() {
                            f.primary_key = !f.primary_key;
                            if f.primary_key {
                                f.not_null = true;
                            }
                            dirty = true;
                            tab.selected = Some(i);
                        }
                        if ui
                            .add(TextEdit::singleline(&mut f.comment).desired_width(140.0))
                            .changed()
                        {
                            dirty = true;
                        }
                        ui.end_row();
                    }
                    if dirty {
                        tab.dirty = true;
                    }
                });

            // Extra blank area for right-click "Neue …"
            let (_rect, resp) =
                ui.allocate_exact_size(egui::vec2(ui.available_width(), 80.0), egui::Sense::click());
            resp.context_menu(|ui| {
                if ui.button("Feld hinzufügen").clicked() {
                    tab.fields.push(DesignField::blank());
                    tab.selected = Some(tab.fields.len() - 1);
                    tab.dirty = true;
                    ui.close();
                }
            });
        });
}

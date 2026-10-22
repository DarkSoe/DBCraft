//! Navicat-style user editor.

use std::collections::HashSet;

use dbcraft_core::{DbGrant, UserDraft, db_privilege_names, server_privilege_names};
use dbcraft_engine::{CommandId, CommandParams, Session};
use egui::{RichText, ScrollArea, TextEdit, Ui};

use crate::icons::{self, glyph};
use crate::theme::Tokens;
use crate::workspace::{UserSubTab, UserTab};

pub fn save_user_tab(session: &mut Session, tab: &mut UserTab) -> Result<(), String> {
    session
        .dispatch(
            CommandId::UsersSave,
            CommandParams {
                user_draft: Some(tab.draft.clone()),
                ..Default::default()
            },
        )
        .map_err(|e| e.to_string())?;
    if let Some(draft) = session.last_user_draft().cloned() {
        tab.draft = draft;
        tab.draft.is_existing = true;
        tab.draft.original_name = tab.draft.name.clone();
        tab.draft.original_host = tab.draft.host.clone();
        tab.draft.password.clear();
        tab.draft.password_repeat.clear();
    }
    tab.message = "Gespeichert".into();
    Ok(())
}

pub fn draw_user_editor(
    ui: &mut Ui,
    tokens: &Tokens,
    tab: &mut UserTab,
    on_save: &mut dyn FnMut(),
) {
    ui.horizontal(|ui| {
        if ui
            .button(icons::label(glyph::SAVE, "Speichern"))
            .clicked()
        {
            on_save();
        }
        if !tab.message.is_empty() {
            ui.colored_label(tokens.muted, &tab.message);
        }
    });
    ui.separator();

    ui.horizontal(|ui| {
        for (sub, label) in [
            (UserSubTab::General, "Allgemein"),
            (UserSubTab::ServerPrivs, "Server-Berechtigungen"),
            (UserSubTab::Rights, "Rechte"),
            (UserSubTab::SqlPreview, "SQL-Vorschau"),
        ] {
            let selected = tab.subtab == sub;
            if ui.selectable_label(selected, label).clicked() {
                tab.subtab = sub;
            }
        }
    });
    ui.separator();

    match tab.subtab {
        UserSubTab::General => draw_general(ui, &mut tab.draft),
        UserSubTab::ServerPrivs => draw_server_privs(ui, tokens, &mut tab.draft),
        UserSubTab::Rights => draw_rights(ui, tokens, tab),
        UserSubTab::SqlPreview => {
            let sql = dbcraft_engine::preview_user_save_sql(&tab.draft);
            ScrollArea::both()
                .id_salt("user_sql_preview")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.add(
                        egui::Label::new(RichText::new(&sql).monospace())
                            .selectable(true),
                    );
                });
        }
    }

    if tab.add_priv_open {
        draw_add_priv_dialog(ui, tokens, tab);
    }
}

fn draw_general(ui: &mut Ui, draft: &mut UserDraft) {
    egui::Grid::new("user_general")
        .num_columns(2)
        .spacing([12.0, 8.0])
        .show(ui, |ui| {
            ui.label("Benutzername");
            ui.add_enabled_ui(!draft.is_existing, |ui| {
                ui.add(
                    TextEdit::singleline(&mut draft.name).desired_width(280.0),
                );
            });
            ui.end_row();

            ui.label("Host");
            ui.add_enabled_ui(!draft.is_existing, |ui| {
                ui.add(
                    TextEdit::singleline(&mut draft.host).desired_width(280.0),
                );
            });
            ui.end_row();

            ui.label("Plugin");
            egui::ComboBox::from_id_salt("user_plugin")
                .selected_text(&draft.plugin)
                .width(280.0)
                .show_ui(ui, |ui| {
                    for p in [
                        "mysql_native_password",
                        "caching_sha2_password",
                        "unix_socket",
                        "ed25519",
                    ] {
                        ui.selectable_value(&mut draft.plugin, p.to_string(), p);
                    }
                });
            ui.end_row();

            ui.label("Kennwort");
            ui.add(
                TextEdit::singleline(&mut draft.password)
                    .password(true)
                    .desired_width(280.0),
            );
            ui.end_row();

            ui.label("Kennwort wiederholen");
            ui.add(
                TextEdit::singleline(&mut draft.password_repeat)
                    .password(true)
                    .desired_width(280.0),
            );
            ui.end_row();

            ui.label("Kennwort-Ablauf");
            egui::ComboBox::from_id_salt("user_pw_expire")
                .selected_text(&draft.password_expiration)
                .width(280.0)
                .show_ui(ui, |ui| {
                    for p in ["DEFAULT", "NEVER", "INTERVAL 90 DAY"] {
                        ui.selectable_value(
                            &mut draft.password_expiration,
                            p.to_string(),
                            p,
                        );
                    }
                });
            ui.end_row();
        });
}

fn draw_server_privs(ui: &mut Ui, tokens: &Tokens, draft: &mut UserDraft) {
    ui.colored_label(
        tokens.muted,
        "Globale Server-Berechtigungen (ON *.*)",
    );
    ScrollArea::vertical()
        .id_salt("server_privs_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            egui::Grid::new("server_privs_grid")
                .striped(true)
                .num_columns(2)
                .show(ui, |ui| {
                    ui.label(RichText::new("Privileg").strong().color(tokens.accent));
                    ui.label(RichText::new("Gewährt").strong().color(tokens.accent));
                    ui.end_row();
                    for priv_name in server_privilege_names() {
                        ui.label(*priv_name);
                        let mut on = draft.server_privs.contains(*priv_name);
                        if ui.checkbox(&mut on, "").changed() {
                            if on {
                                draft.server_privs.insert((*priv_name).to_string());
                            } else {
                                draft.server_privs.remove(*priv_name);
                            }
                        }
                        ui.end_row();
                    }
                });
        });
}

fn draw_rights(ui: &mut Ui, tokens: &Tokens, tab: &mut UserTab) {
    ui.horizontal(|ui| {
        if ui
            .button(icons::label(glyph::ADD, "Privileg hinzufügen"))
            .clicked()
        {
            tab.add_priv_open = true;
            if tab.add_priv_databases.is_empty() {
                // filled by app before open ideally; keep empty until set
            }
            tab.add_priv_selected.clear();
        }
        if ui
            .button(icons::label(glyph::REMOVE, "Privileg löschen"))
            .clicked()
        {
            // remove last grant for MVP simplicity if any selected — remove empty
            tab.draft.db_grants.retain(|g| !g.privileges.is_empty());
        }
    });

    let privs = db_privilege_names();
    ScrollArea::both()
        .id_salt("db_rights_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            egui::Grid::new("db_rights_grid")
                .striped(true)
                .min_col_width(70.0)
                .show(ui, |ui| {
                    ui.label(RichText::new("Datenbank").strong().color(tokens.accent));
                    for p in privs {
                        ui.label(RichText::new(*p).strong().color(tokens.accent));
                    }
                    ui.end_row();

                    let mut grants = tab.draft.db_grants.clone();
                    for grant in &mut grants {
                        ui.label(&grant.database);
                        for p in privs {
                            let mut on = grant.privileges.contains(*p);
                            if ui.checkbox(&mut on, "").changed() {
                                if on {
                                    grant.privileges.insert((*p).to_string());
                                } else {
                                    grant.privileges.remove(*p);
                                }
                            }
                        }
                        ui.end_row();
                    }
                    tab.draft.db_grants = grants;
                });
        });
}

fn draw_add_priv_dialog(ui: &mut Ui, tokens: &Tokens, tab: &mut UserTab) {
    egui::Window::new("Privileg hinzufügen")
        .collapsible(false)
        .resizable(true)
        .default_width(420.0)
        .show(ui.ctx(), |ui| {
            ui.colored_label(tokens.muted, "Datenbanken auswählen:");
            ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                for db in &tab.add_priv_databases {
                    let mut on = tab.add_priv_selected.contains(db);
                    if ui.checkbox(&mut on, db).changed() {
                        if on {
                            tab.add_priv_selected.insert(db.clone());
                        } else {
                            tab.add_priv_selected.remove(db);
                        }
                    }
                }
            });
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("OK").clicked() {
                    let default_privs: HashSet<String> = ["SELECT", "INSERT", "UPDATE", "DELETE"]
                        .into_iter()
                        .map(str::to_string)
                        .collect();
                    for db in tab.add_priv_selected.drain() {
                        if tab.draft.db_grants.iter().any(|g| g.database == db) {
                            continue;
                        }
                        tab.draft.db_grants.push(DbGrant {
                            database: db,
                            privileges: default_privs.clone(),
                        });
                    }
                    tab.add_priv_open = false;
                }
                if ui.button("Abbrechen").clicked() {
                    tab.add_priv_open = false;
                }
            });
        });
}

//! Navicat-style data grid: fill height, bottom toolbar, editable cells.

use std::collections::HashMap;

use dbcraft_core::{ConnectionKind, QueryResult, Row, SqlValue};
use dbcraft_engine::{CommandId, CommandParams, Session};
use egui::{
    Align, Frame, Key, Layout, Margin, RichText, ScrollArea, Sense, Stroke, TextEdit, Ui, Vec2,
};

use crate::icons::{self, glyph};
use crate::theme::Tokens;
use crate::workspace::TableTab;

pub fn refresh_table_tab(session: &mut Session, tab: &mut TableTab) -> Result<(), String> {
    session
        .dispatch(
            CommandId::TablePreview,
            CommandParams {
                database: Some(tab.database.clone()),
                table: Some(tab.table.clone()),
                limit: Some(tab.limit),
                ..Default::default()
            },
        )
        .map_err(|e| e.to_string())?;
    tab.result = session.last_result().cloned();
    tab.pk_columns = session.last_pk_columns().to_vec();
    clear_local_edits(tab);
    Ok(())
}

fn clear_local_edits(tab: &mut TableTab) {
    tab.edits.clear();
    tab.dirty_rows.clear();
    tab.new_rows.clear();
    tab.pending_deletes.clear();
    tab.selected_row = None;
    tab.editing = None;
    tab.message.clear();
}

pub fn discard_table_edits(tab: &mut TableTab) {
    if let Some(result) = tab.result.as_mut() {
        let keep: Vec<Row> = result
            .rows
            .iter()
            .enumerate()
            .filter(|(i, _)| !tab.new_rows.contains(i))
            .map(|(_, r)| r.clone())
            .collect();
        result.rows = keep;
    }
    clear_local_edits(tab);
    tab.message = "Discarded".into();
}

/// Append a blank row (Navicat + / ArrowDown on last row).
pub fn insert_blank_row(tab: &mut TableTab) {
    let Some(result) = tab.result.as_mut() else {
        tab.message = "Keine Daten geladen".into();
        return;
    };
    let ncols = result.columns.len();
    if ncols == 0 {
        tab.message = "Keine Spalten — kann keine Zeile einfügen".into();
        return;
    }
    let row_idx = result.rows.len();
    result.rows.push(Row {
        values: vec![SqlValue::Null; ncols],
    });
    tab.new_rows.insert(row_idx);
    tab.dirty_rows.insert(row_idx);
    tab.selected_row = Some(row_idx);
    // Prefer first non-PK column for editing; else first column.
    let edit_col = result
        .columns
        .iter()
        .enumerate()
        .find(|(_, c)| !tab.pk_columns.iter().any(|p| p == &c.name))
        .map(|(i, _)| i)
        .unwrap_or(0);
    tab.editing = Some((row_idx, edit_col));
    tab.edits
        .entry((row_idx, edit_col))
        .or_insert_with(|| "NULL".into());
    tab.message.clear();
}

/// Remove a never-saved new row, remapping higher indices.
fn remove_new_row(tab: &mut TableTab, idx: usize) {
    let Some(result) = tab.result.as_mut() else {
        return;
    };
    if idx >= result.rows.len() || !tab.new_rows.contains(&idx) {
        return;
    }
    result.rows.remove(idx);
    remap_after_remove(tab, idx);
}

fn remap_after_remove(tab: &mut TableTab, removed: usize) {
    let remap = |i: usize| -> Option<usize> {
        if i == removed {
            None
        } else if i > removed {
            Some(i - 1)
        } else {
            Some(i)
        }
    };
    tab.edits = tab
        .edits
        .drain()
        .filter_map(|((r, c), v)| remap(r).map(|nr| ((nr, c), v)))
        .collect();
    tab.dirty_rows = tab
        .dirty_rows
        .iter()
        .copied()
        .filter_map(remap)
        .collect();
    tab.new_rows = tab.new_rows.iter().copied().filter_map(remap).collect();
    tab.pending_deletes = tab
        .pending_deletes
        .iter()
        .copied()
        .filter_map(remap)
        .collect();
    tab.selected_row = tab.selected_row.and_then(remap);
    tab.editing = tab.editing.and_then(|(r, c)| remap(r).map(|nr| (nr, c)));
}

pub fn delete_selected_row(tab: &mut TableTab) {
    let Some(idx) = tab.selected_row else {
        tab.message = "Select a row to delete".into();
        return;
    };
    if tab.new_rows.contains(&idx) {
        remove_new_row(tab, idx);
        tab.message.clear();
        return;
    }
    if tab.pending_deletes.contains(&idx) {
        tab.pending_deletes.remove(&idx);
    } else {
        tab.pending_deletes.insert(idx);
        // Clear edits for a row marked delete
        tab.edits.retain(|(r, _), _| *r != idx);
        tab.dirty_rows.remove(&idx);
        if tab.editing.is_some_and(|(r, _)| r == idx) {
            tab.editing = None;
        }
    }
}

fn has_pending_changes(tab: &TableTab) -> bool {
    !tab.dirty_rows.is_empty() || !tab.pending_deletes.is_empty() || !tab.new_rows.is_empty()
}

pub fn save_table_edits(session: &mut Session, tab: &mut TableTab) -> Result<(), String> {
    let Some(result) = tab.result.as_ref() else {
        return Err("no data loaded".into());
    };
    if tab.pk_columns.is_empty()
        && (!tab.dirty_rows.is_empty() || !tab.pending_deletes.is_empty())
    {
        // Inserts without PK are ok; updates/deletes need PK.
        let needs_pk = tab
            .dirty_rows
            .iter()
            .any(|r| !tab.new_rows.contains(r))
            || !tab.pending_deletes.is_empty();
        if needs_pk {
            return Err("no primary key — cannot save updates/deletes safely".into());
        }
    }
    if !has_pending_changes(tab) {
        return Ok(());
    }

    let col_index: HashMap<&str, usize> = result
        .columns
        .iter()
        .enumerate()
        .map(|(i, c)| (c.name.as_str(), i))
        .collect();

    for pk in &tab.pk_columns {
        if !col_index.contains_key(pk.as_str()) {
            return Err(format!("primary key column `{pk}` not in result set"));
        }
    }

    let quote = ident_quote(session);
    let table_ref = qualified_table(quote, &tab.database, &tab.table);
    let mut statements = Vec::new();

    // DELETE first
    let deletes: Vec<usize> = tab.pending_deletes.iter().copied().collect();
    for row_idx in deletes {
        if tab.new_rows.contains(&row_idx) {
            continue;
        }
        let Some(row) = result.rows.get(row_idx) else {
            continue;
        };
        statements.push(build_delete(
            quote,
            &table_ref,
            &tab.pk_columns,
            &col_index,
            row,
        )?);
    }

    let dirty: Vec<usize> = tab.dirty_rows.iter().copied().collect();
    for row_idx in dirty {
        if tab.pending_deletes.contains(&row_idx) {
            continue;
        }
        let Some(row) = result.rows.get(row_idx) else {
            continue;
        };
        if tab.new_rows.contains(&row_idx) {
            if let Some(sql) = build_insert(quote, &table_ref, result, tab, row_idx, row) {
                statements.push(sql);
            }
        } else {
            if let Some(sql) =
                build_update(quote, &table_ref, result, tab, &col_index, row_idx, row)?
            {
                statements.push(sql);
            }
        }
    }

    if statements.is_empty() {
        // Only discarded new rows / toggled deletes with no SQL
        refresh_table_tab(session, tab)?;
        tab.message = "Saved".into();
        return Ok(());
    }

    let sql = statements.join(";\n");
    session
        .dispatch(
            CommandId::QueryExecute,
            CommandParams {
                sql: Some(sql),
                ..Default::default()
            },
        )
        .map_err(|e| e.to_string())?;

    refresh_table_tab(session, tab)?;
    tab.message = "Saved".into();
    Ok(())
}

fn build_delete(
    quote: &str,
    table_ref: &str,
    pk_columns: &[String],
    col_index: &HashMap<&str, usize>,
    row: &Row,
) -> Result<String, String> {
    let mut wheres = Vec::new();
    for pk in pk_columns {
        let Some(&ci) = col_index.get(pk.as_str()) else {
            continue;
        };
        let orig = row
            .values
            .get(ci)
            .map(SqlValue::edit_string)
            .unwrap_or_else(|| "NULL".into());
        if orig.eq_ignore_ascii_case("NULL") {
            wheres.push(format!("{q}{pk}{q} IS NULL", q = quote));
        } else {
            wheres.push(format!(
                "{q}{pk}{q} = {lit}",
                q = quote,
                lit = sql_literal(&orig)
            ));
        }
    }
    if wheres.is_empty() {
        return Err("could not build DELETE WHERE clause".into());
    }
    Ok(format!(
        "DELETE FROM {table_ref} WHERE {wheres}",
        wheres = wheres.join(" AND ")
    ))
}

fn cell_value(tab: &TableTab, row: &Row, row_idx: usize, col_idx: usize) -> String {
    tab.edits
        .get(&(row_idx, col_idx))
        .cloned()
        .unwrap_or_else(|| {
            row.values
                .get(col_idx)
                .map(SqlValue::edit_string)
                .unwrap_or_else(|| "NULL".into())
        })
}

fn build_insert(
    quote: &str,
    table_ref: &str,
    result: &QueryResult,
    tab: &TableTab,
    row_idx: usize,
    row: &Row,
) -> Option<String> {
    let mut cols = Vec::new();
    let mut vals = Vec::new();
    for (col_idx, col) in result.columns.iter().enumerate() {
        let val = cell_value(tab, row, row_idx, col_idx);
        let is_pk = tab.pk_columns.iter().any(|p| p == &col.name);
        // Skip NULL PK → let AUTO_INCREMENT / SERIAL fill it.
        if is_pk && val.eq_ignore_ascii_case("NULL") {
            continue;
        }
        cols.push(format!("{q}{name}{q}", q = quote, name = col.name));
        vals.push(sql_literal(&val));
    }
    if cols.is_empty() {
        return None;
    }
    Some(format!(
        "INSERT INTO {table_ref} ({cols}) VALUES ({vals})",
        cols = cols.join(", "),
        vals = vals.join(", ")
    ))
}

fn build_update(
    quote: &str,
    table_ref: &str,
    result: &QueryResult,
    tab: &TableTab,
    col_index: &HashMap<&str, usize>,
    row_idx: usize,
    row: &Row,
) -> Result<Option<String>, String> {
    let mut sets = Vec::new();
    for ((r, c), new_val) in &tab.edits {
        if *r != row_idx {
            continue;
        }
        let Some(col) = result.columns.get(*c) else {
            continue;
        };
        if tab.pk_columns.iter().any(|p| p == &col.name) {
            continue;
        }
        let original = row
            .values
            .get(*c)
            .map(SqlValue::edit_string)
            .unwrap_or_default();
        if *new_val == original {
            continue;
        }
        sets.push(format!(
            "{q}{name}{q} = {lit}",
            q = quote,
            name = col.name,
            lit = sql_literal(new_val)
        ));
    }
    if sets.is_empty() {
        return Ok(None);
    }
    let mut wheres = Vec::new();
    for pk in &tab.pk_columns {
        let Some(&ci) = col_index.get(pk.as_str()) else {
            continue;
        };
        let orig = row
            .values
            .get(ci)
            .map(SqlValue::edit_string)
            .unwrap_or_else(|| "NULL".into());
        if orig.eq_ignore_ascii_case("NULL") {
            wheres.push(format!("{q}{pk}{q} IS NULL", q = quote));
        } else {
            wheres.push(format!(
                "{q}{pk}{q} = {lit}",
                q = quote,
                lit = sql_literal(&orig)
            ));
        }
    }
    if wheres.is_empty() {
        return Err("could not build WHERE clause".into());
    }
    Ok(Some(format!(
        "UPDATE {table_ref} SET {sets} WHERE {wheres}",
        sets = sets.join(", "),
        wheres = wheres.join(" AND ")
    )))
}

fn ident_quote(session: &Session) -> &'static str {
    let kind = session
        .active_profile_id()
        .and_then(|id| session.profiles().iter().find(|p| p.id == id))
        .map(|p| p.kind);
    match kind {
        Some(ConnectionKind::Postgres) => "\"",
        _ => "`",
    }
}

fn qualified_table(q: &str, database: &str, table: &str) -> String {
    if database.is_empty() {
        format!("{q}{table}{q}")
    } else {
        format!("{q}{database}{q}.{q}{table}{q}")
    }
}

fn sql_literal(s: &str) -> String {
    if s.eq_ignore_ascii_case("NULL") {
        return "NULL".into();
    }
    if let Ok(n) = s.parse::<i64>() {
        return n.to_string();
    }
    if let Ok(n) = s.parse::<f64>()
        && s.contains('.')
    {
        return n.to_string();
    }
    format!("'{}'", s.replace('\'', "''"))
}

const TOOLBAR_H: f32 = 28.0;

/// Full table workspace: grid fills space; single bottom toolbar.
pub fn draw_table_workspace(
    ui: &mut Ui,
    tokens: &Tokens,
    tab: &mut TableTab,
    on_refresh: &mut dyn FnMut(),
    on_save: &mut dyn FnMut(),
) {
    let avail = ui.available_size();
    let grid_h = (avail.y - TOOLBAR_H).max(80.0);

    ui.allocate_ui_with_layout(
        Vec2::new(avail.x, grid_h),
        Layout::top_down(Align::Min),
        |ui| {
            ui.set_min_size(Vec2::new(avail.x, grid_h));
            Frame::new()
                .fill(tokens.panel)
                .stroke(Stroke::new(1.0, tokens.border))
                .show(ui, |ui| {
                    ui.set_min_size(ui.available_size());
                    if tab.result.is_some() {
                        draw_editable_grid(ui, tokens, tab);
                    } else {
                        ui.centered_and_justified(|ui| {
                            ui.colored_label(tokens.muted, "No data loaded.");
                        });
                    }
                });
        },
    );

    Frame::new()
        .fill(tokens.toolbar_bg)
        .inner_margin(Margin::symmetric(8, 3))
        .stroke(Stroke::new(1.0, tokens.border))
        .show(ui, |ui| {
            ui.set_min_height(TOOLBAR_H - 6.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new(tab.title()).strong());
                ui.separator();
                ui.label("Limit");
                ui.add(egui::DragValue::new(&mut tab.limit).range(1..=100_000));
                if ui
                    .button(icons::label(glyph::REFRESH, "Refresh"))
                    .clicked()
                {
                    on_refresh();
                }
                if ui
                    .button(RichText::new(glyph::ADD).strong())
                    .on_hover_text("Zeile einfügen")
                    .clicked()
                {
                    insert_blank_row(tab);
                }
                let can_delete = tab.selected_row.is_some();
                ui.add_enabled_ui(can_delete, |ui| {
                    if ui
                        .button(RichText::new(glyph::REMOVE).strong())
                        .on_hover_text("Zeile löschen")
                        .clicked()
                    {
                        delete_selected_row(tab);
                    }
                });
                let can_save = has_pending_changes(tab)
                    && (!tab.pk_columns.is_empty()
                        || tab.dirty_rows.iter().all(|r| tab.new_rows.contains(r)));
                ui.add_enabled_ui(can_save, |ui| {
                    if ui
                        .button(icons::label(glyph::SAVE, "Save"))
                        .clicked()
                    {
                        on_save();
                    }
                });
                if has_pending_changes(tab)
                    && ui.button(icons::label(glyph::CLOSE, "Discard")).clicked()
                {
                    discard_table_edits(tab);
                }

                // Status always on the far right
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if !tab.message.is_empty() {
                        ui.colored_label(tokens.muted, &tab.message);
                        ui.separator();
                    }
                    if !tab.pending_deletes.is_empty() {
                        ui.colored_label(
                            tokens.danger,
                            format!("{} to delete", tab.pending_deletes.len()),
                        );
                        ui.separator();
                    }
                    if !tab.new_rows.is_empty() {
                        ui.colored_label(
                            tokens.accent,
                            format!("{} new", tab.new_rows.len()),
                        );
                        ui.separator();
                    } else if !tab.dirty_rows.is_empty() {
                        ui.colored_label(
                            tokens.accent,
                            format!("{} edited", tab.dirty_rows.len()),
                        );
                        ui.separator();
                    }
                    if tab.pk_columns.is_empty() {
                        ui.colored_label(tokens.danger, "no PK");
                        ui.separator();
                    }
                    if let Some(result) = &tab.result {
                        if let Some(affected) = result.rows_affected {
                            ui.label(format!(
                                "{affected} row(s) affected · {} ms",
                                result.elapsed_ms
                            ));
                        } else {
                            let loaded = result.row_count() - tab.new_rows.len();
                            ui.label(format!("{loaded} row(s) · {} ms", result.elapsed_ms));
                        }
                    } else {
                        ui.colored_label(tokens.muted, "—");
                    }
                });
            });
        });
}

fn draw_editable_grid(ui: &mut Ui, tokens: &Tokens, tab: &mut TableTab) {
    let Some(result) = tab.result.clone() else {
        return;
    };
    let scroll_id = format!("table_scroll_{}_{}", tab.database, tab.table);
    let cell_w = 120.0;
    let cell_h = 22.0;
    let header_h = 24.0;

    // ArrowDown on last row → insert (Navicat)
    let last_idx = result.rows.len().saturating_sub(1);
    let on_last = result.rows.is_empty()
        || tab.selected_row == Some(last_idx)
        || tab.editing.is_some_and(|(r, _)| r == last_idx);
    if on_last && ui.input(|i| i.key_pressed(Key::ArrowDown)) {
        insert_blank_row(tab);
    }

    let result = tab.result.clone().unwrap_or(result);
    let row_count = result.rows.len();

    ScrollArea::both()
        .id_salt(&scroll_id)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let viewport = ui.available_size();
            let content_w = (result.columns.len() as f32 * cell_w).max(viewport.x);
            let content_h = (header_h + row_count as f32 * cell_h).max(viewport.y);
            ui.set_min_size(Vec2::new(content_w, content_h));

            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                for col in &result.columns {
                    header_cell(ui, tokens, &col.name, cell_w, header_h);
                }
            });

            if result.rows.is_empty() {
                let hint = ui.allocate_exact_size(
                    Vec2::new(content_w.max(240.0), 48.0),
                    Sense::click(),
                );
                ui.painter().with_clip_rect(hint.0).text(
                    hint.0.center(),
                    egui::Align2::CENTER_CENTER,
                    "Leere Tabelle — + oder ↓ für neue Zeile",
                    egui::FontId::proportional(13.0),
                    tokens.muted,
                );
                if hint.1.clicked() || hint.1.double_clicked() {
                    insert_blank_row(tab);
                }
                hint.1.context_menu(|ui| {
                    if ui
                        .button(icons::label(glyph::ADD, "Neue Zeile"))
                        .clicked()
                    {
                        insert_blank_row(tab);
                        ui.close();
                    }
                });
            }

            for (row_idx, row) in result.rows.iter().enumerate() {
                let stripe = row_idx % 2 == 1;
                let is_new = tab.new_rows.contains(&row_idx);
                let is_delete = tab.pending_deletes.contains(&row_idx);
                let is_selected = tab.selected_row == Some(row_idx);
                let mut row_bg = if stripe { tokens.stripe } else { tokens.panel };
                if is_new {
                    row_bg = tokens.cell_edit_bg;
                }
                if is_delete {
                    row_bg = tokens.danger.gamma_multiply(0.25);
                }
                if is_selected {
                    row_bg = tokens.accent.gamma_multiply(0.2);
                }

                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    for (col_idx, cell) in row.values.iter().enumerate() {
                        let key = (row_idx, col_idx);
                        let original = cell.edit_string();
                        let value = tab
                            .edits
                            .get(&key)
                            .cloned()
                            .unwrap_or_else(|| original.clone());
                        let is_pk = result
                            .columns
                            .get(col_idx)
                            .is_some_and(|c| tab.pk_columns.iter().any(|p| p == &c.name));
                        // New rows: PK editable (or leave NULL for auto-inc).
                        let pk_locked = is_pk && !is_new;
                        let changed = value != original;
                        let is_dirty = tab.dirty_rows.contains(&row_idx);

                        let (rect, response) =
                            ui.allocate_exact_size(Vec2::new(cell_w, cell_h), Sense::click());

                        let mut bg = row_bg;
                        if is_dirty && changed && !is_delete {
                            bg = tokens.cell_edit_bg;
                        }
                        if tab.editing == Some(key) {
                            bg = tokens.cell_edit_bg;
                        }
                        ui.painter().rect_filled(rect, 0.0, bg);
                        ui.painter().rect_stroke(
                            rect,
                            0.0,
                            Stroke::new(1.0, tokens.grid_line),
                            egui::StrokeKind::Inside,
                        );

                        if response.clicked() {
                            tab.selected_row = Some(row_idx);
                        }

                        let editing = tab.editing == Some(key);
                        if editing && !pk_locked && !is_delete {
                            let mut buf = value;
                            let te_id = ui.id().with(("edit_cell", row_idx, col_idx));
                            let edit = TextEdit::singleline(&mut buf)
                                .id(te_id)
                                .frame(Frame::NONE)
                                .text_color(tokens.text)
                                .margin(Margin::symmetric(4, 2));
                            let inner = ui.put(rect, edit);

                            if buf != original {
                                tab.edits.insert(key, buf);
                                tab.dirty_rows.insert(row_idx);
                            } else {
                                tab.edits.remove(&key);
                                let still = tab.edits.keys().any(|(r, _)| *r == row_idx);
                                if !still && !is_new {
                                    tab.dirty_rows.remove(&row_idx);
                                }
                            }

                            let commit = inner.lost_focus()
                                || (inner.has_focus()
                                    && ui.input(|i| i.key_pressed(Key::Enter)));
                            if commit {
                                tab.editing = None;
                            } else if !inner.has_focus() {
                                inner.request_focus();
                            }
                        } else {
                            let display = flatten_cell_text(&value);
                            let color = if is_delete {
                                tokens.muted
                            } else if display.eq_ignore_ascii_case("NULL") {
                                tokens.muted
                            } else if pk_locked {
                                tokens.muted
                            } else {
                                tokens.text
                            };
                            let text_rect = rect.shrink2(Vec2::new(4.0, 0.0));
                            ui.painter().with_clip_rect(text_rect).text(
                                text_rect.left_center(),
                                egui::Align2::LEFT_CENTER,
                                &display,
                                egui::FontId::proportional(13.0),
                                color,
                            );
                            if response.hovered() && display.len() > 24 {
                                response.clone().on_hover_text(value.as_str());
                            }
                            if (response.clicked() || response.double_clicked())
                                && !pk_locked
                                && !is_delete
                            {
                                tab.editing = Some(key);
                                tab.edits.entry(key).or_insert(value);
                                tab.selected_row = Some(row_idx);
                            }
                        }
                    }
                });
            }
        });
}

fn header_cell(ui: &mut Ui, tokens: &Tokens, name: &str, width: f32, height: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    ui.painter().rect_filled(rect, 0.0, tokens.header_bg);
    ui.painter().rect_stroke(
        rect,
        0.0,
        Stroke::new(1.0, tokens.grid_line),
        egui::StrokeKind::Inside,
    );
    let text_rect = rect.shrink2(Vec2::new(4.0, 0.0));
    ui.painter().with_clip_rect(text_rect).text(
        text_rect.left_center(),
        egui::Align2::LEFT_CENTER,
        name,
        egui::FontId::proportional(13.0),
        tokens.accent,
    );
}

fn flatten_cell_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len().min(256));
    let mut prev_space = false;
    for ch in s.chars().take(512) {
        if ch.is_whitespace() {
            if !prev_space && !out.is_empty() {
                out.push(' ');
                prev_space = true;
            }
        } else {
            out.push(ch);
            prev_space = false;
        }
    }
    if s.chars().count() > 512 {
        out.push('…');
    }
    out
}

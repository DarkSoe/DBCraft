//! Lightweight Unicode icons for toolbars (egui default emoji/symbol fonts).

use egui::{Color32, Sense, Ui, Vec2};

pub mod glyph {
    pub const TABLE: &str = "▦";
    pub const VIEW: &str = "👁";
    pub const FUNCTION: &str = "ƒ";
    pub const USER: &str = "👤";
    pub const DATABASE: &str = "🗄";
    pub const SERVER: &str = "🖥";
    pub const CONNECTED: &str = "●";
    pub const DISCONNECTED: &str = "○";
    pub const OPEN: &str = "📂";
    pub const DESIGN: &str = "✎";
    pub const ADD: &str = "＋";
    pub const REMOVE: &str = "－";
    pub const DELETE: &str = "🗑";
    pub const SAVE: &str = "💾";
    pub const REFRESH: &str = "↻";
    pub const QUERY: &str = "⌘";
    pub const RUN: &str = "▶";
    pub const KEY: &str = "🔑";
    pub const UP: &str = "↑";
    pub const DOWN: &str = "↓";
    pub const EDIT: &str = "✎";
    pub const SQL: &str = "</>";
    pub const CLOSE: &str = "×";
}

pub fn label(icon: &str, text: &str) -> String {
    format!("{icon}  {text}")
}

/// Navicat-style category chip: icon above label, highlighted when selected.
pub fn category_chip(
    ui: &mut Ui,
    selected: bool,
    icon: &str,
    caption: &str,
    accent: Color32,
    muted: Color32,
) -> egui::Response {
    let desired = Vec2::new(72.0, 52.0);
    let (rect, response) = ui.allocate_exact_size(desired, Sense::click());

    let bg = if selected {
        accent.gamma_multiply(0.22)
    } else if response.hovered() {
        accent.gamma_multiply(0.10)
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, 4.0, bg);
    if selected {
        ui.painter().rect_stroke(
            rect,
            4.0,
            egui::Stroke::new(1.0, accent),
            egui::StrokeKind::Inside,
        );
    }

    let icon_color = if selected { accent } else { muted };
    let text_color = if selected {
        ui.visuals().strong_text_color()
    } else {
        muted
    };

    ui.painter().text(
        rect.center() + Vec2::new(0.0, -8.0),
        egui::Align2::CENTER_CENTER,
        icon,
        egui::FontId::proportional(18.0),
        icon_color,
    );
    ui.painter().text(
        rect.center() + Vec2::new(0.0, 12.0),
        egui::Align2::CENTER_CENTER,
        caption,
        egui::FontId::proportional(12.0),
        text_color,
    );

    response
}

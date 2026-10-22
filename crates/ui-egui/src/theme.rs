use craft_ui::theme::WidgetPalette;
use egui::{Color32, CornerRadius, Stroke, Visuals};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    Dark,
    Light,
}

pub struct Tokens {
    pub bg: Color32,
    pub panel: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub accent: Color32,
    pub border: Color32,
    pub danger: Color32,
    /// Navicat-like striped row tint.
    pub stripe: Color32,
    pub header_bg: Color32,
    pub toolbar_bg: Color32,
    pub grid_line: Color32,
    pub cell_edit_bg: Color32,
}

impl Tokens {
    pub fn for_mode(mode: ThemeMode) -> Self {
        match mode {
            // Navicat-inspired dark: cool gray chrome, teal selection
            ThemeMode::Dark => Self {
                bg: Color32::from_rgb(32, 34, 38),
                panel: Color32::from_rgb(40, 42, 48),
                text: Color32::from_rgb(228, 230, 234),
                muted: Color32::from_rgb(138, 144, 154),
                accent: Color32::from_rgb(52, 132, 210),
                border: Color32::from_rgb(58, 62, 70),
                danger: Color32::from_rgb(200, 80, 80),
                stripe: Color32::from_rgb(36, 48, 62),
                header_bg: Color32::from_rgb(48, 52, 60),
                toolbar_bg: Color32::from_rgb(44, 46, 52),
                grid_line: Color32::from_rgb(58, 62, 70),
                cell_edit_bg: Color32::from_rgb(28, 48, 72),
            },
            // Navicat-inspired light: white panels, soft blue stripes
            ThemeMode::Light => Self {
                bg: Color32::from_rgb(240, 242, 245),
                panel: Color32::from_rgb(255, 255, 255),
                text: Color32::from_rgb(32, 34, 38),
                muted: Color32::from_rgb(110, 116, 126),
                accent: Color32::from_rgb(30, 110, 190),
                border: Color32::from_rgb(200, 206, 214),
                danger: Color32::from_rgb(180, 50, 50),
                stripe: Color32::from_rgb(232, 244, 252),
                header_bg: Color32::from_rgb(245, 247, 250),
                toolbar_bg: Color32::from_rgb(248, 249, 251),
                grid_line: Color32::from_rgb(220, 224, 230),
                cell_edit_bg: Color32::from_rgb(255, 252, 230),
            },
        }
    }

    pub fn apply(&self, ctx: &egui::Context, dark: bool) {
        let mut visuals = if dark {
            Visuals::dark()
        } else {
            Visuals::light()
        };
        visuals.window_fill = self.panel;
        visuals.panel_fill = self.panel;
        visuals.override_text_color = Some(self.text);
        visuals.selection.bg_fill = self.accent.gamma_multiply(0.35);
        visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, self.border);
        visuals.extreme_bg_color = self.bg;
        visuals.faint_bg_color = self.stripe;
        let palette = WidgetPalette {
            noninteractive: self.panel,
            inactive: self.toolbar_bg,
            hovered: self.bg,
            active: self.accent.gamma_multiply(0.25),
            open: self.bg,
        };
        palette.apply_to(
            &mut visuals.widgets,
            CornerRadius::same(2),
            Stroke::new(1.0, self.text),
        );
        ctx.set_visuals(visuals);
    }
}

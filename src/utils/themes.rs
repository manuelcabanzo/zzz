use egui::{Color32, FontData, Stroke, Rounding, epaint::Shadow, Vec2};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    pub background_color: Color32,
    pub text_color: Color32,
    pub accent_color: Color32,
    pub secondary_color: Color32,
    pub font_family: String,
    pub font_size: f32,
    pub extreme_bg_color: Color32,
    pub panel_fill: Color32,
    pub window_shadow: Shadow,
    pub selection_color: Color32,
    pub hover_color: Color32,
    pub active_color: Color32,
    pub widget_border_color: Color32,
    pub widget_border_width: f32,
    pub widget_rounding: Rounding,
}

impl Theme {
    const DEFAULT_FONT_SIZE: f32 = 14.0;
    const DEFAULT_WIDGET_BORDER_WIDTH: f32 = 1.0;
    const DEFAULT_WIDGET_ROUNDING: Rounding = Rounding::same(0.0);

    fn create_shadow(offset: Vec2, blur: f32, spread: f32, alpha: u8) -> Shadow {
        Shadow {
            offset,
            blur,
            spread,
            color: Color32::from_black_alpha(alpha),
        }
    }

    pub fn cream() -> Self {
        Self::create_theme(
            Color32::from_rgb(240, 240, 240),
            Color32::from_rgb(60, 60, 60),
            Color32::from_rgb(0, 122, 204),
            Color32::from_rgb(180, 180, 180),
            Color32::from_rgb(30, 30, 30),
            Color32::from_rgb(230, 230, 230),
            Self::create_shadow(Vec2::new(2.0, 2.0), 8.0, 0.0, 20),
            Color32::from_rgb(173, 214, 255),
            Color32::from_rgb(220, 220, 220),
            Color32::from_rgb(200, 200, 200),
            Color32::from_rgb(180, 180, 180),
        )
    }

    pub fn black() -> Self {
        Self::create_theme(
            Color32::from_rgb(30, 30, 40),
            Color32::from_rgb(255, 255, 255),
            Color32::from_rgb(0, 122, 204),
            Color32::from_rgb(80, 80, 80),
            Color32::from_rgb(20, 20, 20),
            Color32::from_rgb(40, 40, 50),
            Self::create_shadow(Vec2::new(2.0, 2.0), 8.0, 0.0, 40),
            Color32::from_rgb(70, 130, 180),
            Color32::from_rgb(50, 50, 60),
            Color32::from_rgb(60, 60, 70),
            Color32::from_rgb(70, 70, 80),
        )
    }

    pub fn purple() -> Self {
        Self::create_theme(
            Color32::from_rgb(230, 230, 250),
            Color32::from_rgb(25, 25, 112),
            Color32::from_rgb(255, 105, 180),
            Color32::from_rgb(180, 180, 220),
            Color32::from_rgb(30, 30, 30),
            Color32::from_rgb(220, 220, 240),
            Self::create_shadow(Vec2::new(2.0, 2.0), 8.0, 0.0, 20),
            Color32::from_rgb(255, 182, 193),
            Color32::from_rgb(240, 240, 255),
            Color32::from_rgb(220, 220, 235),
            Color32::from_rgb(180, 180, 220),
        )
    }

    fn create_theme(
        background_color: Color32,
        text_color: Color32,
        accent_color: Color32,
        secondary_color: Color32,
        extreme_bg_color: Color32,
        panel_fill: Color32,
        window_shadow: Shadow,
        selection_color: Color32,
        hover_color: Color32,
        active_color: Color32,
        widget_border_color: Color32,
    ) -> Self {
        Self {
            background_color,
            text_color,
            accent_color,
            secondary_color,
            font_family: "Caskaydia Cove Nerd Font Mono".to_string(),
            font_size: Self::DEFAULT_FONT_SIZE,
            extreme_bg_color,
            panel_fill,
            window_shadow,
            selection_color,
            hover_color,
            active_color,
            widget_border_color,
            widget_border_width: Self::DEFAULT_WIDGET_BORDER_WIDTH,
            widget_rounding: Self::DEFAULT_WIDGET_ROUNDING,
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::purple()
    }
}

pub fn custom_theme(ctx: &egui::Context, theme: &Theme) -> egui::Visuals {
    let mut visuals = egui::Visuals::light();

    // Set window rounding to zero
    visuals.window_rounding = Rounding::same(0.0);

    // Remove rounding from popup windows and tooltips
    visuals.popup_shadow.blur = 0.0;
    visuals.popup_shadow.spread = 0.0;

    visuals.window_fill = theme.background_color;
    visuals.panel_fill = theme.panel_fill;
    visuals.override_text_color = Some(theme.text_color);
    visuals.selection.bg_fill = theme.selection_color;
    visuals.selection.stroke = Stroke::new(1.0, theme.accent_color);
    visuals.window_shadow = theme.window_shadow;

    // Set all widget roundings to zero
    visuals.widgets.noninteractive.rounding = Rounding::same(0.0);
    visuals.widgets.inactive.rounding = Rounding::same(0.0);
    visuals.widgets.hovered.rounding = Rounding::same(0.0);
    visuals.widgets.active.rounding = Rounding::same(0.0);
    visuals.widgets.open.rounding = Rounding::same(0.0);

    visuals.widgets.noninteractive.bg_fill = theme.background_color;
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, theme.text_color);
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(theme.widget_border_width, theme.widget_border_color);

    visuals.widgets.inactive = visuals.widgets.noninteractive.clone();
    visuals.widgets.inactive.bg_fill = theme.secondary_color;

    visuals.widgets.hovered = visuals.widgets.inactive.clone();
    visuals.widgets.hovered.bg_fill = theme.hover_color;
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.5, theme.text_color);

    visuals.widgets.active = visuals.widgets.hovered.clone();
    visuals.widgets.active.bg_fill = theme.active_color;
    visuals.widgets.active.fg_stroke = Stroke::new(2.0, theme.accent_color);

    visuals.widgets.open = visuals.widgets.active.clone();

    ctx.set_style(egui::Style {
        visuals: visuals.clone(),
        animation_time: 0.1,
        ..Default::default()
    });

    // Load custom font
    let font_data = FontData::from_static(include_bytes!("../resources/CaskaydiaCoveNerdFontMono-Regular.ttf"));
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(theme.font_family.clone(), font_data);
    fonts.families
        .get_mut(&egui::FontFamily::Proportional)
        .unwrap()
        .insert(0, theme.font_family.clone());
    fonts.families
        .get_mut(&egui::FontFamily::Monospace)
        .unwrap()
        .insert(0, theme.font_family.clone());

    ctx.set_fonts(fonts);

    visuals
}
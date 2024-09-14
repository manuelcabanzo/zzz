use egui::{Color32, FontData};

#[derive(Clone)]
pub struct Theme {
    pub background_color: Color32,
    pub text_color: Color32,
    pub accent_color: Color32,
    pub font_family: String,
    #[allow(dead_code)]
    pub font_size: f32,
    pub extreme_bg_color: Color32,
}

impl Theme {

    pub fn cream() -> Self {
        Self {
            background_color: Color32::from_rgb(240, 240, 240),
            text_color: Color32::from_rgb(0, 0, 0),
            accent_color: Color32::from_rgb(0, 122, 204),
            font_family: "JetBrains Mono".to_string(),
            font_size: 14.0,
            extreme_bg_color: Color32::from_rgb(30, 30, 30),
        }
    }

    pub fn black() -> Self {
        Self {
            background_color: Color32::from_rgb(30, 30, 40),
            text_color: Color32::from_rgb(255, 255, 255),
            accent_color: Color32::from_rgb(0, 122, 204),
            font_family: "JetBrains Mono".to_string(),
            font_size: 14.0,
            extreme_bg_color: Color32::from_rgb(30, 30, 30),
        }
    }

    pub fn purple() -> Self {
        Self {
            background_color: Color32::from_rgb(230, 230, 250), // Light purple background
            text_color: Color32::from_rgb(25, 25, 112),         // Dark blue text
            accent_color: Color32::from_rgb(255, 105, 180),     // Hot pink accent
            font_family: "JetBrains Mono".to_string(),
            font_size: 14.0,
            extreme_bg_color: Color32::from_rgb(30, 30, 30),
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
    visuals.window_fill = theme.background_color;
    visuals.panel_fill = theme.background_color;
    visuals.override_text_color = None;
    visuals.selection.bg_fill = theme.accent_color;
    visuals.selection.stroke.color = theme.accent_color;
    visuals.widgets.noninteractive.bg_fill = theme.background_color;
    visuals.widgets.noninteractive.fg_stroke.color = theme.text_color;
    visuals.widgets.inactive.bg_fill = theme.background_color;
    visuals.widgets.hovered.bg_fill = theme.accent_color.linear_multiply(0.3);
    visuals.widgets.active.bg_fill = theme.accent_color.linear_multiply(0.5);

    ctx.set_style(egui::Style {
        visuals: visuals.clone(),
        ..Default::default()
    });

    let font_data = FontData::from_static(include_bytes!("../resources/JetBrainsMono-Regular.ttf"));
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(theme.font_family.clone().into(), font_data);
    ctx.set_fonts(fonts);

    visuals
}

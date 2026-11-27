use eframe::egui::{
    Color32, Visuals, Style, FontDefinitions, FontFamily, FontData, Context, Shadow, Stroke,
    Margin, TextStyle, FontId, vec2, style::{Widgets, WidgetVisuals, Selection},
};

/// Orokin Gold — color principal Warframe
pub const GOLD: Color32 = Color32::from_rgb(0xC8, 0xA8, 0x48);
/// Energy Blue — acentos, links, highlights
pub const ENERGY_BLUE: Color32 = Color32::from_rgb(0x00, 0x78, 0xD7);
/// Void White — texto principal
pub const FG: Color32 = Color32::from_rgb(0xE8, 0xE8, 0xE0);
/// Gris tenue — texto secundario, hints
pub const DIM: Color32 = Color32::from_rgb(0x88, 0x88, 0x80);
/// Fondo oscuro — estilo Void
pub const BG: Color32 = Color32::from_rgb(0x12, 0x12, 0x14);
/// Panel surface
pub const SURFACE: Color32 = Color32::from_rgb(0x1A, 0x1A, 0x1E);
/// Border sutil
pub const BORDER: Color32 = Color32::from_rgb(0x38, 0x30, 0x20);
/// Verde éxito (tools, OK)
pub const OK: Color32 = Color32::from_rgb(0x4E, 0xC8, 0x4E);
/// Rojo error
pub const ERR: Color32 = Color32::from_rgb(0xE8, 0x4E, 0x4E);
/// Acento naranja/oro
pub const ACCENT: Color32 = Color32::from_rgb(0xD8, 0xA0, 0x30);
/// Color usuario (Tenno)
pub const USER_COLOR: Color32 = Color32::from_rgb(0x40, 0xB0, 0xF0);

/// Aplica el tema Warframe al contexto egui.
pub fn apply(ctx: &Context) {
    let mut style = Style::default();

    // Helper para crear WidgetVisuals base
    let base_widget = WidgetVisuals {
        bg_fill: SURFACE,
        weak_bg_fill: BG,
        fg_stroke: Stroke::new(1.0_f32, FG),
        bg_stroke: Stroke::new(1.0_f32, BORDER),
        rounding: 4.0.into(),
        expansion: 0.0,
    };

    // Visuals
    style.visuals = Visuals {
        dark_mode: true,
        override_text_color: Some(FG),
        widgets: Widgets {
            noninteractive: WidgetVisuals {
                bg_fill: SURFACE,
                weak_bg_fill: BG,
                fg_stroke: Stroke::new(1.0_f32, FG),
                bg_stroke: Stroke::new(1.0_f32, BORDER),
                rounding: 4.0.into(),
                expansion: 0.0,
            },
            inactive: WidgetVisuals {
                bg_fill: SURFACE,
                weak_bg_fill: BG,
                fg_stroke: Stroke::new(1.0_f32, FG),
                bg_stroke: Stroke::new(1.0_f32, BORDER),
                rounding: 4.0.into(),
                expansion: 0.0,
            },
            hovered: WidgetVisuals {
                bg_fill: Color32::from_rgb(0x28, 0x24, 0x1C),
                weak_bg_fill: BG,
                fg_stroke: Stroke::new(1.0_f32, GOLD),
                bg_stroke: Stroke::new(1.0_f32, GOLD),
                rounding: 4.0.into(),
                expansion: 0.0,
            },
            active: WidgetVisuals {
                bg_fill: Color32::from_rgb(0x30, 0x28, 0x18),
                weak_bg_fill: BG,
                fg_stroke: Stroke::new(1.0_f32, GOLD),
                bg_stroke: Stroke::new(2.0_f32, GOLD),
                rounding: 4.0.into(),
                expansion: 0.0,
            },
            open: WidgetVisuals {
                bg_fill: SURFACE,
                weak_bg_fill: BG,
                fg_stroke: Stroke::new(1.0_f32, FG),
                bg_stroke: Stroke::new(1.0_f32, BORDER),
                rounding: 4.0.into(),
                expansion: 0.0,
            },
        },
        selection: Selection {
            bg_fill: Color32::from_rgba_premultiplied(0xC8, 0xA8, 0x48, 0x60),
            stroke: Stroke::new(1.0_f32, GOLD),
        },
        hyperlink_color: ENERGY_BLUE,
        faint_bg_color: BG,
        extreme_bg_color: BG,
        code_bg_color: Color32::from_rgb(0x0A, 0x0A, 0x0C),
        warn_fg_color: Color32::from_rgb(0xF0, 0xC0, 0x30),
        error_fg_color: ERR,
        window_rounding: 8.0.into(),
        window_shadow: Shadow {
            offset: vec2(8.0, 8.0),
            blur: 16.0,
            spread: 0.0,
            color: Color32::from_rgba_premultiplied(0x00, 0x00, 0x00, 0xC0),
        },
        window_fill: BG,
        panel_fill: BG,
        ..Default::default()
    };

    // Spacing
    style.spacing.item_spacing = vec2(8.0, 6.0);
    style.spacing.window_margin = Margin::same(12.0);
    style.spacing.button_padding = vec2(12.0, 6.0);
    style.spacing.menu_margin = Margin::same(8.0);
    style.spacing.indent = 20.0;
    style.spacing.icon_width = 18.0;
    style.spacing.combo_width = 200.0;
    style.spacing.text_edit_width = 280.0;

    // Interaction
    style.interaction.tooltip_delay = 0.3;
    style.interaction.show_tooltips_only_when_still = false;

    // Fonts — usar fuente del sistema con fallback
    let mut fonts = FontDefinitions::default();
    fonts.families.insert(FontFamily::Monospace, vec!["JetBrains Mono".to_string(), "Cascadia Code".to_string(), "Consolas".to_string(), "monospace".to_string()]);
    fonts.families.insert(FontFamily::Proportional, vec!["Segoe UI".to_string(), "Inter".to_string(), "Roboto".to_string(), "sans-serif".to_string()]);
    style.text_styles.insert(
        TextStyle::Monospace,
        FontId::new(13.0, FontFamily::Monospace),
    );
    style.text_styles.insert(
        TextStyle::Body,
        FontId::new(14.0, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Button,
        FontId::new(13.0, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Heading,
        FontId::new(22.0, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Small,
        FontId::new(11.0, FontFamily::Monospace),
    );

    ctx.set_style(style);
    ctx.set_fonts(fonts);
}
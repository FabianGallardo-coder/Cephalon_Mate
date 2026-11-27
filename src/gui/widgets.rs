//! Widgets reutilizables para la GUI Cephalon.

use eframe::egui::{Color32, RichText, Ui, Vec2, Sense, Response, Painter, Rect, Shape, Stroke, TextureHandle, Pos2, pos2, vec2, Frame, Image, Button};
// Tooltip is not available as a public struct in egui 0.30; using ui.tooltip() instead

use crate::gui::theme::{GOLD, BG, FG, DIM, OK, ERR, ACCENT};

/// Panel con estilo Warframe (borde oro sutil, fondo void).
pub fn warframe_panel(ui: &mut Ui, title: &str, add_contents: impl FnOnce(&mut Ui)) {
    Frame::none()
        .fill(crate::gui::theme::SURFACE)
        .stroke(Stroke::new(1.0, crate::gui::theme::BORDER))
        .rounding(8.0)
        .inner_margin(12.0)
        .show(ui, |ui| {
            if !title.is_empty() {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(title).color(GOLD).strong().size(16.0));
                    ui.separator();
                });
                ui.add_space(8.0);
            }
            add_contents(ui);
        });
}

/// Botón estilo Warframe (oro sobre void).
pub fn warframe_button(ui: &mut Ui, label: &str, color: Color32) -> Response {
    let text_color = if color == GOLD { BG } else { FG };
    ui.add(
        egui::Button::new(RichText::new(label).color(text_color).strong())
            .fill(color)
            .rounding(4.0)
            .min_size(Vec2::new(80.0, 32.0))
    )
}

/// Línea divisoria decorativa (estilo Orokin).
pub fn orokin_divider(ui: &mut Ui) {
    let (rect, _) = ui.allocate_at_least(Vec2::new(ui.available_width(), 2.0), Sense::hover());
    let painter = ui.painter();
    let y = rect.center().y;
    painter.line_segment(
        [rect.left_top(), rect.right_top()],
        Stroke::new(1.0, crate::gui::theme::BORDER),
    );
    // Detalle central dorado
    let cx = rect.center().x;
    painter.circle_filled(pos2(cx, y), 4.0, GOLD);
    painter.circle_stroke(pos2(cx, y), 6.0, Stroke::new(1.0, GOLD));
}

/// Indicador de estado (círculo coloreado + label).
pub fn status_indicator(ui: &mut Ui, label: &str, color: Color32, online: bool) {
    ui.horizontal(|ui| {
        let radius = 6.0;
        let center = ui.cursor().left_top() + vec2(radius, radius);
        let painter = ui.painter();
        painter.circle_filled(center, radius, color);
        if online {
            painter.circle_stroke(center, radius + 2.0, Stroke::new(1.5, color.gamma_multiply(0.5)));
        }
        ui.add_space(radius * 2.0 + 8.0);
        ui.label(RichText::new(label).size(12.0).color(if online { FG } else { DIM }));
    });
}

/// Tarjeta de item/warframe compacta para chat o worldstate.
pub fn item_card(ui: &mut Ui, name: &str, subtitle: &str, icon: Option<&TextureHandle>, color: Color32) {
    Frame::none()
        .fill(crate::gui::theme::SURFACE)
        .stroke(Stroke::new(1.0, crate::gui::theme::BORDER))
        .rounding(6.0)
        .inner_margin(8.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                if let Some(tex) = icon {
                    ui.add(Image::new(tex).fit_to_exact_size(Vec2::new(48.0, 48.0)));
                } else {
                    // Placeholder: círculo color
                    let (rect, _) = ui.allocate_at_least(Vec2::new(48.0, 48.0), Sense::hover());
                    ui.painter().circle_filled(rect.center(), 20.0, color);
                }
                ui.vertical(|ui| {
                    ui.label(RichText::new(name).color(FG).strong().size(14.0));
                    ui.label(RichText::new(subtitle).color(DIM).size(11.0));
                });
            });
        });
}

/// Spinner personalizado estilo Cephalon (anillo rotando oro).
pub fn cephalon_spinner(ui: &mut Ui, size: f32) {
    let (rect, _) = ui.allocate_at_least(Vec2::splat(size), Sense::hover());
    let painter = ui.painter();
    let center = rect.center();
    let radius = size / 2.0 - 2.0;
    let time = ui.input(|i| i.time) * 3.0; // velocidad
    let start_angle = time as f32;
    let end_angle = time as f32 + std::f32::consts::PI * 1.5;
    painter.circle_stroke(
        center,
        radius,
        Stroke::new(3.0_f32, GOLD),
    );
    // Arco animado
    let points: Vec<Pos2> = (0..=30).map(|i| {
        let angle = start_angle + (end_angle - start_angle) * (i as f32 / 30.0);
        center + vec2(angle.cos(), angle.sin()) * radius
    }).collect();
    painter.add(Shape::line(points, Stroke::new(3.0_f32, GOLD)));
}

/// Tooltip enriquecido para tools (muestra args y resultado).
/// Usa un panel simple en lugar de Tooltip (no disponible en egui 0.30).
pub fn tool_tooltip(ui: &mut Ui, tool_name: &str, args: &str, result_preview: &str) {
    egui::Frame::none()
        .fill(crate::gui::theme::SURFACE)
        .stroke(Stroke::new(1.0, crate::gui::theme::BORDER))
        .rounding(8.0)
        .inner_margin(12.0)
        .show(ui, |ui| {
            ui.set_min_width(300.0);
            warframe_panel(ui, &format!("🔧 Tool: {}", tool_name), |ui| {
                ui.label(RichText::new("Args:").color(ACCENT).strong());
                ui.code(args);
                ui.add_space(8.0);
                ui.label(RichText::new("Preview:").color(ACCENT).strong());
                ui.label(RichText::new(result_preview).color(FG).size(11.0));
            });
        });
}
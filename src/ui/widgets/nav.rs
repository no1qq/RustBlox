use egui::{Align2, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2};

use crate::ui::icons::{self, Icon};
use crate::ui::theme::{self, Theme};

use super::button::blend;

pub fn nav_item(
    ui: &mut Ui,
    icon: Icon,
    label: &str,
    selected: bool,
    badge: Option<&str>,
    expansion: f32,
) -> Response {
    let theme = Theme::get(ui.ctx());
    let height = theme.metrics.row_h;
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::click());
    let expansion = expansion.clamp(0.0, 1.0);

    if ui.is_rect_visible(rect) {
        let palette = theme.palette;
        let hover = ui.ctx().animate_bool_with_time(
            response.id.with("hover"),
            response.hovered(),
            theme.anim(0.11),
        );
        let active =
            ui.ctx()
                .animate_bool_with_time(response.id.with("active"), selected, theme.anim(0.15));

        if hover > 0.0 || active > 0.0 {
            let fill = palette
                .surface_hover
                .gamma_multiply((hover * 0.6 + active * 0.9).clamp(0.0, 1.0));
            ui.painter().rect_filled(rect, theme.radius_sm(), fill);
        }

        if response.has_focus() {
            ui.painter().rect_stroke(
                rect,
                theme.radius_sm(),
                Stroke::new(2.0, palette.accent),
                StrokeKind::Inside,
            );
        }

        let tint = if selected {
            palette.text
        } else {
            blend(palette.text_muted, palette.text, hover)
        };
        let glyph = if selected { palette.accent } else { tint };

        let icon_rect = Rect::from_min_size(
            egui::pos2(rect.left() + 14.0, rect.center().y - 8.0),
            Vec2::splat(16.0),
        );
        icons::draw(ui.painter(), icon, icon_rect, glyph, 1.7);

        let font = if selected {
            theme::medium(theme::size::BODY)
        } else {
            theme::text_style(theme::size::BODY)
        };
        let reveal = ((expansion - 0.35) / 0.65).clamp(0.0, 1.0);
        if reveal > 0.0 {
            ui.painter().text(
                egui::pos2(icon_rect.right() + 11.0, rect.center().y),
                Align2::LEFT_CENTER,
                label,
                font,
                tint.gamma_multiply(reveal),
            );
        }

        if let Some(badge) = badge {
            if expansion > 0.5 {
                let badge_font = theme::medium(theme::size::MICRO);
                let galley = ui.painter().layout_no_wrap(
                    badge.to_owned(),
                    badge_font.clone(),
                    egui::Color32::PLACEHOLDER,
                );
                let badge_rect = Rect::from_center_size(
                    egui::pos2(rect.right() - 14.0 - galley.size().x / 2.0, rect.center().y),
                    Vec2::new(galley.size().x + 12.0, 16.0),
                );
                ui.painter()
                    .rect_filled(badge_rect, theme.radius_sm(), palette.surface_press);
                ui.painter().text(
                    badge_rect.center(),
                    Align2::CENTER_CENTER,
                    badge,
                    badge_font,
                    palette.text_muted,
                );
            } else {
                let dot = egui::pos2(icon_rect.right() + 1.0, icon_rect.top() + 1.0);
                ui.painter()
                    .circle_filled(dot, 3.5, palette.accent.gamma_multiply(1.0 - reveal));
            }
        }
    }

    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    if expansion < 0.5 {
        response.on_hover_text(label)
    } else {
        response
    }
}

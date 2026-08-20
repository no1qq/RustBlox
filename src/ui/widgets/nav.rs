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
) -> Response {
    let theme = Theme::get(ui.ctx());
    let height = theme.metrics.row_h;
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::click());

    if ui.is_rect_visible(rect) {
        let palette = theme.palette;
        let hover =
            ui.ctx()
                .animate_bool_with_time(response.id.with("hover"), response.hovered(), 0.11);
        let active = ui
            .ctx()
            .animate_bool_with_time(response.id.with("active"), selected, 0.15);

        if hover > 0.0 || active > 0.0 {
            let fill = blend(palette.surface_hover, palette.accent_soft, active)
                .gamma_multiply((hover * 0.7 + active).clamp(0.0, 1.0));
            ui.painter().rect_filled(rect, theme.radius_sm(), fill);
        }

        if active > 0.0 {
            let bar = Rect::from_min_size(
                egui::pos2(rect.left() + 1.0, rect.center().y - height * 0.28 * active),
                Vec2::new(3.0, height * 0.56 * active),
            );
            ui.painter()
                .rect_filled(bar, egui::CornerRadius::same(2), palette.accent);
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

        let icon_rect = Rect::from_min_size(
            egui::pos2(rect.left() + 15.0, rect.center().y - 9.0),
            Vec2::splat(18.0),
        );
        icons::draw(
            ui.painter(),
            icon,
            icon_rect,
            tint,
            if selected { 1.9 } else { 1.7 },
        );

        let font = if selected {
            theme::medium(theme::size::BODY)
        } else {
            theme::text_style(theme::size::BODY)
        };
        ui.painter().text(
            egui::pos2(icon_rect.right() + 12.0, rect.center().y),
            Align2::LEFT_CENTER,
            label,
            font,
            tint,
        );

        if let Some(badge) = badge {
            let badge_font = theme::medium(theme::size::MICRO);
            let galley = ui.painter().layout_no_wrap(
                badge.to_owned(),
                badge_font.clone(),
                egui::Color32::PLACEHOLDER,
            );
            let badge_rect = Rect::from_center_size(
                egui::pos2(rect.right() - 14.0 - galley.size().x / 2.0, rect.center().y),
                Vec2::new(galley.size().x + 14.0, 18.0),
            );
            ui.painter().rect_filled(
                badge_rect,
                egui::CornerRadius::same(9),
                palette.accent.gamma_multiply(0.2),
            );
            ui.painter().text(
                badge_rect.center(),
                Align2::CENTER_CENTER,
                badge,
                badge_font,
                palette.accent,
            );
        }
    }

    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    response
}

pub fn brand(ui: &mut Ui, name: &str, tag: &str) {
    let theme = Theme::get(ui.ctx());
    let palette = theme.palette;
    let height = 44.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::hover());

    let mark = Rect::from_min_size(
        egui::pos2(rect.left() + 12.0, rect.center().y - 15.0),
        Vec2::splat(30.0),
    );
    ui.painter()
        .rect_filled(mark, theme.radius_md(), palette.accent);

    let inset = mark.shrink(7.0);
    ui.painter().add(egui::Shape::convex_polygon(
        vec![
            egui::pos2(inset.left(), inset.bottom()),
            egui::pos2(inset.center().x, inset.top()),
            egui::pos2(inset.right(), inset.bottom()),
        ],
        palette.on_accent,
        Stroke::NONE,
    ));

    let core = inset.shrink(4.0);
    ui.painter().add(egui::Shape::convex_polygon(
        vec![
            egui::pos2(core.left(), core.bottom()),
            egui::pos2(core.center().x, core.center().y - core.height() * 0.1),
            egui::pos2(core.right(), core.bottom()),
        ],
        palette.accent,
        Stroke::NONE,
    ));

    ui.painter().text(
        egui::pos2(mark.right() + 11.0, rect.center().y - 8.0),
        Align2::LEFT_CENTER,
        name,
        theme::strong(theme::size::SECTION),
        palette.text,
    );
    ui.painter().text(
        egui::pos2(mark.right() + 11.0, rect.center().y + 8.0),
        Align2::LEFT_CENTER,
        tag,
        theme::text_style(theme::size::MICRO),
        palette.text_faint,
    );
}

pub fn section_label(ui: &mut Ui, text: &str) {
    let theme = Theme::get(ui.ctx());
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 24.0), Sense::hover());
    ui.painter().text(
        egui::pos2(rect.left() + 16.0, rect.center().y + 2.0),
        Align2::LEFT_CENTER,
        text.to_uppercase(),
        theme::medium(theme::size::MICRO - 0.5),
        theme.palette.text_faint,
    );
}

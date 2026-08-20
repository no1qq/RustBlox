use egui::{Rect, Sense, Stroke, StrokeKind, Vec2};

use crate::app::{AppState, ToastKind};

use super::icons::{self, Icon};
use super::theme::{self, Theme};

const WIDTH: f32 = 340.0;
const MARGIN: f32 = 18.0;

pub fn render(ctx: &egui::Context, theme: &Theme, state: &mut AppState) {
    if state.toasts.is_empty() {
        return;
    }

    let palette = theme.palette;
    let screen = ctx.viewport_rect();
    let mut dismissed = None;

    egui::Area::new(egui::Id::new("toasts"))
        .order(egui::Order::Tooltip)
        .fixed_pos(egui::pos2(
            screen.right() - WIDTH - MARGIN,
            screen.top() + 56.0,
        ))
        .interactable(true)
        .show(ctx, |ui| {
            ui.set_width(WIDTH);
            ui.spacing_mut().item_spacing.y = 10.0;

            for (index, toast) in state.toasts.items().iter().enumerate() {
                let (tint, icon) = match toast.kind {
                    ToastKind::Success => (palette.success, Icon::Check),
                    ToastKind::Info => (palette.info, Icon::Info),
                    ToastKind::Warning => (palette.warning, Icon::Warning),
                    ToastKind::Error => (palette.danger, Icon::Cross),
                };

                let entry =
                    ui.ctx()
                        .animate_bool_with_time(egui::Id::new(("toast", index)), true, 0.18);

                let response = egui::Frame::new()
                    .fill(palette.surface)
                    .stroke(Stroke::new(1.0, palette.border))
                    .corner_radius(theme.radius_md())
                    .inner_margin(egui::Margin::symmetric(14, 12))
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 8],
                        blur: 22,
                        spread: 0,
                        color: egui::Color32::from_black_alpha(if palette.is_dark {
                            120
                        } else {
                            40
                        }),
                    })
                    .show(ui, |ui| {
                        ui.set_width(WIDTH - 28.0);
                        ui.horizontal_top(|ui| {
                            ui.spacing_mut().item_spacing.x = 11.0;

                            let (icon_rect, _) =
                                ui.allocate_exact_size(Vec2::splat(18.0), Sense::hover());
                            icons::draw(ui.painter(), icon, icon_rect, tint, 1.9);

                            ui.vertical(|ui| {
                                ui.set_width(WIDTH - 76.0);
                                ui.label(
                                    egui::RichText::new(&toast.title)
                                        .font(theme::medium(theme::size::SMALL))
                                        .color(palette.text),
                                );
                                if let Some(body) = &toast.body {
                                    ui.add_space(2.0);
                                    ui.label(
                                        egui::RichText::new(body)
                                            .font(theme::text_style(theme::size::MICRO))
                                            .color(palette.text_muted),
                                    );
                                }
                            });

                            let (close_rect, close) =
                                ui.allocate_exact_size(Vec2::splat(18.0), Sense::click());
                            let tone = if close.hovered() {
                                palette.text
                            } else {
                                palette.text_faint
                            };
                            icons::draw(
                                ui.painter(),
                                Icon::Close,
                                close_rect.shrink(3.0),
                                tone,
                                1.6,
                            );
                            if close.clicked() {
                                dismissed = Some(index);
                            }
                            if close.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                        });
                    })
                    .response;

                let bar = Rect::from_min_size(
                    egui::pos2(response.rect.left(), response.rect.bottom() - 2.5),
                    Vec2::new(response.rect.width() * toast.remaining(), 2.5),
                );
                ui.painter().rect_filled(
                    bar,
                    egui::CornerRadius::same(2),
                    tint.gamma_multiply(0.7),
                );

                if entry < 1.0 {
                    ui.painter().rect_stroke(
                        response.rect,
                        theme.radius_md(),
                        Stroke::new(1.0, tint.gamma_multiply(1.0 - entry)),
                        StrokeKind::Inside,
                    );
                }
            }
        });

    if let Some(index) = dismissed {
        state.toasts.dismiss(index);
    }

    ctx.request_repaint_after(std::time::Duration::from_millis(100));
}

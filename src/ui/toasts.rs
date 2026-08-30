use egui::{Color32, CornerRadius, Pos2, Rect, Sense, Stroke, Vec2};

use crate::app::{AppState, ToastKind};

use super::icons::{self, Icon};
use super::theme::{self, Theme};

const WIDTH: f32 = 350.0;
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
            screen.top() + 52.0,
        ))
        .interactable(true)
        .show(ctx, |ui| {
            ui.set_width(WIDTH);
            ui.spacing_mut().item_spacing.y = 8.0;

            for toast in state.toasts.items() {
                let (tint, icon) = match toast.kind {
                    ToastKind::Success => (palette.success, Icon::Check),
                    ToastKind::Info => (palette.info, Icon::Info),
                    ToastKind::Warning => (palette.warning, Icon::Warning),
                    ToastKind::Error => (palette.danger, Icon::Cross),
                };

                let entry = ui.ctx().animate_bool_with_time(
                    egui::Id::new(("toast-entry", toast.id)),
                    true,
                    theme.anim(0.22),
                );

                let slide_x = (1.0 - entry) * 20.0;
                let card_alpha = entry.clamp(0.0, 1.0);

                let fill = if palette.is_dark {
                    palette.surface
                } else {
                    palette.surface_alt
                };

                let shadow_alpha = if palette.is_dark { 90 } else { 30 };

                let response = egui::Frame::new()
                    .fill(fill)
                    .stroke(Stroke::new(1.0, palette.border))
                    .corner_radius(theme.radius_md())
                    .inner_margin(egui::Margin::symmetric(12, 10))
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 6],
                        blur: 18,
                        spread: 0,
                        color: Color32::from_black_alpha(
                            ((shadow_alpha as f32) * card_alpha) as u8,
                        ),
                    })
                    .show(ui, |ui| {
                        ui.set_width(WIDTH - 24.0);
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 10.0;

                            let (badge_rect, _) =
                                ui.allocate_exact_size(Vec2::splat(26.0), Sense::hover());
                            let badge_rect = badge_rect.translate(Vec2::new(slide_x, 0.0));
                            ui.painter().rect_filled(
                                badge_rect,
                                CornerRadius::same(6),
                                tint.gamma_multiply(0.14),
                            );
                            ui.painter().rect_stroke(
                                badge_rect,
                                CornerRadius::same(6),
                                Stroke::new(1.0, tint.gamma_multiply(0.32)),
                                egui::StrokeKind::Inside,
                            );
                            icons::draw(ui.painter(), icon, badge_rect.shrink(5.0), tint, 1.8);

                            ui.vertical(|ui| {
                                ui.set_width(WIDTH - 82.0);
                                ui.label(
                                    egui::RichText::new(&toast.title)
                                        .font(theme::medium(theme::size::SMALL))
                                        .color(palette.text),
                                );
                                if let Some(body) = &toast.body {
                                    ui.add_space(1.0);
                                    ui.label(
                                        egui::RichText::new(body)
                                            .font(theme::text_style(theme::size::MICRO))
                                            .color(palette.text_muted),
                                    );
                                }
                            });

                            let (close_rect, close) =
                                ui.allocate_exact_size(Vec2::splat(20.0), Sense::click());
                            if close.hovered() {
                                ui.painter().rect_filled(
                                    close_rect,
                                    CornerRadius::same(10),
                                    palette.surface_hover,
                                );
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                            let close_tone = if close.hovered() {
                                palette.text
                            } else {
                                palette.text_faint
                            };
                            icons::draw(
                                ui.painter(),
                                Icon::Close,
                                close_rect.shrink(4.5),
                                close_tone,
                                1.5,
                            );
                            if close.clicked() {
                                dismissed = Some(toast.id);
                            }
                        });
                    })
                    .response;

                let track_width = response.rect.width() - 16.0;
                let track_rect = Rect::from_min_size(
                    Pos2::new(response.rect.left() + 8.0, response.rect.bottom() - 3.5),
                    Vec2::new(track_width, 2.0),
                );
                ui.painter().rect_filled(
                    track_rect,
                    CornerRadius::same(1),
                    palette.border.gamma_multiply(0.25),
                );

                let progress_rect = Rect::from_min_size(
                    Pos2::new(response.rect.left() + 8.0, response.rect.bottom() - 3.5),
                    Vec2::new(track_width * toast.remaining(), 2.0),
                );
                ui.painter().rect_filled(
                    progress_rect,
                    CornerRadius::same(1),
                    tint.gamma_multiply(0.85),
                );
            }
        });

    if let Some(id) = dismissed {
        state.toasts.dismiss(id);
    }

    ctx.request_repaint_after(std::time::Duration::from_millis(16));
}

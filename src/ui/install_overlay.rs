use egui::{Align, Align2, Layout, Rect, Sense, Stroke, Vec2};

use crate::app::{AppState, InstallPhase};
use crate::roblox::installer::{StageRow, StageState};
use crate::util::format;

use super::icons::{self, Icon};
use super::theme::{self, Theme};
use super::widgets::{self, feedback, MarkerState};

const CARD_WIDTH: f32 = 470.0;

pub fn render(ctx: &egui::Context, theme: &Theme, state: &mut AppState) {
    if state.install.phase == InstallPhase::Idle {
        return;
    }

    let palette = theme.palette;
    let mut dismiss = false;
    let mut cancel = false;
    let mut retry = false;

    egui::Modal::new(egui::Id::new("install-overlay"))
        .backdrop_color(palette.scrim)
        .frame(
            egui::Frame::new()
                .fill(palette.surface)
                .stroke(Stroke::new(1.0, palette.border))
                .corner_radius(theme.radius_lg())
                .inner_margin(egui::Margin::same(0))
                .shadow(egui::epaint::Shadow {
                    offset: [0, 20],
                    blur: 48,
                    spread: 0,
                    color: egui::Color32::from_black_alpha(160),
                }),
        )
        .show(ctx, |ui| {
            ui.set_width(CARD_WIDTH);

            header(ui, theme, state);

            if let Some(progress) = state.install.progress.clone() {
                egui::Frame::new()
                    .inner_margin(egui::Margin {
                        left: 22,
                        right: 22,
                        top: 16,
                        bottom: 4,
                    })
                    .show(ui, |ui| {
                        ui.set_width(CARD_WIDTH - 44.0);
                        widgets::progress_bar(
                            ui,
                            progress.fraction(),
                            &progress.label,
                            &progress.summary(),
                        );
                    });
            }

            egui::Frame::new()
                .inner_margin(egui::Margin::symmetric(22, 18))
                .show(ui, |ui| {
                    ui.set_width(CARD_WIDTH - 44.0);
                    ui.spacing_mut().item_spacing.y = 0.0;

                    let last = state.install.stages.len();
                    for (index, row) in state.install.stages.iter().enumerate() {
                        stage_row(ui, theme, row, index + 1, index + 1 == last);
                    }
                });

            if let Some(failure) = state.install.failure.clone() {
                egui::Frame::new()
                    .inner_margin(egui::Margin {
                        left: 22,
                        right: 22,
                        top: 0,
                        bottom: 16,
                    })
                    .show(ui, |ui| {
                        ui.set_width(CARD_WIDTH - 44.0);
                        let tone = if failure.cancelled {
                            feedback::Tone::Warning
                        } else {
                            feedback::Tone::Danger
                        };
                        widgets::banner(
                            ui,
                            tone,
                            &failure.message,
                            failure.hint.as_deref().unwrap_or(""),
                        );
                    });
            }

            egui::Frame::new()
                .fill(palette.surface_alt)
                .corner_radius(egui::CornerRadius {
                    nw: 0,
                    ne: 0,
                    sw: theme.metrics.radius_lg,
                    se: theme.metrics.radius_lg,
                })
                .inner_margin(egui::Margin::symmetric(22, 14))
                .show(ui, |ui| {
                    ui.set_width(CARD_WIDTH - 44.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!(
                                "Elapsed {}",
                                format::elapsed(state.install.elapsed())
                            ))
                            .font(theme::text_style(theme::size::MICRO))
                            .color(palette.text_faint),
                        );

                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            match state.install.phase {
                                InstallPhase::Running => {
                                    let stopping = state.install.cancel_requested();
                                    cancel = widgets::Button::new(if stopping {
                                        "Stopping"
                                    } else {
                                        "Cancel"
                                    })
                                    .tone(widgets::Tone::Ghost)
                                    .enabled(!stopping)
                                    .show(ui)
                                    .clicked();
                                }
                                InstallPhase::Succeeded => {
                                    dismiss = widgets::Button::primary("Done")
                                        .min_width(96.0)
                                        .show(ui)
                                        .clicked();
                                }
                                _ => {
                                    retry = widgets::Button::primary("Try again")
                                        .icon(Icon::Refresh)
                                        .show(ui)
                                        .clicked();
                                    dismiss = widgets::Button::new("Close")
                                        .tone(widgets::Tone::Ghost)
                                        .show(ui)
                                        .clicked();
                                }
                            }
                        });
                    });
                });
        });

    if state.install.phase.is_busy() {
        ctx.request_repaint_after(std::time::Duration::from_millis(80));
    }

    if cancel {
        state.cancel_install();
    }
    if dismiss {
        state.dismiss_install();
    }
    if retry {
        state.dismiss_install();
        state.install_roblox(false);
    }
}

fn header(ui: &mut egui::Ui, theme: &Theme, state: &AppState) {
    let palette = theme.palette;

    let framed = egui::Frame::new()
        .inner_margin(egui::Margin {
            left: 22,
            right: 22,
            top: 20,
            bottom: 16,
        })
        .show(ui, |ui| {
            ui.set_width(CARD_WIDTH - 44.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 14.0;

                let (badge, _) = ui.allocate_exact_size(Vec2::splat(46.0), Sense::hover());
                let (tint, icon) = match state.install.phase {
                    InstallPhase::Succeeded => (palette.success, Some(Icon::Check)),
                    InstallPhase::Failed => (palette.danger, Some(Icon::Cross)),
                    InstallPhase::Cancelled => (palette.warning, Some(Icon::Cross)),
                    _ => (palette.accent, None),
                };

                ui.painter()
                    .circle_filled(badge.center(), 23.0, tint.gamma_multiply(0.14));

                match icon {
                    Some(icon) => icons::draw(ui.painter(), icon, badge.shrink(13.0), tint, 2.4),
                    None => {
                        let time = ui.input(|input| input.time);
                        icons::ring(ui.painter(), badge.shrink(10.0), palette.border_strong, 2.4);
                        icons::spinner(ui.painter(), badge.shrink(10.0), tint, 2.4, time);
                    }
                }

                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new(state.install.headline())
                            .font(theme::strong(theme::size::TITLE))
                            .color(palette.text),
                    );
                    let subline = state.install.subline();
                    if !subline.is_empty() {
                        ui.add_space(2.0);
                        ui.label(
                            egui::RichText::new(subline)
                                .font(theme::text_style(theme::size::SMALL))
                                .color(palette.text_muted),
                        );
                    }
                });
            });
        });

    let rect = framed.response.rect;
    ui.painter().hline(
        rect.x_range(),
        rect.bottom(),
        Stroke::new(1.0, palette.border),
    );
}

fn stage_row(ui: &mut egui::Ui, theme: &Theme, row: &StageRow, index: usize, last: bool) {
    let palette = theme.palette;
    let height = if row.detail.is_some() { 44.0 } else { 34.0 };
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::hover());

    let marker_rect = Rect::from_center_size(
        egui::pos2(rect.left() + 11.0, rect.top() + 16.0),
        Vec2::splat(22.0),
    );

    let marker = match row.state {
        StageState::Pending => MarkerState::Pending,
        StageState::Active => MarkerState::Active,
        StageState::Done => MarkerState::Done,
        StageState::Failed => MarkerState::Failed,
    };
    widgets::step_marker(ui, marker_rect, marker, index);

    if !last {
        let color = if row.state == StageState::Done {
            palette.success.gamma_multiply(0.35)
        } else {
            palette.border
        };
        ui.painter().vline(
            marker_rect.center().x,
            egui::Rangef::new(marker_rect.bottom() + 2.0, rect.bottom() + 4.0),
            Stroke::new(1.5, color),
        );
    }

    let text_color = match row.state {
        StageState::Pending => palette.text_faint,
        StageState::Failed => palette.danger,
        StageState::Active => palette.text,
        StageState::Done => palette.text_muted,
    };
    let font = if row.state == StageState::Active {
        theme::medium(theme::size::BODY)
    } else {
        theme::text_style(theme::size::BODY)
    };

    ui.painter().text(
        egui::pos2(marker_rect.right() + 12.0, rect.top() + 16.0),
        Align2::LEFT_CENTER,
        row.stage.title(),
        font,
        text_color,
    );

    if let Some(detail) = &row.detail {
        ui.painter().text(
            egui::pos2(marker_rect.right() + 12.0, rect.top() + 32.0),
            Align2::LEFT_CENTER,
            detail,
            theme::text_style(theme::size::MICRO),
            palette.text_faint,
        );
    }
}

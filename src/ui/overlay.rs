use egui::{Align, Align2, Layout, Rect, Sense, Stroke, Vec2};

use crate::app::{AppState, Phase};
use crate::roblox::launch::{Step, StepState};
use crate::util::format;

use super::icons::{self, Icon};
use super::theme::{self, Theme};
use super::widgets::{self, feedback, MarkerState};
use super::UiState;

const CARD_WIDTH: f32 = 470.0;

pub fn launch_overlay(
    ctx: &egui::Context,
    theme: &Theme,
    state: &mut AppState,
    _ui_state: &mut UiState,
) {
    if state.session.phase == Phase::Idle {
        return;
    }

    let palette = theme.palette;
    let mut dismiss = false;
    let mut cancel = false;
    let mut retry = false;

    let modal = egui::Modal::new(egui::Id::new("launch-overlay"))
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
        );

    modal.show(ctx, |ui| {
        ui.set_width(CARD_WIDTH);

        header(ui, theme, state);

        egui::Frame::new()
            .inner_margin(egui::Margin::symmetric(22, 18))
            .show(ui, |ui| {
                ui.set_width(CARD_WIDTH - 44.0);
                ui.spacing_mut().item_spacing.y = 0.0;

                for (index, step) in state.session.steps.iter().enumerate() {
                    step_row(
                        ui,
                        theme,
                        step,
                        index + 1,
                        index + 1 == state.session.steps.len(),
                    );
                }
            });

        if let Some(failure) = state.session.failure.clone() {
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

        if let Some(note) = state
            .session
            .report
            .as_ref()
            .and_then(|report| report.note.clone())
        {
            egui::Frame::new()
                .inner_margin(egui::Margin {
                    left: 22,
                    right: 22,
                    top: 0,
                    bottom: 16,
                })
                .show(ui, |ui| {
                    ui.set_width(CARD_WIDTH - 44.0);
                    widgets::banner(ui, feedback::Tone::Info, &note, "");
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
                            format::elapsed(state.session.elapsed())
                        ))
                        .font(theme::text_style(theme::size::MICRO))
                        .color(palette.text_faint),
                    );

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        match state.session.phase {
                            Phase::Running => {
                                let stopping = state.session.cancel_requested();
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
                            Phase::Succeeded => {
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

    if state.session.phase.is_busy() {
        ctx.request_repaint_after(std::time::Duration::from_millis(80));
    }

    if cancel {
        state.cancel_launch();
    }
    if dismiss {
        state.dismiss_launch();
    }
    if retry {
        if let Some(target) = state.session.target.clone() {
            state.dismiss_launch();
            state.launch(target);
        }
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
                let (tint, icon) = match state.session.phase {
                    Phase::Succeeded => (palette.success, Some(Icon::Check)),
                    Phase::Failed => (palette.danger, Some(Icon::Cross)),
                    Phase::Cancelled => (palette.warning, Some(Icon::Cross)),
                    _ => (palette.accent, None),
                };

                ui.painter()
                    .circle_filled(badge.center(), 23.0, tint.gamma_multiply(0.14));

                match icon {
                    Some(icon) => {
                        icons::draw(ui.painter(), icon, badge.shrink(13.0), tint, 2.4);
                    }
                    None => {
                        let time = ui.input(|input| input.time);
                        icons::ring(ui.painter(), badge.shrink(10.0), palette.border_strong, 2.4);
                        icons::spinner(ui.painter(), badge.shrink(10.0), tint, 2.4, time);
                    }
                }

                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new(state.session.headline())
                            .font(theme::strong(theme::size::TITLE))
                            .color(palette.text),
                    );
                    let subline = state.session.subline();
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

fn step_row(ui: &mut egui::Ui, theme: &Theme, step: &Step, index: usize, last: bool) {
    let palette = theme.palette;
    let height = if step.detail.is_some() { 44.0 } else { 34.0 };
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::hover());

    let marker_rect = Rect::from_center_size(
        egui::pos2(rect.left() + 11.0, rect.top() + 16.0),
        Vec2::splat(22.0),
    );

    let marker = match step.state {
        StepState::Pending => MarkerState::Pending,
        StepState::Active => MarkerState::Active,
        StepState::Done => MarkerState::Done,
        StepState::Skipped => MarkerState::Skipped,
        StepState::Failed => MarkerState::Failed,
    };
    widgets::step_marker(ui, marker_rect, marker, index);

    if !last {
        let color = if matches!(step.state, StepState::Done | StepState::Skipped) {
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

    let text_color = match step.state {
        StepState::Pending => palette.text_faint,
        StepState::Failed => palette.danger,
        StepState::Active => palette.text,
        _ => palette.text_muted,
    };
    let font = if step.state == StepState::Active {
        theme::medium(theme::size::BODY)
    } else {
        theme::text_style(theme::size::BODY)
    };

    ui.painter().text(
        egui::pos2(marker_rect.right() + 12.0, rect.top() + 16.0),
        Align2::LEFT_CENTER,
        step.id.title(),
        font,
        text_color,
    );

    if let Some(detail) = &step.detail {
        ui.painter().text(
            egui::pos2(marker_rect.right() + 12.0, rect.top() + 32.0),
            Align2::LEFT_CENTER,
            detail,
            theme::text_style(theme::size::MICRO),
            palette.text_faint,
        );
    }
}

pub fn confirm_dialog(
    ctx: &egui::Context,
    theme: &Theme,
    state: &mut AppState,
    ui_state: &mut UiState,
) {
    let Some(target) = ui_state.confirm.clone() else {
        return;
    };

    let palette = theme.palette;
    let mut confirmed = false;
    let mut cancelled = false;

    let response = egui::Modal::new(egui::Id::new("confirm-launch"))
        .backdrop_color(palette.scrim)
        .frame(
            egui::Frame::new()
                .fill(palette.surface)
                .stroke(Stroke::new(1.0, palette.border))
                .corner_radius(theme.radius_lg())
                .inner_margin(egui::Margin::same(22)),
        )
        .show(ctx, |ui| {
            ui.set_width(360.0);
            ui.label(
                egui::RichText::new("Start Roblox?")
                    .font(theme::strong(theme::size::TITLE))
                    .color(palette.text),
            );
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new(target.headline())
                    .font(theme::medium(theme::size::BODY))
                    .color(palette.accent),
            );
            ui.add_space(2.0);
            ui.label(
                egui::RichText::new(target.detail())
                    .font(theme::text_style(theme::size::SMALL))
                    .color(palette.text_muted),
            );
            ui.add_space(theme.metrics.gap_lg);

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                confirmed = widgets::Button::primary("Launch")
                    .icon(Icon::Rocket)
                    .show(ui)
                    .clicked();
                cancelled = widgets::Button::new("Cancel")
                    .tone(widgets::Tone::Ghost)
                    .show(ui)
                    .clicked();
            });
        });

    if response.should_close() {
        cancelled = true;
    }

    if confirmed {
        ui_state.confirm = None;
        state.launch(target);
    } else if cancelled {
        ui_state.confirm = None;
    }
}

use egui::{Align, Layout, Rect, Sense, Ui, Vec2};

use crate::app::{AppState, FlowStage, FlowStatus};

use super::theme::{self, Theme};
use super::widgets;
use super::{appicon, UiState};

pub const WIDTH: f32 = 400.0;
pub const HEIGHT: f32 = 246.0;

const LOGO: f32 = 44.0;
const BUTTON: f32 = 118.0;

pub fn render(ui: &mut Ui, theme: &Theme, state: &mut AppState, ui_state: &mut UiState) {
    let palette = theme.palette;
    let status: FlowStatus = state.flow_status();
    let stage = state.flow.stage;

    let mut cancel = false;
    let mut retry = false;
    let mut dismiss = false;

    ui.with_layout(Layout::bottom_up(Align::Center), |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        let buttons = if stage == FlowStage::Failed { 2 } else { 1 };
        let row = BUTTON * buttons as f32 + theme.metrics.gap_sm * (buttons - 1) as f32;

        ui.allocate_ui_with_layout(
            Vec2::new(row, theme.metrics.button_h),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.spacing_mut().item_spacing.x = theme.metrics.gap_sm;
                match stage {
                    FlowStage::Failed => {
                        retry = widgets::Button::primary("Try again")
                            .min_width(BUTTON)
                            .show(ui)
                            .clicked();
                        dismiss = widgets::Button::new("Close")
                            .tone(widgets::Tone::Neutral)
                            .min_width(BUTTON)
                            .show(ui)
                            .clicked();
                    }
                    FlowStage::Finished => {
                        dismiss = widgets::Button::primary("Done")
                            .min_width(BUTTON)
                            .show(ui)
                            .clicked();
                    }
                    _ => {
                        let stopping = stage == FlowStage::Preparing
                            && state.install.cancel_requested()
                            || stage == FlowStage::Launching && state.session.cancel_requested();
                        cancel = widgets::Button::new(if stopping { "Stopping" } else { "Cancel" })
                            .tone(widgets::Tone::Neutral)
                            .min_width(BUTTON)
                            .enabled(!stopping)
                            .show(ui)
                            .clicked();
                    }
                }
            },
        );

        ui.add_space(theme.metrics.gap_md);

        ui.with_layout(Layout::top_down(Align::Center), |ui| {
            ui.add_space(theme.metrics.gap_xs);

            let (logo, _) = ui.allocate_exact_size(Vec2::splat(LOGO), Sense::hover());
            appicon::paint(ui, logo, theme.radius_md());

            ui.add_space(theme.metrics.gap_sm);
            ui.label(
                egui::RichText::new(&status.headline)
                    .font(theme::medium(theme::size::TITLE))
                    .color(palette.text),
            );

            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(if status.detail.is_empty() {
                    " ".to_string()
                } else {
                    crate::util::format::truncate_middle(&status.detail, 46)
                })
                .font(theme::text_style(theme::size::SMALL))
                .color(palette.text_muted),
            );

            ui.add_space(theme.metrics.gap_sm);
            track(ui, theme, stage, status.progress);
        });
    });

    if stage.is_busy() {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(60));
    }

    if stage == FlowStage::Finished {
        state.close_requested = true;
    }
    if cancel {
        state.cancel_flow();
    }
    if retry {
        state.retry_flow();
    }
    if dismiss {
        leave(state, ui_state);
    }
}

fn leave(state: &mut AppState, ui_state: &mut UiState) {
    state.dismiss_flow();
    ui_state.shell = ui_state.return_shell;
}

fn track(ui: &mut Ui, theme: &Theme, stage: FlowStage, progress: Option<f32>) {
    let palette = theme.palette;
    let width = ui.available_width().min(300.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 8.0), Sense::hover());
    let radius = egui::CornerRadius::same(4);

    ui.painter()
        .rect_filled(rect, radius, palette.surface_press);

    let tint = match stage {
        FlowStage::Failed => palette.danger,
        FlowStage::Finished => palette.success,
        _ => palette.accent,
    };

    match progress {
        Some(fraction) => {
            let eased = ui.ctx().animate_value_with_time(
                ui.id().with("splash-progress"),
                fraction.clamp(0.0, 1.0),
                theme.anim(0.18),
            );
            if eased > 0.0 {
                let filled = Rect::from_min_size(
                    rect.min,
                    Vec2::new((rect.width() * eased).max(8.0), rect.height()),
                );
                ui.painter().rect_filled(filled, radius, tint);
            }
        }
        None if stage == FlowStage::Failed => {}
        None => {
            let span = rect.width() * 0.34;
            let offset = if theme.metrics.animations {
                let time = ui.input(|input| input.time) as f32;
                let sweep = (time * 0.55).fract() * (rect.width() + span) - span;
                ui.ctx().request_repaint();
                sweep
            } else {
                (rect.width() - span) / 2.0
            };

            let left = (rect.left() + offset).max(rect.left());
            let right = (rect.left() + offset + span).min(rect.right());
            if right > left {
                ui.painter().rect_filled(
                    Rect::from_min_max(
                        egui::pos2(left, rect.top()),
                        egui::pos2(right, rect.bottom()),
                    ),
                    radius,
                    tint,
                );
            }
        }
    }
}

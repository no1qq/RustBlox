use egui::{Align, Align2, Layout, Rect, Sense, Stroke, StrokeKind, Ui, Vec2, ViewportCommand};

use crate::app::AppState;
use crate::uninstall;

use super::icons::{self, Icon};
use super::theme::{self, Theme};
use super::widgets;
use super::{appicon, Page, Shell, UiState};

pub const WIDTH: f32 = 430.0;
pub const HEIGHT: f32 = 250.0;

enum Choice {
    Launch,
    Configure,
    Uninstall,
}

pub fn title_bar(ui: &mut Ui, theme: &Theme, state: &mut AppState) {
    let palette = theme.palette;
    let rect = ui.max_rect();

    let drag = ui.interact(
        Rect::from_min_max(
            rect.left_top(),
            egui::pos2(rect.right() - 46.0, rect.bottom()),
        ),
        egui::Id::new("launcher-drag"),
        Sense::click_and_drag(),
    );
    if drag.drag_started_by(egui::PointerButton::Primary) {
        ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
    }

    let mark = Rect::from_min_size(
        egui::pos2(rect.left() + 12.0, rect.center().y - 8.0),
        Vec2::splat(16.0),
    );
    appicon::paint(ui, mark, theme.radius_sm());

    ui.painter().text(
        egui::pos2(mark.right() + 10.0, rect.center().y - 1.0),
        Align2::LEFT_CENTER,
        "RustBlox",
        theme::medium(theme::size::SMALL),
        palette.text_muted,
    );

    ui.scope_builder(
        egui::UiBuilder::new().max_rect(Rect::from_min_max(
            egui::pos2(rect.right() - 46.0, rect.top()),
            rect.right_bottom(),
        )),
        |ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                if super::chrome::window_button(ui, theme, Icon::Close, true).clicked() {
                    state.close_requested = true;
                }
            });
        },
    );
}

pub fn render(ui: &mut Ui, theme: &Theme, state: &mut AppState, ui_state: &mut UiState) {
    let ready = state.detection.active().is_some();
    let busy = !state.can_launch();
    let mut choice = None;

    ui.spacing_mut().item_spacing.y = theme.metrics.gap_sm;

    if entry(
        ui,
        theme,
        Icon::Rocket,
        "Launch Roblox",
        if ready {
            None
        } else {
            Some("Roblox is not installed yet")
        },
        ready && !busy,
        true,
    ) {
        choice = Some(Choice::Launch);
    }

    if entry(
        ui,
        theme,
        Icon::Sliders,
        "Configure settings",
        None,
        true,
        false,
    ) {
        choice = Some(Choice::Configure);
    }

    if entry(
        ui,
        theme,
        Icon::Trash,
        "Uninstall RustBlox",
        Some("Removes the files it created"),
        true,
        false,
    ) {
        choice = Some(Choice::Uninstall);
    }

    match choice {
        Some(Choice::Launch) => {
            let target = state.default_target();
            super::chrome::request_launch(state, ui_state, target);
        }
        Some(Choice::Configure) => {
            ui_state.shell = Shell::Full;
            if !ready {
                ui_state.page = Page::Installation;
            }
        }
        Some(Choice::Uninstall) => ui_state.uninstall = Some(false),
        None => {}
    }
}

fn entry(
    ui: &mut Ui,
    theme: &Theme,
    icon: Icon,
    label: &str,
    detail: Option<&str>,
    enabled: bool,
    primary: bool,
) -> bool {
    let palette = theme.palette;
    let height = if detail.is_some() { 54.0 } else { 48.0 };
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), height),
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );

    if !ui.is_rect_visible(rect) {
        return false;
    }

    let hover = ui
        .ctx()
        .animate_bool_with_time(response.id.with("hover"), response.hovered(), 0.1);

    let fill = if primary {
        let base = palette.accent;
        if hover > 0.0 {
            palette.accent_hover
        } else {
            base
        }
    } else {
        let base = palette.surface;
        if hover > 0.0 {
            palette.surface_hover
        } else {
            base
        }
    };

    let fill = if enabled {
        fill
    } else {
        palette.surface.gamma_multiply(0.7)
    };

    ui.painter().rect_filled(rect, theme.radius_sm(), fill);
    if !primary {
        ui.painter().rect_stroke(
            rect,
            theme.radius_sm(),
            Stroke::new(1.0, palette.border),
            StrokeKind::Inside,
        );
    }

    let ink = if !enabled {
        palette.text_faint
    } else if primary {
        palette.on_accent
    } else {
        palette.text
    };
    let sub = if !enabled {
        palette.text_faint
    } else if primary {
        palette.on_accent.gamma_multiply(0.75)
    } else {
        palette.text_muted
    };

    let icon_rect = Rect::from_min_size(
        egui::pos2(rect.left() + 18.0, rect.center().y - 9.0),
        Vec2::splat(18.0),
    );
    icons::draw(ui.painter(), icon, icon_rect, ink, 1.7);

    let text_x = icon_rect.right() + 14.0;
    match detail {
        Some(detail) => {
            ui.painter().text(
                egui::pos2(text_x, rect.center().y - 8.0),
                Align2::LEFT_CENTER,
                label,
                theme::medium(theme::size::BODY),
                ink,
            );
            ui.painter().text(
                egui::pos2(text_x, rect.center().y + 9.0),
                Align2::LEFT_CENTER,
                detail,
                theme::text_style(theme::size::MICRO),
                sub,
            );
        }
        None => {
            ui.painter().text(
                egui::pos2(text_x, rect.center().y),
                Align2::LEFT_CENTER,
                label,
                theme::medium(theme::size::BODY),
                ink,
            );
        }
    }

    icons::draw(
        ui.painter(),
        Icon::ChevronRight,
        Rect::from_center_size(
            egui::pos2(rect.right() - 22.0, rect.center().y),
            Vec2::splat(15.0),
        ),
        sub,
        1.6,
    );

    if enabled && response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    enabled && response.clicked()
}

pub fn uninstall_dialog(
    ctx: &egui::Context,
    theme: &Theme,
    state: &mut AppState,
    ui_state: &mut UiState,
) {
    let Some(mut remove_settings) = ui_state.uninstall else {
        return;
    };

    let mut go = false;
    let mut cancel = false;

    let dismissed = super::overlay::modal(ctx, theme, "uninstall", 420.0, |ui| {
        ui.label(
            egui::RichText::new("Uninstall RustBlox")
                .font(theme::medium(theme::size::TITLE))
                .color(theme.palette.text),
        );
        ui.add_space(theme.metrics.gap_sm);
        ui.label(
            egui::RichText::new(
                "This removes every Roblox copy RustBlox installed, its logs, its saved state and its flag profile. Roblox itself, and any copy Roblox installed, is left alone.",
            )
            .font(theme::text_style(theme::size::SMALL))
            .color(theme.palette.text_muted),
        );

        ui.add_space(theme.metrics.gap_md);
        widgets::separator(ui);
        ui.add_space(theme.metrics.gap_sm);

        for path in uninstall::targets(state.store.paths(), plan(remove_settings)) {
            ui.label(
                egui::RichText::new(path.display().to_string())
                    .font(egui::FontId::new(
                        theme::size::MICRO,
                        egui::FontFamily::Monospace,
                    ))
                    .color(theme.palette.text_faint),
            );
        }
        ui.label(
            egui::RichText::new(
                state
                    .exe_path
                    .as_ref()
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|| "the RustBlox program".into()),
            )
            .font(egui::FontId::new(
                theme::size::MICRO,
                egui::FontFamily::Monospace,
            ))
            .color(theme.palette.text_faint),
        );

        ui.add_space(theme.metrics.gap_md);
        widgets::checkbox_row(
            ui,
            &mut remove_settings,
            "Also delete my settings",
            "Keeping them means a reinstall starts where you left off.",
        );

        ui.add_space(theme.metrics.gap_lg);
        ui.horizontal(|ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = theme.metrics.gap_sm;
                go = widgets::Button::danger("Uninstall")
                    .icon(Icon::Trash)
                    .show(ui)
                    .clicked();
                cancel = widgets::Button::new("Keep RustBlox")
                    .tone(widgets::Tone::Ghost)
                    .show(ui)
                    .clicked();
            });
        });
    });

    ui_state.uninstall = Some(remove_settings);

    if cancel || dismissed {
        ui_state.uninstall = None;
    }
    if go {
        ui_state.uninstall = None;
        state.uninstall(remove_settings);
    }
}

fn plan(remove_settings: bool) -> uninstall::Plan {
    uninstall::Plan {
        remove_settings,
        remove_executable: true,
    }
}

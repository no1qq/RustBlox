use egui::{Align, Layout, Sense, Vec2};

use crate::app::AppState;
use crate::config::{LaunchOutcome, QuickTarget};
use crate::roblox::launch::LaunchTarget;
use crate::roblox::uri;
use crate::util::format;

use crate::ui::chrome::request_launch;
use crate::ui::icons::Icon;
use crate::ui::theme::{self, Theme};
use crate::ui::widgets::{self, feedback};
use crate::ui::{Page, UiState};

pub fn render(ui: &mut egui::Ui, state: &mut AppState, ui_state: &mut UiState) {
    let theme = Theme::get(ui.ctx());
    let installed = state.detection.active().is_some();

    let mut rescan = false;
    widgets::page_header(ui, "Home", "Launch Roblox and watch the client.", |ui| {
        rescan = widgets::Button::new("Rescan")
            .icon(Icon::Refresh)
            .tone(widgets::Tone::Ghost)
            .enabled(!state.tasks.is_scanning())
            .show(ui)
            .clicked();
    });
    if rescan {
        state.rescan();
    }

    if !installed {
        missing_install(ui, &theme, state, ui_state);
        return;
    }

    hero(ui, &theme, state, ui_state);
    ui.add_space(theme.metrics.gap_lg);
    quick_launch(ui, &theme, state, ui_state);
    ui.add_space(theme.metrics.gap_lg);
    activity(ui, &theme, state);
}

fn missing_install(ui: &mut egui::Ui, theme: &Theme, state: &mut AppState, ui_state: &mut UiState) {
    let scanning = state.tasks.is_scanning();
    let can_install = state.can_install();
    let mut go_to_installation = false;
    let mut pick = false;
    let mut install = false;

    widgets::card(ui, |ui| {
        widgets::empty_state(
            ui,
            Icon::Package,
            if scanning {
                "Looking for Roblox"
            } else {
                "Roblox was not found"
            },
            if scanning {
                "Checking the usual install locations on this machine."
            } else {
                "RustBlox checked the standard Roblox folders and the registered launch handler without finding a client. RustBlox can download and install it for you, or point it at a copy you already have."
            },
            |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = theme.metrics.gap_sm;
                    install = widgets::Button::primary("Install Roblox")
                        .icon(Icon::Package)
                        .enabled(!scanning && can_install)
                        .show(ui)
                        .clicked();
                    pick = widgets::Button::new("Choose a folder")
                        .icon(Icon::Folder)
                        .enabled(!scanning)
                        .show(ui)
                        .clicked();
                    go_to_installation = widgets::Button::new("Details")
                        .tone(widgets::Tone::Ghost)
                        .show(ui)
                        .clicked();
                });
            },
        );
    });

    if install {
        state.install_roblox(false);
    }
    if pick {
        if let Some(folder) = state.pick_install_folder() {
            state.settings.advanced.custom_install_root = Some(folder);
            state.mark_settings_dirty();
            state.flush_settings();
            state.rescan();
        }
    }
    if go_to_installation {
        ui_state.page = Page::Installation;
    }
}

fn hero(ui: &mut egui::Ui, theme: &Theme, state: &mut AppState, ui_state: &mut UiState) {
    let palette = theme.palette;
    let running = state.roblox.player_running();
    let can_launch = state.can_launch();

    let install = state.detection.active().cloned();
    let Some(install) = install else { return };

    let mut launch_default = false;
    let mut launch_app = false;
    let mut open_installation = false;
    let update = state.update_available().cloned();

    widgets::card(ui, |ui| {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = theme.metrics.gap_lg;

            ui.vertical(|ui| {
                let width = (ui.available_width() - 210.0).max(220.0);
                ui.set_width(width);

                ui.horizontal(|ui| {
                    let (tone, label) = if running {
                        (feedback::Tone::Success, state.roblox.summary())
                    } else {
                        (feedback::Tone::Neutral, "Client idle".to_string())
                    };
                    widgets::status_pill(ui, &label, tone, running);
                    widgets::badge(ui, install.source.short(), feedback::Tone::Accent);
                });

                ui.add_space(theme.metrics.gap_md);
                ui.label(
                    egui::RichText::new("Roblox is ready")
                        .font(theme::strong(theme::size::DISPLAY))
                        .color(palette.text),
                );
                ui.add_space(3.0);
                ui.label(
                    egui::RichText::new(format!(
                        "Version {} - {}",
                        install.display_version(),
                        install.source.label()
                    ))
                    .font(theme::text_style(theme::size::SMALL))
                    .color(palette.text_muted),
                );

                if let Some(update) = &update {
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        widgets::badge(ui, "Update", feedback::Tone::Accent);
                        ui.label(
                            egui::RichText::new(format!("Roblox {} is available", update.version))
                                .font(theme::text_style(theme::size::SMALL))
                                .color(palette.text_muted),
                        );
                        if widgets::Button::new("Install it")
                            .tone(widgets::Tone::Quiet)
                            .size(widgets::Size::Small)
                            .show(ui)
                            .clicked()
                        {
                            open_installation = true;
                        }
                    });
                }

                ui.add_space(theme.metrics.gap_lg);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = theme.metrics.gap_sm;
                    launch_default = widgets::Button::primary("Launch Roblox")
                        .icon(Icon::Rocket)
                        .size(widgets::Size::Large)
                        .enabled(can_launch)
                        .min_width(190.0)
                        .show(ui)
                        .clicked();

                    launch_app = widgets::Button::new("Open home screen")
                        .tone(widgets::Tone::Ghost)
                        .size(widgets::Size::Large)
                        .enabled(can_launch)
                        .show(ui)
                        .clicked();
                });
            });

            ui.with_layout(Layout::top_down(Align::Min), |ui| {
                ui.set_width(ui.available_width().max(160.0));
                widgets::nested(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = theme.metrics.gap_sm;
                    widgets::detail_row(ui, "Version", install.display_version(), true);
                    widgets::detail_row(ui, "Folder", &install.folder_id, true);
                    widgets::detail_row(
                        ui,
                        "Launches",
                        &state.persisted.launch_count.to_string(),
                        false,
                    );
                    widgets::detail_row(
                        ui,
                        "Flags",
                        &if state.settings.advanced.apply_flag_profile {
                            format!("{} on launch", state.flags.active_count())
                        } else {
                            "Not applied".to_string()
                        },
                        false,
                    );
                });
            });
        });
    });

    if launch_default {
        let target = state.default_target();
        request_launch(state, ui_state, target);
    }
    if launch_app {
        request_launch(state, ui_state, LaunchTarget::App);
    }
    if open_installation {
        ui_state.page = Page::Installation;
    }
}

fn quick_launch(ui: &mut egui::Ui, theme: &Theme, state: &mut AppState, ui_state: &mut UiState) {
    let can_launch = state.can_launch();
    let mut launch: Option<LaunchTarget> = None;
    let mut remove: Option<u64> = None;
    let mut add = false;

    widgets::section(
        ui,
        "Quick launch",
        Some("Opened through the Roblox deep link handler."),
        |ui| {
            if state.settings.launch.quick_targets.is_empty() {
                widgets::nested(ui, |ui| {
                    ui.label(
                        egui::RichText::new(
                            "No saved places yet. Add a place ID or a roblox.com game link below.",
                        )
                        .font(theme::text_style(theme::size::SMALL))
                        .color(theme.palette.text_muted),
                    );
                });
                ui.add_space(theme.metrics.gap_md);
            } else {
                for target in state.settings.launch.quick_targets.clone() {
                    widgets::nested(ui, |ui| {
                        ui.horizontal(|ui| {
                            let (icon_rect, _) =
                                ui.allocate_exact_size(Vec2::splat(18.0), Sense::hover());
                            crate::ui::icons::draw(
                                ui.painter(),
                                Icon::Play,
                                icon_rect,
                                theme.palette.accent,
                                1.6,
                            );

                            ui.vertical(|ui| {
                                ui.set_width((ui.available_width() - 160.0).max(120.0));
                                ui.label(
                                    egui::RichText::new(&target.name)
                                        .font(theme::medium(theme::size::BODY))
                                        .color(theme.palette.text),
                                );
                                ui.label(
                                    egui::RichText::new(format!("Place {}", target.place_id))
                                        .font(theme::text_style(theme::size::MICRO))
                                        .color(theme.palette.text_faint),
                                );
                            });

                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if widgets::icon_button(ui, Icon::Trash, "Remove", true).clicked() {
                                    remove = Some(target.place_id);
                                }
                                if widgets::Button::new("Launch")
                                    .size(widgets::Size::Small)
                                    .enabled(can_launch)
                                    .show(ui)
                                    .clicked()
                                {
                                    launch = Some(LaunchTarget::Place {
                                        place_id: target.place_id,
                                        label: Some(target.name.clone()),
                                    });
                                }
                            });
                        });
                    });
                    ui.add_space(theme.metrics.gap_sm);
                }
            }

            widgets::separator(ui);
            ui.add_space(theme.metrics.gap_md);

            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = theme.metrics.gap_sm;
                let field_width = ((ui.available_width() - 200.0) * 0.5).clamp(120.0, 260.0);
                widgets::text_field(ui, &mut ui_state.quick_name, "Name", field_width);
                widgets::text_field(
                    ui,
                    &mut ui_state.quick_input,
                    "Place ID or game link",
                    field_width,
                );
                add = widgets::Button::new("Add")
                    .icon(Icon::Plus)
                    .show(ui)
                    .clicked();
            });

            if let Some(error) = &ui_state.quick_error {
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(error)
                        .font(theme::text_style(theme::size::SMALL))
                        .color(theme.palette.danger),
                );
            }
        },
    );

    if add {
        match uri::parse_place_input(&ui_state.quick_input) {
            Ok(place_id) => {
                let name = if ui_state.quick_name.trim().is_empty() {
                    format!("Place {place_id}")
                } else {
                    ui_state.quick_name.trim().to_owned()
                };

                if state
                    .settings
                    .launch
                    .quick_targets
                    .iter()
                    .any(|entry| entry.place_id == place_id)
                {
                    ui_state.quick_error = Some("That place is already saved.".into());
                } else {
                    state
                        .settings
                        .launch
                        .quick_targets
                        .push(QuickTarget { name, place_id });
                    state.mark_settings_dirty();
                    state.flush_settings();
                    ui_state.quick_input.clear();
                    ui_state.quick_name.clear();
                    ui_state.quick_error = None;
                    state.toasts.success("Quick launch entry added");
                }
            }
            Err(err) => ui_state.quick_error = Some(err.to_string()),
        }
    }

    if let Some(place_id) = remove {
        state
            .settings
            .launch
            .quick_targets
            .retain(|entry| entry.place_id != place_id);
        state.mark_settings_dirty();
        state.flush_settings();
    }

    if let Some(target) = launch {
        request_launch(state, ui_state, target);
    }
}

fn activity(ui: &mut egui::Ui, theme: &Theme, state: &mut AppState) {
    let mut open_folder = None;

    widgets::section(ui, "Activity", None, |ui| {
        match &state.persisted.last_launch {
            Some(record) => {
                let tone = match record.outcome {
                    LaunchOutcome::Succeeded => feedback::Tone::Success,
                    LaunchOutcome::Failed => feedback::Tone::Danger,
                    LaunchOutcome::Cancelled => feedback::Tone::Warning,
                };

                ui.horizontal(|ui| {
                    widgets::badge(ui, record.outcome.label(), tone);
                    ui.label(
                        egui::RichText::new(&record.target)
                            .font(theme::medium(theme::size::BODY))
                            .color(theme.palette.text),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new(format::relative_time(record.at))
                                .font(theme::text_style(theme::size::SMALL))
                                .color(theme.palette.text_muted),
                        )
                        .on_hover_text(format::timestamp(record.at));
                    });
                });

                if let Some(detail) = &record.detail {
                    ui.add_space(6.0);
                    ui.label(
                        egui::RichText::new(detail)
                            .font(theme::text_style(theme::size::SMALL))
                            .color(theme.palette.text_muted),
                    );
                }
            }
            None => {
                ui.label(
                    egui::RichText::new("Nothing has been launched from RustBlox yet.")
                        .font(theme::text_style(theme::size::SMALL))
                        .color(theme.palette.text_muted),
                );
            }
        }

        ui.add_space(theme.metrics.gap_md);
        widgets::separator(ui);
        ui.add_space(theme.metrics.gap_md);

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = theme.metrics.gap_xl;
            widgets::stat(
                ui,
                "Total launches",
                &state.persisted.launch_count.to_string(),
                feedback::Tone::Accent,
            );
            widgets::stat(
                ui,
                "Installs found",
                &state.detection.installations.len().to_string(),
                feedback::Tone::Neutral,
            );
            widgets::stat(
                ui,
                "Saved places",
                &state.settings.launch.quick_targets.len().to_string(),
                feedback::Tone::Neutral,
            );

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if widgets::Button::new("Open data folder")
                    .icon(Icon::Folder)
                    .tone(widgets::Tone::Ghost)
                    .size(widgets::Size::Small)
                    .show(ui)
                    .clicked()
                {
                    open_folder = Some(state.store.paths().data_dir().to_path_buf());
                }
            });
        });
    });

    if let Some(path) = open_folder {
        state.open_path(path);
    }
}

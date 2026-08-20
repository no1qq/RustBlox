use egui::{Align, Layout};

use crate::app::AppState;
use crate::config::{Accent, AppearanceSettings, Density, StartupTarget, Theme as ThemeChoice};

use crate::ui::icons::Icon;
use crate::ui::theme::{self, Theme};
use crate::ui::widgets::{self, feedback, Segmented};
use crate::ui::{SettingsTab, UiState};

pub fn render(ui: &mut egui::Ui, state: &mut AppState, ui_state: &mut UiState) {
    let theme = Theme::get(ui.ctx());

    widgets::page_header(
        ui,
        "Settings",
        "Every option here changes real behaviour.",
        |ui| {
            if state.settings_pending() {
                widgets::badge(ui, "Saving", feedback::Tone::Accent);
            }
        },
    );

    let tabs: Vec<(SettingsTab, &str)> = SettingsTab::ALL
        .iter()
        .map(|tab| (*tab, tab.label()))
        .collect();
    Segmented::new(&tabs).show(ui, &mut ui_state.settings_tab);
    ui.add_space(theme.metrics.gap_lg);

    match ui_state.settings_tab {
        SettingsTab::General => general(ui, &theme, state),
        SettingsTab::Launch => launch(ui, &theme, state),
        SettingsTab::Appearance => appearance(ui, &theme, state),
        SettingsTab::Advanced => advanced(ui, &theme, state, ui_state),
    }
}

fn general(ui: &mut egui::Ui, theme: &Theme, state: &mut AppState) {
    let mut changed = false;
    let mut open_config = false;
    let mut open_data = false;
    let mut reset = false;

    widgets::section(ui, "After a launch", None, |ui| {
        widgets::setting_row(
            ui,
            "Minimise RustBlox once Roblox is running",
            "Drops the launcher to the taskbar as soon as the client reports it started.",
            |ui| {
                changed |=
                    widgets::toggle(ui, &mut state.settings.launch.hide_window_on_launch).changed();
            },
        );
        ui.add_space(theme.metrics.gap_md);
        widgets::setting_row(
            ui,
            "Close RustBlox once Roblox is running",
            "Frees the launcher from memory as soon as the client reports it started.",
            |ui| {
                changed |=
                    widgets::toggle(ui, &mut state.settings.launch.close_after_launch).changed();
            },
        );
    });

    ui.add_space(theme.metrics.gap_lg);

    widgets::section(
        ui,
        "Stored data",
        Some("Settings live in a per-user folder, never next to the executable unless you pass --portable."),
        |ui| {
            widgets::detail_row(
                ui,
                "Settings",
                &state.store.paths().settings_file().display().to_string(),
                true,
            );
            ui.add_space(theme.metrics.gap_sm);
            widgets::detail_row(
                ui,
                "State and logs",
                &state.store.paths().data_dir().display().to_string(),
                true,
            );

            ui.add_space(theme.metrics.gap_md);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = theme.metrics.gap_sm;
                open_config = widgets::Button::new("Open settings folder")
                    .icon(Icon::Folder)
                    .size(widgets::Size::Small)
                    .show(ui)
                    .clicked();
                open_data = widgets::Button::new("Open data folder")
                    .icon(Icon::Folder)
                    .size(widgets::Size::Small)
                    .show(ui)
                    .clicked();
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    reset = widgets::Button::danger("Reset to defaults")
                        .size(widgets::Size::Small)
                        .show(ui)
                        .clicked();
                });
            });
        },
    );

    if changed {
        state.mark_settings_dirty();
    }
    if open_config {
        let path = state.store.paths().config_dir().to_path_buf();
        state.open_path(path);
    }
    if open_data {
        let path = state.store.paths().data_dir().to_path_buf();
        state.open_path(path);
    }
    if reset {
        state.settings = crate::config::Settings::default();
        state.mark_settings_dirty();
        state.flush_settings();
        state.toasts.success("Settings reset to defaults");
        state.rescan();
    }
}

fn launch(ui: &mut egui::Ui, theme: &Theme, state: &mut AppState) {
    let mut changed = false;

    widgets::section(
        ui,
        "Startup",
        Some("What the Launch button does when no specific place is chosen."),
        |ui| {
            let options: Vec<(StartupTarget, &str)> = StartupTarget::ALL
                .iter()
                .map(|target| (*target, target.label()))
                .collect();

            widgets::setting_row(
                ui,
                "Default target",
                state.settings.launch.startup_target.description(),
                |ui| {
                    changed |= Segmented::new(&options)
                        .show(ui, &mut state.settings.launch.startup_target)
                        .changed();
                },
            );
        },
    );

    ui.add_space(theme.metrics.gap_lg);

    widgets::section(ui, "Safety checks", None, |ui| {
        widgets::setting_row(
            ui,
            "Ask before launching",
            "Shows a short confirmation with the target before anything starts.",
            |ui| {
                changed |=
                    widgets::toggle(ui, &mut state.settings.launch.confirm_before_launch).changed();
            },
        );
        ui.add_space(theme.metrics.gap_md);
        widgets::setting_row(
            ui,
            "Stop if a client is already running",
            "Turn this off to allow more than one Roblox window. Roblox itself may still refuse.",
            |ui| {
                changed |=
                    widgets::toggle(ui, &mut state.settings.launch.warn_when_already_running)
                        .changed();
            },
        );
        ui.add_space(theme.metrics.gap_md);
        widgets::setting_row(
            ui,
            "How long to wait for the client",
            "RustBlox watches for the client process for this long before reporting that it could not confirm the launch.",
            |ui| {
                changed |= widgets::stepper(
                    ui,
                    &mut state.settings.launch.launch_timeout_secs,
                    5..=120,
                    "s",
                );
            },
        );
    });

    ui.add_space(theme.metrics.gap_lg);

    let count = state.settings.launch.quick_targets.len();
    widgets::section(
        ui,
        "Quick launch entries",
        Some("Manage these from the Home page."),
        |ui| {
            ui.label(
                egui::RichText::new(match count {
                    0 => "No places saved.".to_string(),
                    1 => "1 place saved.".to_string(),
                    n => format!("{n} places saved."),
                })
                .font(theme::text_style(theme::size::SMALL))
                .color(theme.palette.text_muted),
            );
        },
    );

    if changed {
        state.mark_settings_dirty();
    }
}

fn appearance(ui: &mut egui::Ui, theme: &Theme, state: &mut AppState) {
    let mut changed = false;

    widgets::section(ui, "Theme", None, |ui| {
        let options: Vec<(ThemeChoice, &str)> = ThemeChoice::ALL
            .iter()
            .map(|choice| (*choice, choice.label()))
            .collect();
        widgets::setting_row(ui, "Colour scheme", "Applies immediately.", |ui| {
            changed |= Segmented::new(&options)
                .show(ui, &mut state.settings.appearance.theme)
                .changed();
        });

        ui.add_space(theme.metrics.gap_md);
        widgets::setting_row(
            ui,
            "Accent",
            "Used for primary actions and highlights.",
            |ui| {
                let mut selected = state.settings.appearance.accent;
                for accent in Accent::ALL.iter().rev() {
                    if accent_swatch(ui, theme, *accent, selected == *accent) {
                        selected = *accent;
                        changed = true;
                    }
                }
                state.settings.appearance.accent = selected;
            },
        );
    });

    ui.add_space(theme.metrics.gap_lg);

    widgets::section(ui, "Layout", None, |ui| {
        let options: Vec<(Density, &str)> = Density::ALL
            .iter()
            .map(|density| (*density, density.label()))
            .collect();
        widgets::setting_row(
            ui,
            "Density",
            "Compact tightens spacing throughout the interface.",
            |ui| {
                changed |= Segmented::new(&options)
                    .show(ui, &mut state.settings.appearance.density)
                    .changed();
            },
        );

        ui.add_space(theme.metrics.gap_md);
        widgets::setting_row(
            ui,
            "Interface scale",
            &format!(
                "{:.0}% of the default size.",
                state.settings.appearance.ui_scale * 100.0
            ),
            |ui| {
                changed |= widgets::slider(
                    ui,
                    &mut state.settings.appearance.ui_scale,
                    AppearanceSettings::MIN_SCALE..=AppearanceSettings::MAX_SCALE,
                )
                .changed();
            },
        );

        ui.add_space(theme.metrics.gap_md);
        widgets::setting_row(
            ui,
            "Motion",
            "Turn off to remove hover, selection and launch animations.",
            |ui| {
                changed |= widgets::toggle(ui, &mut state.settings.appearance.animations).changed();
            },
        );
    });

    if changed {
        state.mark_settings_dirty();
    }
}

fn accent_swatch(ui: &mut egui::Ui, theme: &Theme, accent: Accent, selected: bool) -> bool {
    let [r, g, b] = accent.rgb();
    let color = egui::Color32::from_rgb(r, g, b);
    let (rect, response) = ui.allocate_exact_size(egui::Vec2::splat(26.0), egui::Sense::click());

    let grow = ui
        .ctx()
        .animate_bool_with_time(response.id.with("hover"), response.hovered(), 0.1);

    ui.painter()
        .circle_filled(rect.center(), 9.0 + grow * 1.0, color);

    if selected {
        ui.painter().circle_stroke(
            rect.center(),
            12.0,
            egui::Stroke::new(2.0, theme.palette.text),
        );
    }

    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    response.on_hover_text(accent.label()).clicked()
}

fn advanced(ui: &mut egui::Ui, theme: &Theme, state: &mut AppState, ui_state: &mut UiState) {
    let mut changed = false;
    let mut clear_pin = false;

    widgets::section(
        ui,
        "Launch pipeline",
        Some("These run as steps in the launch panel."),
        |ui| {
            widgets::setting_row(
                ui,
                "Check the install before launching",
                "Confirms the player executable and content folders exist. Turning this off skips the check.",
                |ui| {
                    changed |=
                        widgets::toggle(ui, &mut state.settings.advanced.verify_before_launch)
                            .changed();
                },
            );
            ui.add_space(theme.metrics.gap_md);
            widgets::setting_row(
                ui,
                "Write the flag profile on every launch",
                "Keeps the client in sync with the Flags page, including after a Roblox update.",
                |ui| {
                    changed |= widgets::toggle(ui, &mut state.settings.advanced.apply_flag_profile)
                        .changed();
                },
            );
        },
    );

    ui.add_space(theme.metrics.gap_lg);

    let pinned = state.settings.advanced.pinned_version_folder.clone();
    widgets::section(ui, "Version pinning", None, |ui| {
        widgets::setting_row(
            ui,
            "Pinned version folder",
            &pinned
                .clone()
                .unwrap_or_else(|| "Not pinned, the newest install is used".into()),
            |ui| {
                if pinned.is_some()
                    && widgets::Button::new("Unpin")
                        .tone(widgets::Tone::Ghost)
                        .size(widgets::Size::Small)
                        .show(ui)
                        .clicked()
                {
                    clear_pin = true;
                }
            },
        );
    });

    ui.add_space(theme.metrics.gap_lg);

    widgets::section(
        ui,
        "Extra player arguments",
        Some("Appended after the launch argument. Leave empty unless you know what a switch does."),
        |ui| {
            let buffer = ui_state
                .extra_args_buffer
                .get_or_insert_with(|| state.settings.advanced.extra_player_arguments.clone());

            let response =
                widgets::text_field(ui, buffer, "--fullscreen", ui.available_width() - 4.0);
            if response.changed() {
                state.settings.advanced.extra_player_arguments = buffer.clone();
                changed = true;
            }

            let parsed = crate::roblox::launch::split_arguments(buffer);
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new(if parsed.is_empty() {
                    "No extra arguments.".to_string()
                } else {
                    format!("Parsed as: {}", parsed.join("  "))
                })
                .font(egui::FontId::new(
                    theme::size::MICRO,
                    egui::FontFamily::Monospace,
                ))
                .color(theme.palette.text_faint),
            );
        },
    );

    ui.add_space(theme.metrics.gap_lg);

    widgets::section(
        ui,
        "Downloading Roblox",
        Some("Used by the install and update buttons on the Installation page."),
        |ui| {
            let buffer = ui_state
                .channel_buffer
                .get_or_insert_with(|| state.settings.advanced.channel.clone());

            widgets::setting_row(
                ui,
                "Release channel",
                "LIVE is the public build. Other channels are Roblox internal names and may not exist.",
                |ui| {
                    if widgets::text_field(ui, buffer, "LIVE", 180.0).changed()
                        && crate::roblox::deploy::is_valid_channel(buffer)
                    {
                        state.settings.advanced.channel = buffer.trim().to_owned();
                        changed = true;
                    }
                },
            );

            ui.add_space(theme.metrics.gap_md);
            widgets::setting_row(
                ui,
                "Keep downloaded packages",
                "Leaves the zip files on disk so a reinstall does not download them again.",
                |ui| {
                    changed |=
                        widgets::toggle(ui, &mut state.settings.advanced.keep_downloads).changed();
                },
            );
        },
    );

    ui.add_space(theme.metrics.gap_lg);

    widgets::section(ui, "Diagnostics", None, |ui| {
        widgets::setting_row(
            ui,
            "Keep a launch log",
            "Writes each step and failure to the log file for later inspection.",
            |ui| {
                if widgets::toggle(ui, &mut state.settings.advanced.keep_launch_logs).changed() {
                    crate::util::log::set_file_logging(state.settings.advanced.keep_launch_logs);
                    changed = true;
                }
            },
        );
    });

    if clear_pin {
        state.settings.advanced.pinned_version_folder = None;
        changed = true;
        state.rescan();
    }
    if changed {
        state.mark_settings_dirty();
    }
}

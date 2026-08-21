use egui::{Align, Layout};

use crate::app::AppState;
use crate::config::{Accent, AppearanceSettings, Density, StartupTarget, ThemeMode};

use crate::ui::icons::Icon;
use crate::ui::theme::{self, Theme};
use crate::ui::widgets::{self, feedback, Segmented};
use crate::ui::{SettingsTab, UiState};

const SCALE_STEP: f32 = 0.05;

pub fn render(ui: &mut egui::Ui, state: &mut AppState, ui_state: &mut UiState) {
    let theme = Theme::get(ui.ctx());
    let advanced = state.settings.advanced_mode;

    widgets::page_header(ui, "Settings", "Every change saves itself.", |ui| {
        if state.settings_pending() {
            widgets::badge(ui, "Saving", feedback::Tone::Accent);
        }
    });

    let tabs: Vec<(SettingsTab, &str)> = SettingsTab::visible(advanced)
        .iter()
        .map(|tab| (*tab, tab.label()))
        .collect();
    if !tabs.iter().any(|(tab, _)| *tab == ui_state.settings_tab) {
        ui_state.settings_tab = SettingsTab::General;
    }
    Segmented::new(&tabs).show(ui, &mut ui_state.settings_tab);
    ui.add_space(theme.metrics.gap_lg);

    match ui_state.settings_tab {
        SettingsTab::General => general(ui, &theme, state),
        SettingsTab::Launch => launch(ui, &theme, state),
        SettingsTab::Appearance => appearance(ui, &theme, state),
        SettingsTab::Advanced => advanced_tab(ui, &theme, state, ui_state),
    }
}

fn general(ui: &mut egui::Ui, theme: &Theme, state: &mut AppState) {
    let mut changed = false;
    let mut open_config = false;
    let mut open_data = false;
    let mut reset = false;

    widgets::section(ui, "How RustBlox behaves", None, |ui| {
        widgets::setting_row(
            ui,
            "Keep Roblox up to date",
            "Checks for a newer Roblox every time you press Launch and installs it first.",
            |ui| {
                changed |= widgets::toggle(ui, &mut state.settings.launch.update_roblox_on_launch)
                    .changed();
            },
        );
        ui.add_space(theme.metrics.gap_md);
        widgets::setting_row(
            ui,
            "Advanced options",
            "Shows the Flags page, the Advanced tab and the extra install controls.",
            |ui| {
                changed |= widgets::toggle(ui, &mut state.settings.advanced_mode).changed();
            },
        );
    });

    ui.add_space(theme.metrics.gap_lg);

    widgets::section(
        ui,
        "Where your files live",
        Some("Per user, unless RustBlox was started with --portable."),
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
                "Roblox, logs and state",
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
    let advanced = state.settings.advanced_mode;
    let count = state.settings.launch.quick_targets.len();

    widgets::section(
        ui,
        "What Launch opens",
        Some("Used by the launcher window and by --launch."),
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

            ui.add_space(theme.metrics.gap_md);
            widgets::setting_row(
                ui,
                "Ask before launching",
                "Shows a short confirmation naming the target before anything starts.",
                |ui| {
                    changed |=
                        widgets::toggle(ui, &mut state.settings.launch.confirm_before_launch)
                            .changed();
                },
            );

            ui.add_space(theme.metrics.gap_md);
            ui.label(
                egui::RichText::new(match count {
                    0 => "No quick launch places saved yet. Add them on the Home page.".to_string(),
                    1 => "1 quick launch place saved, managed on the Home page.".to_string(),
                    n => format!("{n} quick launch places saved, managed on the Home page."),
                })
                .font(theme::text_style(theme::size::SMALL))
                .color(theme.palette.text_muted),
            );
        },
    );

    ui.add_space(theme.metrics.gap_lg);

    widgets::section(ui, "Once Roblox is running", None, |ui| {
        widgets::setting_row(
            ui,
            "Close RustBlox",
            "The launcher window always closes. This covers launches started from this window.",
            |ui| {
                changed |=
                    widgets::toggle(ui, &mut state.settings.launch.close_after_launch).changed();
            },
        );
        ui.add_space(theme.metrics.gap_md);
        widgets::setting_row(
            ui,
            "Minimise RustBlox",
            "Drops this window to the taskbar instead of closing it.",
            |ui| {
                changed |=
                    widgets::toggle(ui, &mut state.settings.launch.hide_window_on_launch).changed();
            },
        );
    });

    if advanced {
        ui.add_space(theme.metrics.gap_lg);

        widgets::section(ui, "Safety checks", None, |ui| {
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
                "RustBlox watches for the client process for this long before saying it could not confirm the launch.",
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
    }

    if changed {
        state.mark_settings_dirty();
    }
}

fn appearance(ui: &mut egui::Ui, theme: &Theme, state: &mut AppState) {
    let mut changed = false;

    let mode = state.settings.appearance.mode;
    let following = if state.system_dark {
        "Windows is set to dark"
    } else {
        "Windows is set to light"
    };

    widgets::section(ui, "Colours", None, |ui| {
        let options: Vec<(ThemeMode, &str)> = ThemeMode::ALL
            .iter()
            .map(|choice| (*choice, choice.label()))
            .collect();
        widgets::setting_row(
            ui,
            "Light or dark",
            if mode == ThemeMode::Auto {
                following
            } else {
                mode.detail()
            },
            |ui| {
                changed |= Segmented::new(&options)
                    .show(ui, &mut state.settings.appearance.mode)
                    .changed();
            },
        );

        ui.add_space(theme.metrics.gap_md);
        widgets::setting_row(ui, "Accent", "Colours the main buttons.", |ui| {
            let mut selected = state.settings.appearance.accent;
            for accent in Accent::ALL.iter().rev() {
                if accent_swatch(ui, theme, *accent, selected == *accent) {
                    selected = *accent;
                    changed = true;
                }
            }
            state.settings.appearance.accent = selected;
        });
    });

    ui.add_space(theme.metrics.gap_lg);

    widgets::section(ui, "Size and motion", None, |ui| {
        let options: Vec<(Density, &str)> = Density::ALL
            .iter()
            .map(|density| (*density, density.label()))
            .collect();
        widgets::setting_row(ui, "Density", "Compact tightens the spacing.", |ui| {
            changed |= Segmented::new(&options)
                .show(ui, &mut state.settings.appearance.density)
                .changed();
        });

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
                    SCALE_STEP,
                )
                .changed();
            },
        );

        ui.add_space(theme.metrics.gap_md);
        widgets::setting_row(
            ui,
            "Motion",
            "Turn off to remove every hover, selection and progress animation.",
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

    let grow = ui.ctx().animate_bool_with_time(
        response.id.with("hover"),
        response.hovered(),
        theme.anim(0.1),
    );

    ui.painter().circle_filled(rect.center(), 9.0 + grow, color);

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

fn advanced_tab(ui: &mut egui::Ui, theme: &Theme, state: &mut AppState, ui_state: &mut UiState) {
    let mut changed = false;
    let mut clear_pin = false;

    widgets::banner(
        ui,
        feedback::Tone::Warning,
        "These change how launches work",
        "The defaults suit almost everyone. Nothing here is needed to play.",
    );
    ui.add_space(theme.metrics.gap_lg);

    widgets::section(ui, "Launch pipeline", None, |ui| {
        widgets::setting_row(
            ui,
            "Check the install before launching",
            "Confirms the player and content folders exist before starting the client.",
            |ui| {
                changed |= widgets::toggle(ui, &mut state.settings.advanced.verify_before_launch)
                    .changed();
            },
        );
        ui.add_space(theme.metrics.gap_md);
        widgets::setting_row(
            ui,
            "Write the flag profile on every launch",
            "On by default. Turn it off and flags only reach the client when you press Write now on the Flags page.",
            |ui| {
                changed |=
                    widgets::toggle(ui, &mut state.settings.advanced.apply_flag_profile).changed();
            },
        );
        ui.add_space(theme.metrics.gap_md);

        let buffer = ui_state
            .extra_args_buffer
            .get_or_insert_with(|| state.settings.advanced.extra_player_arguments.clone());
        widgets::setting_row(
            ui,
            "Extra player arguments",
            "Appended after the launch argument. Leave empty unless you know what you need.",
            |ui| {
                if widgets::text_field(ui, buffer, "--fullscreen", 240.0).changed() {
                    state.settings.advanced.extra_player_arguments = buffer.clone();
                    changed = true;
                }
            },
        );

        let parsed = crate::roblox::launch::split_arguments(buffer);
        if !parsed.is_empty() {
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new(format!("Parsed as: {}", parsed.join("  ")))
                    .font(egui::FontId::new(
                        theme::size::MICRO,
                        egui::FontFamily::Monospace,
                    ))
                    .color(theme.palette.text_faint),
            );
        }
    });

    ui.add_space(theme.metrics.gap_lg);

    let pinned = state.settings.advanced.pinned_version_folder.clone();
    widgets::section(ui, "Downloading Roblox", None, |ui| {
        let buffer = ui_state
            .channel_buffer
            .get_or_insert_with(|| state.settings.advanced.channel.clone());

        widgets::setting_row(
            ui,
            "Release channel",
            "LIVE is the public build. Other names are Roblox internal and may not exist.",
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
            "Leaves the zip files on disk so a reinstall does not fetch them again.",
            |ui| {
                changed |=
                    widgets::toggle(ui, &mut state.settings.advanced.keep_downloads).changed();
            },
        );

        ui.add_space(theme.metrics.gap_md);
        widgets::setting_row(
            ui,
            "Pinned version folder",
            &pinned
                .clone()
                .unwrap_or_else(|| "Not pinned, the newest copy is used".into()),
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

use egui::{Align, Layout};

use crate::app::AppState;
use crate::config::{Accent, AppearanceSettings, Density, StartupTarget, ThemeMode};

use crate::ui::icons::Icon;
use crate::ui::theme::{self, Theme};
use crate::ui::widgets::{self, feedback, Segmented};
use crate::ui::{SettingsTab, UiState};

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
        SettingsTab::General => general(ui, &theme, state, ui_state),
        SettingsTab::Launch => launch(ui, &theme, state),
        SettingsTab::Appearance => appearance(ui, &theme, state, ui_state),
        SettingsTab::Advanced => advanced_tab(ui, &theme, state, ui_state),
    }

    add_account_dialog(ui.ctx(), &theme, state, ui_state);
}

fn general(ui: &mut egui::Ui, theme: &Theme, state: &mut AppState, ui_state: &mut UiState) {
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
            "Shows the Flags, Mods, Shortcuts and Installation pages, the Advanced tab and the extra launch settings.",
            |ui| {
                changed |= widgets::toggle(ui, &mut state.settings.advanced_mode).changed();
            },
        );
    });

    ui.add_space(theme.metrics.gap_lg);

    accounts_section(ui, theme, state, ui_state);

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
                "Watch what you are playing",
                "Reads the game you are in out of the client's own log, on this PC only. Nothing is sent anywhere.",
                |ui| {
                    changed |=
                        widgets::toggle(ui, &mut state.settings.launch.track_activity).changed();
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
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new("RustBlox closes itself once the client is running.")
                    .font(theme::text_style(theme::size::SMALL))
                    .color(theme.palette.text_muted),
            );
        },
    );

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
            ui.add_space(theme.metrics.gap_md);
            widgets::setting_row(
                ui,
                "Multiple Roblox instances",
                "Allows opening multiple Roblox clients concurrently without singleton mutex conflicts.",
                |ui| {
                    changed |=
                        widgets::toggle(ui, &mut state.settings.launch.multi_instance).changed();
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

        ui.add_space(theme.metrics.gap_lg);

        widgets::section(
            ui,
            "TheWatcher Anti-Cheat",
            Some("Active in the background when Roblox runs from RustBlox."),
            |ui| {
                widgets::setting_row(
                    ui,
                    "Protection status",
                    "Enforced and active. Flags external cheat tools, DLL injection, and script executors while Roblox runs.",
                    |ui| {
                        widgets::badge(ui, "Protected", feedback::Tone::Success);
                    },
                );
            },
        );
    }

    if advanced {
        ui.add_space(theme.metrics.gap_lg);
        changed |= discord(ui, theme, state);
    }

    if changed {
        state.mark_settings_dirty();
    }
}

fn discord(ui: &mut egui::Ui, theme: &Theme, state: &mut AppState) -> bool {
    let mut changed = false;
    let mut restart = false;
    let mut open_portal = false;
    let mut reset_id = false;
    let status = state.presence_status();
    let usable = state.settings.discord.is_usable();
    let built_in = state.settings.discord.is_built_in();

    widgets::section(
        ui,
        "Discord",
        Some("Shows what you are playing on your Discord profile, using the application you registered."),
        |ui| {
            widgets::setting_row(
                ui,
                "Show what you are playing",
                "RustBlox stays open while Roblox runs so it can keep the status up to date, and closes with it. Discord has to be running too.",
                |ui| {
                    if widgets::toggle(ui, &mut state.settings.discord.enabled).changed() {
                        changed = true;
                        restart = true;
                    }
                },
            );

            ui.add_space(theme.metrics.gap_md);
            widgets::setting_row(
                ui,
                "Streamer & privacy mode",
                "Hides the specific game and place details from Discord Rich Presence.",
                |ui| {
                    if widgets::toggle(ui, &mut state.settings.discord.streamer_mode).changed() {
                        changed = true;
                        restart = true;
                    }
                },
            );

            ui.add_space(theme.metrics.gap_md);
            widgets::setting_row(
                ui,
                "Application ID",
                "The Discord application whose name shows as the game. RustBlox comes with one, so this only needs changing if you would rather use your own.",
                |ui| {
                    if !built_in
                        && widgets::Button::new("Reset")
                            .tone(widgets::Tone::Ghost)
                            .size(widgets::Size::Small)
                            .show(ui)
                            .on_hover_text("Reset to the built-in Application ID")
                            .clicked()
                    {
                        reset_id = true;
                    } else if built_in {
                        widgets::badge(ui, "built in", feedback::Tone::Neutral);
                    }
                    if widgets::text_field(
                        ui,
                        &mut state.settings.discord.application_id,
                        "18 digits",
                        140.0,
                    )
                    .changed()
                    {
                        changed = true;
                        restart = true;
                    }
                },
            );

            ui.add_space(theme.metrics.gap_md);
            widgets::setting_row(
                ui,
                "Look up the game's name",
                "Asks roblox.com what the place you are in is called. Without it the status says the place ID.",
                |ui| {
                    if widgets::toggle(ui, &mut state.settings.discord.show_place_name).changed() {
                        changed = true;
                    }
                },
            );

            ui.add_space(theme.metrics.gap_md);
            widgets::nested(ui, |ui| {
                ui.horizontal(|ui| {
                    widgets::badge(
                        ui,
                        &status.label(),
                        if status.is_failed() {
                            feedback::Tone::Danger
                        } else if usable {
                            feedback::Tone::Success
                        } else {
                            feedback::Tone::Neutral
                        },
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        open_portal = widgets::Button::new("Make an application")
                            .icon(Icon::External)
                            .tone(widgets::Tone::Ghost)
                            .size(widgets::Size::Small)
                            .show(ui)
                            .clicked();
                    });
                });
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(
                        "Discord shows the name of the application, not the name of RustBlox. \
                         To use your own instead, make one at the Discord developer portal, \
                         name it whatever you want shown, and paste its Application ID above.",
                    )
                    .font(theme::text_style(theme::size::SMALL))
                    .color(theme.palette.text_muted),
                );
            });
        },
    );

    if reset_id {
        state.settings.discord.application_id = crate::discord::DEFAULT_APPLICATION_ID.to_owned();
        changed = true;
        restart = true;
    }
    if restart {
        state.flush_settings();
        state.refresh_presence();
    }
    if open_portal {
        state.open_url("https://discord.com/developers/applications");
    }

    changed
}

fn appearance(ui: &mut egui::Ui, theme: &Theme, state: &mut AppState, ui_state: &mut UiState) {
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
        let buffer = ui_state
            .scale_buffer
            .get_or_insert_with(|| format!("{:.0}", state.settings.appearance.ui_scale * 100.0));
        widgets::setting_row(
            ui,
            "Interface scale",
            "Between 50% and 200% of the default size.",
            |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = theme.metrics.gap_xs;
                    if widgets::text_field(ui, buffer, "100", 65.0).changed() {
                        let cleaned = buffer.trim().trim_end_matches('%').trim();
                        if let Ok(val) = cleaned.parse::<f32>() {
                            let clamped = (val / 100.0).clamp(
                                AppearanceSettings::MIN_SCALE,
                                AppearanceSettings::MAX_SCALE,
                            );
                            state.settings.appearance.ui_scale = clamped;
                            changed = true;
                        }
                    }
                    ui.label(
                        egui::RichText::new("%")
                            .font(theme::medium(theme::size::BODY))
                            .color(theme.palette.text_muted),
                    );
                });
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
            "On by default. Turn it off and flags only reach the client when you press Write to the client now on the Flags page.",
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

fn accounts_section(
    ui: &mut egui::Ui,
    theme: &Theme,
    state: &mut AppState,
    ui_state: &mut UiState,
) {
    widgets::section(
        ui,
        "Roblox Accounts & Switcher",
        Some("Switch between saved Roblox accounts without re-logging in."),
        |ui| {
            if state.accounts.is_empty() {
                widgets::nested(ui, |ui| {
                    ui.label(
                        egui::RichText::new(
                            "No accounts saved yet. Add an account using your .ROBLOSECURITY cookie.",
                        )
                        .font(theme::text_style(theme::size::SMALL))
                        .color(theme.palette.text_muted),
                    );
                });
            } else {
                let mut remove_id = None;
                let mut switch_id = None;

                for acc in &state.accounts {
                    let is_active = state.active_account_id == Some(acc.id);
                    widgets::nested(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new(&acc.display_name)
                                            .font(theme::strong(theme::size::BODY))
                                            .color(theme.palette.text),
                                    );
                                    ui.label(
                                        egui::RichText::new(format!("@{}", acc.username))
                                            .font(theme::text_style(theme::size::SMALL))
                                            .color(theme.palette.text_muted),
                                    );
                                    if is_active {
                                        widgets::badge(ui, "Active", feedback::Tone::Success);
                                    }
                                });
                                ui.label(
                                    egui::RichText::new(format!(
                                        "ID: {} • Added: {}",
                                        acc.id, acc.created_at
                                    ))
                                    .font(theme::text_style(theme::size::SMALL))
                                    .color(theme.palette.text_muted),
                                );
                            });

                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if widgets::Button::new("Remove")
                                    .tone(widgets::Tone::Danger)
                                    .size(widgets::Size::Small)
                                    .show(ui)
                                    .clicked()
                                {
                                    remove_id = Some(acc.id);
                                }
                                if !is_active
                                    && widgets::Button::new("Switch")
                                        .tone(widgets::Tone::Primary)
                                        .size(widgets::Size::Small)
                                        .show(ui)
                                        .clicked()
                                {
                                    switch_id = Some(acc.id);
                                }
                            });
                        });
                    });
                    ui.add_space(theme.metrics.gap_xs);
                }

                if let Some(id) = remove_id {
                    state.remove_account(id);
                }
                if let Some(id) = switch_id {
                    state.switch_account(id);
                }
            }

            ui.add_space(theme.metrics.gap_md);
            if widgets::Button::new("Add Account")
                .icon(Icon::Plus)
                .tone(widgets::Tone::Neutral)
                .size(widgets::Size::Small)
                .show(ui)
                .clicked()
            {
                ui_state.account_cookie_input.clear();
                ui_state.account_error = None;
                ui_state.show_add_account_dialog = true;
            }
        },
    );
}

fn add_account_dialog(
    ctx: &egui::Context,
    theme: &Theme,
    state: &mut AppState,
    ui_state: &mut UiState,
) {
    if !ui_state.show_add_account_dialog {
        return;
    }

    let palette = theme.palette;
    let mut close = false;
    let mut add_cookie = None;

    let response = egui::Modal::new(egui::Id::new("add-account-modal"))
        .backdrop_color(palette.scrim)
        .frame(
            egui::Frame::new()
                .fill(palette.surface)
                .stroke(egui::Stroke::new(1.0, palette.border))
                .corner_radius(theme.radius_lg())
                .inner_margin(egui::Margin::same(22)),
        )
        .show(ctx, |ui| {
            ui.set_width(460.0);

            ui.label(
                egui::RichText::new("Add Roblox Account")
                    .font(theme::strong(theme::size::TITLE))
                    .color(palette.text),
            );
            ui.add_space(theme.metrics.gap_xs);
            ui.label(
                egui::RichText::new(
                    "Paste your .ROBLOSECURITY cookie from your browser to authenticate.",
                )
                .font(theme::text_style(theme::size::SMALL))
                .color(palette.text_muted),
            );

            ui.add_space(theme.metrics.gap_md);
            ui.label(
                egui::RichText::new("Cookie (.ROBLOSECURITY):")
                    .font(theme::strong(theme::size::BODY))
                    .color(palette.text),
            );
            ui.add_space(theme.metrics.gap_xs);
            widgets::text_field(
                ui,
                &mut ui_state.account_cookie_input,
                "_|WARNING:-DO-NOT-SHARE-THIS.--...",
                420.0,
            );

            if let Some(err) = &ui_state.account_error {
                ui.add_space(theme.metrics.gap_xs);
                ui.label(
                    egui::RichText::new(err)
                        .font(theme::text_style(theme::size::SMALL))
                        .color(palette.danger),
                );
            }

            ui.add_space(theme.metrics.gap_lg);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = theme.metrics.gap_sm;
                if widgets::Button::primary("Save Account")
                    .size(widgets::Size::Small)
                    .show(ui)
                    .clicked()
                {
                    add_cookie = Some(ui_state.account_cookie_input.clone());
                }
                if widgets::Button::new("Cancel")
                    .tone(widgets::Tone::Neutral)
                    .size(widgets::Size::Small)
                    .show(ui)
                    .clicked()
                {
                    close = true;
                }
            });
        });

    if let Some(cookie) = add_cookie {
        if cookie.trim().is_empty() {
            ui_state.account_error = Some("Cookie cannot be empty".into());
        } else {
            state.add_account(&cookie);
            let sanitized = crate::roblox::account::sanitize_cookie(&cookie);
            if state.accounts.iter().any(|acc| acc.cookie == sanitized) {
                ui_state.show_add_account_dialog = false;
                ui_state.account_cookie_input.clear();
                ui_state.account_error = None;
            } else {
                ui_state.account_error = Some("Could not authenticate with this cookie".into());
            }
        }
    }

    if response.should_close() || close {
        ui_state.show_add_account_dialog = false;
        ui_state.account_error = None;
    }
}

use egui::{Align, Layout};

use crate::app::AppState;
use crate::roblox::flags::{self, FlagValue};

use crate::ui::icons::Icon;
use crate::ui::theme::{self, Theme};
use crate::ui::widgets::{self, feedback};
use crate::ui::UiState;

enum Action {
    Add,
    Remove(String),
    Toggle(String),
    Edit { key: String, value: String },
    ToggleApply,
    WriteNow,
    AskReset,
    Reset,
    CancelReset,
    OpenRaw,
    CloseRaw,
    CommitRaw,
    Copied,
    SwitchProfile(String),
    OpenNewProfile,
    CloseNewProfile,
    CreateProfile { name: String, clone: bool },
    DeleteProfile(String),
    ApplyPreset(usize),
}

pub fn render(ui: &mut egui::Ui, state: &mut AppState, ui_state: &mut UiState) {
    let theme = Theme::get(ui.ctx());
    let mut action = None;
    state.refresh_denied_flags();

    widgets::page_header(
        ui,
        "FFlags",
        "Applied as soon as you change them, and written again before Roblox starts.",
        |ui| {
            if widgets::Button::primary("Reset flags")
                .icon(Icon::Trash)
                .enabled(!state.flags.entries.is_empty())
                .show(ui)
                .clicked()
            {
                action = Some(Action::AskReset);
            }
            if widgets::Button::new("Import or export")
                .icon(Icon::Copy)
                .tone(widgets::Tone::Ghost)
                .show(ui)
                .clicked()
            {
                action = Some(if ui_state.raw_editor.is_some() {
                    Action::CloseRaw
                } else {
                    Action::OpenRaw
                });
            }
        },
    );

    profile_selector(ui, &theme, state, ui_state, &mut action);
    ui.add_space(theme.metrics.gap_lg);

    presets_section(ui, &theme, state, &mut action);
    ui.add_space(theme.metrics.gap_lg);

    let refused = state.denied_active_flags();
    if !refused.is_empty() {
        widgets::banner(
            ui,
            feedback::Tone::Danger,
            "Roblox refused some of these on the last launch",
            &format!(
                "The client read the file and then ignored {}. Anything marked refused below came back the same way.",
                refused.join(", ")
            ),
        );
        ui.add_space(theme.metrics.gap_lg);
    }

    applying(ui, &theme, state, &mut action);
    ui.add_space(theme.metrics.gap_lg);

    editor(ui, &theme, state, ui_state, &mut action);

    let ctx = ui.ctx().clone();
    json_dialog(&ctx, &theme, ui_state, &mut action);
    reset_dialog(&ctx, &theme, ui_state, &mut action);
    new_profile_dialog(&ctx, &theme, ui_state, &mut action);

    if let Some(action) = action {
        apply(state, ui_state, action);
    }
}

fn profile_selector(
    ui: &mut egui::Ui,
    theme: &Theme,
    state: &AppState,
    _ui_state: &mut UiState,
    action: &mut Option<Action>,
) {
    let profiles = state.flag_profiles();
    let current = state.settings.advanced.active_flag_profile.clone();

    widgets::section(
        ui,
        "Active Profile",
        Some("Switch between different FastFlag configurations or create separate setups for specific games."),
        |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = theme.metrics.gap_sm;

                for name in &profiles {
                    let is_active = name == &current;
                    let display_name = if name == "default" {
                        "Default".to_string()
                    } else {
                        flags::sanitize_profile_name(name)
                    };

                    let btn = if is_active {
                        widgets::Button::primary(&display_name)
                    } else {
                        widgets::Button::new(&display_name).tone(widgets::Tone::Neutral)
                    };

                    if btn.show(ui).clicked() && !is_active {
                        *action = Some(Action::SwitchProfile(name.clone()));
                    }
                }

                if widgets::Button::new("New profile")
                    .icon(Icon::Plus)
                    .tone(widgets::Tone::Ghost)
                    .show(ui)
                    .clicked()
                {
                    *action = Some(Action::OpenNewProfile);
                }

                if current != "default"
                    && widgets::Button::new("Delete profile")
                        .icon(Icon::Trash)
                        .tone(widgets::Tone::Ghost)
                        .show(ui)
                        .clicked()
                {
                    *action = Some(Action::DeleteProfile(current));
                }
            });
        },
    );
}

fn presets_section(
    ui: &mut egui::Ui,
    theme: &Theme,
    _state: &AppState,
    action: &mut Option<Action>,
) {
    widgets::section(
        ui,
        "Curated Presets",
        Some("One-click tested flag bundles. Applying a preset adds or updates its flags in your active profile."),
        |ui| {
            for (index, preset) in flags::PRESETS.iter().enumerate() {
                widgets::nested(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.set_width((ui.available_width() - 120.0).max(100.0));
                            ui.label(
                                egui::RichText::new(preset.name)
                                    .font(theme::strong(theme::size::BODY))
                                    .color(theme.palette.text),
                            );
                            ui.add_space(2.0);
                            ui.label(
                                egui::RichText::new(preset.description)
                                    .font(theme::text_style(theme::size::MICRO))
                                    .color(theme.palette.text_muted),
                            );
                        });

                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if widgets::Button::new("Apply")
                                .icon(Icon::Check)
                                .tone(widgets::Tone::Neutral)
                                .size(widgets::Size::Small)
                                .show(ui)
                                .clicked()
                            {
                                *action = Some(Action::ApplyPreset(index));
                            }
                        });
                    });
                });
                ui.add_space(theme.metrics.gap_xs);
            }
        },
    );
}

fn applying(ui: &mut egui::Ui, theme: &Theme, state: &AppState, action: &mut Option<Action>) {
    let mut applied = state.settings.advanced.apply_flag_profile;
    let file = state
        .client_flag_file()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "no Roblox is installed yet".into());

    widgets::section(
        ui,
        "How they reach the client",
        Some("Roblox wipes the version folder when it updates, so the profile is kept here and written again."),
        |ui| {
            widgets::setting_row(
                ui,
                "Write them on every launch",
                "Off means the client keeps whatever it has until you write them yourself.",
                |ui| {
                    if widgets::toggle(ui, &mut applied).changed() {
                        *action = Some(Action::ToggleApply);
                    }
                },
            );

            ui.add_space(theme.metrics.gap_md);
            widgets::detail_row(ui, "Client file", &file, true);

            ui.add_space(theme.metrics.gap_md);
            ui.horizontal(|ui| {
                if widgets::Button::new("Write to the client now")
                    .icon(Icon::Check)
                    .size(widgets::Size::Small)
                    .enabled(state.detection.active().is_some())
                    .show(ui)
                    .clicked()
                {
                    *action = Some(Action::WriteNow);
                }
                ui.label(
                    egui::RichText::new(format!("{} active", state.flags.active_count()))
                        .font(theme::text_style(theme::size::MICRO))
                        .color(theme.palette.text_faint),
                );
            });
        },
    );
}

const DIALOG_WIDTH: f32 = 520.0;

fn reset_dialog(
    ctx: &egui::Context,
    theme: &Theme,
    ui_state: &UiState,
    action: &mut Option<Action>,
) {
    if !ui_state.confirm_flag_reset {
        return;
    }

    let palette = theme.palette;
    let mut close = false;

    let response = egui::Modal::new(egui::Id::new("flag-reset"))
        .backdrop_color(palette.scrim)
        .frame(
            egui::Frame::new()
                .fill(palette.surface)
                .stroke(egui::Stroke::new(1.0, palette.border))
                .corner_radius(theme.radius_lg())
                .inner_margin(egui::Margin::same(22)),
        )
        .show(ctx, |ui| {
            ui.set_width(360.0);
            ui.label(
                egui::RichText::new("Reset every flag?")
                    .font(theme::strong(theme::size::TITLE))
                    .color(palette.text),
            );
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new(
                    "All custom flags in your profile are removed, and the client goes back to its own defaults. A copy of the file it is using now is kept in the backup folder.",
                )
                .font(theme::text_style(theme::size::SMALL))
                .color(palette.text_muted),
            );
            ui.add_space(theme.metrics.gap_lg);

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if widgets::Button::primary("Reset flags")
                    .icon(Icon::Trash)
                    .show(ui)
                    .clicked()
                {
                    *action = Some(Action::Reset);
                }
                if widgets::Button::new("Keep them")
                    .tone(widgets::Tone::Ghost)
                    .show(ui)
                    .clicked()
                {
                    close = true;
                }
            });
        });

    if (close || response.should_close()) && action.is_none() {
        *action = Some(Action::CancelReset);
    }
}

fn json_dialog(
    ctx: &egui::Context,
    theme: &Theme,
    ui_state: &mut UiState,
    action: &mut Option<Action>,
) {
    if ui_state.raw_editor.is_none() {
        return;
    }

    let palette = theme.palette;
    let pasted = ctx.input(|input| {
        input.events.iter().find_map(|event| match event {
            egui::Event::Paste(text) => Some(text.clone()),
            _ => None,
        })
    });

    let mut close = false;

    let response = egui::Modal::new(egui::Id::new("flag-json"))
        .backdrop_color(palette.scrim)
        .frame(
            egui::Frame::new()
                .fill(palette.surface)
                .stroke(egui::Stroke::new(1.0, palette.border))
                .corner_radius(theme.radius_lg())
                .inner_margin(egui::Margin::same(22)),
        )
        .show(ctx, |ui| {
            ui.set_width(DIALOG_WIDTH);

            ui.label(
                egui::RichText::new("Import or export flags")
                    .font(theme::strong(theme::size::TITLE))
                    .color(palette.text),
            );
            ui.add_space(theme.metrics.gap_xs);
            ui.label(
                egui::RichText::new(
                    "This is the same JSON Roblox reads. Copy it to share your setup, or paste someone else's over it and replace the profile.",
                )
                .font(theme::text_style(theme::size::SMALL))
                .color(palette.text_muted),
            );

            ui.add_space(theme.metrics.gap_md);

            let buffer = ui_state.raw_editor.as_mut().expect("the dialog is open");
            let field = widgets::multiline_field(ui, buffer, 10);
            if let Some(text) = &pasted {
                if !field.has_focus() {
                    buffer.clear();
                    buffer.push_str(text);
                }
            }

            let parsed = flags::FlagProfile::parse(buffer);
            let (tone, note) = match &parsed {
                Ok(profile) => (
                    palette.success,
                    match profile.entries.len() {
                        0 => "Valid, no flags in it".to_string(),
                        1 => "Valid, 1 flag".to_string(),
                        count => format!("Valid, {count} flags"),
                    },
                ),
                Err(err) => (palette.danger, err.to_string()),
            };

            ui.add_space(theme.metrics.gap_sm);
            ui.label(
                egui::RichText::new(note)
                    .font(theme::text_style(theme::size::SMALL))
                    .color(tone),
            );

            ui.add_space(theme.metrics.gap_lg);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = theme.metrics.gap_sm;

                if widgets::Button::new("Copy")
                    .icon(Icon::Copy)
                    .tone(widgets::Tone::Neutral)
                    .show(ui)
                    .clicked()
                {
                    ui.ctx().copy_text(buffer.clone());
                    *action = Some(Action::Copied);
                }

                if widgets::Button::new("Paste")
                    .icon(Icon::Plus)
                    .tone(widgets::Tone::Neutral)
                    .show(ui)
                    .clicked()
                {
                    ui.ctx()
                        .send_viewport_cmd(egui::ViewportCommand::RequestPaste);
                }

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::Button::primary("Replace profile")
                        .enabled(parsed.is_ok())
                        .show(ui)
                        .clicked()
                    {
                        *action = Some(Action::CommitRaw);
                    }
                    if widgets::Button::new("Cancel")
                        .tone(widgets::Tone::Ghost)
                        .show(ui)
                        .clicked()
                    {
                        close = true;
                    }
                });
            });
        });

    if (close || response.should_close()) && action.is_none() {
        *action = Some(Action::CloseRaw);
    }
}

fn editor(
    ui: &mut egui::Ui,
    theme: &Theme,
    state: &AppState,
    ui_state: &mut UiState,
    action: &mut Option<Action>,
) {
    let refused = state.denied_active_flags();
    let filter = ui_state.flag_filter.trim().to_lowercase();
    let entries = state.flags.entries.clone();
    let visible: Vec<_> = entries
        .iter()
        .filter(|entry| {
            filter.is_empty()
                || entry.key.to_lowercase().contains(&filter)
                || entry.value.display().to_lowercase().contains(&filter)
        })
        .collect();

    widgets::section(
        ui,
        "Profile Flags",
        Some("Every flag RustBlox will write to ClientAppSettings.json."),
        |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = theme.metrics.gap_sm;
                let width = ((ui.available_width() - 210.0) * 0.55).clamp(80.0, 300.0);
                widgets::text_field(ui, &mut ui_state.flag_key, "FFlagExampleName", width);
                widgets::text_field(ui, &mut ui_state.flag_value, "Value", width * 0.6);
                if widgets::Button::new("Add")
                    .icon(Icon::Plus)
                    .show(ui)
                    .clicked()
                {
                    *action = Some(Action::Add);
                }
            });

            if let Some(error) = &ui_state.flag_error {
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(error)
                        .font(theme::text_style(theme::size::SMALL))
                        .color(theme.palette.danger),
                );
            }

            ui.add_space(theme.metrics.gap_md);

            if entries.is_empty() {
                widgets::nested(ui, |ui| {
                    ui.label(
                        egui::RichText::new(
                            "The profile is empty. Roblox will use its own defaults.",
                        )
                        .font(theme::text_style(theme::size::SMALL))
                        .color(theme.palette.text_muted),
                    );
                });
                return;
            }

            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = theme.metrics.gap_sm;
                widgets::text_field(
                    ui,
                    &mut ui_state.flag_filter,
                    "Search flags by name or value...",
                    280.0,
                );
                if !ui_state.flag_filter.is_empty()
                    && widgets::icon_button(ui, Icon::Close, "Clear search", true).clicked()
                {
                    ui_state.flag_filter.clear();
                }
            });
            ui.add_space(theme.metrics.gap_sm);

            if visible.is_empty() {
                ui.label(
                    egui::RichText::new("No flags match your search query.")
                        .font(theme::text_style(theme::size::SMALL))
                        .color(theme.palette.text_muted),
                );
                return;
            }

            columns(ui, theme);

            for entry in visible {
                widgets::nested(ui, |ui| {
                    ui.horizontal(|ui| {
                        let mut enabled = entry.enabled;
                        if widgets::toggle(ui, &mut enabled).changed() {
                            *action = Some(Action::Toggle(entry.key.clone()));
                        }

                        ui.vertical(|ui| {
                            ui.set_width((ui.available_width() - 220.0).max(70.0));
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(&entry.key)
                                        .font(egui::FontId::new(
                                            theme::size::SMALL,
                                            egui::FontFamily::Monospace,
                                        ))
                                        .color(if entry.enabled {
                                            theme.palette.text
                                        } else {
                                            theme.palette.text_faint
                                        }),
                                );
                                if refused.iter().any(|key| key == &entry.key) {
                                    widgets::badge(ui, "refused by Roblox", feedback::Tone::Danger);
                                } else if flags::looks_unusual(&entry.key) {
                                    widgets::badge(ui, "unknown prefix", feedback::Tone::Warning);
                                }
                            });
                        });

                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if widgets::icon_button(ui, Icon::Trash, "Remove", true).clicked() {
                                *action = Some(Action::Remove(entry.key.clone()));
                            }

                            let mut buffer = entry.value.display();
                            if widgets::text_field(ui, &mut buffer, "value", 130.0).changed() {
                                *action = Some(Action::Edit {
                                    key: entry.key.clone(),
                                    value: buffer,
                                });
                            }
                        });
                    });
                });
                ui.add_space(6.0);
            }
        },
    );
}

fn columns(ui: &mut egui::Ui, theme: &Theme) {
    let label = |ui: &mut egui::Ui, text: &str| {
        ui.label(
            egui::RichText::new(text)
                .font(theme::medium(theme::size::MICRO))
                .color(theme.palette.text_faint),
        );
    };

    ui.horizontal(|ui| {
        ui.add_space(4.0);
        label(ui, "ON");
        ui.add_space(theme.metrics.gap_lg);
        label(ui, "NAME");
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(38.0);
            label(ui, "VALUE");
        });
    });
    ui.add_space(theme.metrics.gap_xs);
}

fn new_profile_dialog(
    ctx: &egui::Context,
    theme: &Theme,
    ui_state: &mut UiState,
    action: &mut Option<Action>,
) {
    if !ui_state.show_new_profile_dialog {
        return;
    }

    let palette = theme.palette;
    let mut close = false;

    let response = egui::Modal::new(egui::Id::new("new-flag-profile"))
        .backdrop_color(palette.scrim)
        .frame(
            egui::Frame::new()
                .fill(palette.surface)
                .stroke(egui::Stroke::new(1.0, palette.border))
                .corner_radius(theme.radius_lg())
                .inner_margin(egui::Margin::same(22)),
        )
        .show(ctx, |ui| {
            ui.set_width(380.0);

            ui.label(
                egui::RichText::new("Create FastFlag Profile")
                    .font(theme::strong(theme::size::TITLE))
                    .color(palette.text),
            );
            ui.add_space(theme.metrics.gap_xs);
            ui.label(
                egui::RichText::new("Enter a name for your new configuration profile.")
                    .font(theme::text_style(theme::size::SMALL))
                    .color(palette.text_muted),
            );

            ui.add_space(theme.metrics.gap_md);
            widgets::text_field(
                ui,
                &mut ui_state.new_profile_name,
                "Profile name (e.g. competitive)",
                320.0,
            );

            ui.add_space(theme.metrics.gap_sm);
            ui.horizontal(|ui| {
                widgets::toggle(ui, &mut ui_state.clone_profile_on_create);
                ui.label(
                    egui::RichText::new("Copy flags from current profile")
                        .font(theme::text_style(theme::size::SMALL))
                        .color(palette.text),
                );
            });

            ui.add_space(theme.metrics.gap_lg);
            ui.horizontal(|ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let valid = !ui_state.new_profile_name.trim().is_empty();
                    if widgets::Button::primary("Create")
                        .enabled(valid)
                        .show(ui)
                        .clicked()
                    {
                        let name = ui_state.new_profile_name.trim().to_string();
                        let clone = ui_state.clone_profile_on_create;
                        *action = Some(Action::CreateProfile { name, clone });
                    }
                    if widgets::Button::new("Cancel")
                        .tone(widgets::Tone::Ghost)
                        .show(ui)
                        .clicked()
                    {
                        close = true;
                    }
                });
            });
        });

    if (close || response.should_close()) && action.is_none() {
        *action = Some(Action::CloseNewProfile);
    }
}

fn apply(state: &mut AppState, ui_state: &mut UiState, action: Action) {
    match action {
        Action::Add => {
            let key = ui_state.flag_key.trim().to_owned();
            match flags::validate_key(&key) {
                Ok(()) if state.flags.contains(&key) => {
                    ui_state.flag_error = Some("That flag is already in the profile.".into());
                }
                Ok(()) => {
                    let value = FlagValue::from_input(&ui_state.flag_value);
                    state.flags.set(key, value);
                    state.flags.sort();
                    ui_state.flag_key.clear();
                    ui_state.flag_value.clear();
                    ui_state.flag_error = None;
                    state.commit_flags();
                }
                Err(err) => ui_state.flag_error = Some(err.to_string()),
            }
        }
        Action::Remove(key) => {
            state.flags.remove(&key);
            state.commit_flags();
        }
        Action::ToggleApply => {
            state.settings.advanced.apply_flag_profile =
                !state.settings.advanced.apply_flag_profile;
            state.mark_settings_dirty();
            state.flush_settings();
        }
        Action::WriteNow => state.write_flags_now(),
        Action::Copied => state.toasts.success("Copied to the clipboard"),
        Action::Toggle(key) => {
            if let Some(entry) = state
                .flags
                .entries
                .iter_mut()
                .find(|entry| entry.key == key)
            {
                entry.enabled = !entry.enabled;
            }
            state.commit_flags();
        }
        Action::Edit { key, value } => {
            if let Some(entry) = state
                .flags
                .entries
                .iter_mut()
                .find(|entry| entry.key == key)
            {
                entry.value = FlagValue::from_input(&value);
            }
            state.mark_flags_dirty();
        }
        Action::AskReset => ui_state.confirm_flag_reset = true,
        Action::Reset => {
            ui_state.confirm_flag_reset = false;
            ui_state.flag_filter.clear();
            ui_state.flag_error = None;
            state.reset_flags();
        }
        Action::CancelReset => ui_state.confirm_flag_reset = false,
        Action::OpenRaw => ui_state.raw_editor = Some(state.flags.to_pretty()),
        Action::CloseRaw => ui_state.raw_editor = None,
        Action::CommitRaw => {
            let text = ui_state.raw_editor.clone().unwrap_or_default();
            if let Ok(profile) = flags::FlagProfile::parse(&text) {
                state.flags = profile;
                ui_state.raw_editor = None;
                state.commit_flags();
            }
        }
        Action::SwitchProfile(name) => state.switch_flag_profile(&name),
        Action::OpenNewProfile => {
            ui_state.new_profile_name.clear();
            ui_state.clone_profile_on_create = true;
            ui_state.show_new_profile_dialog = true;
        }
        Action::CloseNewProfile => {
            ui_state.show_new_profile_dialog = false;
        }
        Action::CreateProfile { name, clone } => {
            ui_state.show_new_profile_dialog = false;
            state.create_flag_profile(&name, clone);
        }
        Action::DeleteProfile(name) => state.delete_flag_profile(&name),
        Action::ApplyPreset(index) => state.apply_preset_flags(index),
    }
}

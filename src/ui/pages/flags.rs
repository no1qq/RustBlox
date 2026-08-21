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
    TogglePreset(&'static str),
    Save,
    Apply,
    ClearApplied,
    OpenRaw,
    CloseRaw,
    CommitRaw,
    Copied,
    OpenAppliedFile,
}

pub fn render(ui: &mut egui::Ui, state: &mut AppState, ui_state: &mut UiState) {
    let theme = Theme::get(ui.ctx());
    let mut action = None;

    widgets::page_header(
        ui,
        "Flags",
        "Written to ClientAppSettings.json before Roblox starts.",
        |ui| {
            if widgets::Button::primary("Save profile")
                .icon(Icon::Check)
                .show(ui)
                .clicked()
            {
                action = Some(Action::Save);
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

    widgets::banner(
        ui,
        feedback::Tone::Warning,
        "These are unofficial client settings",
        "Roblox does not document or support editing this file. Unknown values are ignored, bad values can stop the client from starting, and Roblox may change or remove any of them without notice. RustBlox keeps a timestamped backup every time it writes.");
    ui.add_space(theme.metrics.gap_lg);

    status_card(ui, &theme, state, &mut action);
    ui.add_space(theme.metrics.gap_lg);

    presets(ui, &theme, state, &mut action);
    ui.add_space(theme.metrics.gap_lg);

    editor(ui, &theme, state, ui_state, &mut action);

    let ctx = ui.ctx().clone();
    json_dialog(&ctx, &theme, ui_state, &mut action);

    if let Some(action) = action {
        apply(state, ui_state, action);
    }
}

fn presets(ui: &mut egui::Ui, theme: &Theme, state: &AppState, action: &mut Option<Action>) {
    widgets::section(
        ui,
        "Presets",
        Some("Each one adds or removes the handful of flags it needs."),
        |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing =
                    egui::vec2(theme.metrics.gap_sm, theme.metrics.gap_sm);

                for preset in &flags::PRESETS {
                    let on = state.flags.preset_applied(preset);
                    let response = widgets::Button::new(preset.name)
                        .icon(if on { Icon::Check } else { Icon::Plus })
                        .tone(if on {
                            widgets::Tone::Primary
                        } else {
                            widgets::Tone::Neutral
                        })
                        .size(widgets::Size::Small)
                        .show(ui)
                        .on_hover_text(preset.detail);

                    if response.clicked() {
                        *action = Some(Action::TogglePreset(preset.name));
                    }
                }
            });
        },
    );
}

fn status_card(ui: &mut egui::Ui, theme: &Theme, state: &AppState, action: &mut Option<Action>) {
    let profile_count = state.flags.active_count();
    let applied = state.applied_flags.clone();
    let applied_count = applied.as_ref().map(|profile| profile.entries.len());
    let install = state.detection.active().cloned();
    let in_sync = applied
        .as_ref()
        .map(|disk| disk.to_json() == state.flags.to_json())
        .unwrap_or(profile_count == 0);

    widgets::card(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = theme.metrics.gap_xl;
            widgets::stat(
                ui,
                "In this profile",
                &profile_count.to_string(),
                feedback::Tone::Accent,
            );
            widgets::stat(
                ui,
                "Written to the client",
                &applied_count
                    .map(|count| count.to_string())
                    .unwrap_or_else(|| "none".into()),
                if in_sync {
                    feedback::Tone::Success
                } else {
                    feedback::Tone::Warning
                },
            );
            widgets::stat(
                ui,
                "Applied on launch",
                if state.settings.advanced.apply_flag_profile {
                    "Yes"
                } else {
                    "No"
                },
                feedback::Tone::Neutral,
            );

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if widgets::Button::new("Write now")
                    .icon(Icon::Flag)
                    .size(widgets::Size::Small)
                    .enabled(install.is_some())
                    .show(ui)
                    .clicked()
                {
                    *action = Some(Action::Apply);
                }
                if applied_count.is_some()
                    && widgets::Button::new("Remove from client")
                        .tone(widgets::Tone::Ghost)
                        .size(widgets::Size::Small)
                        .show(ui)
                        .clicked()
                {
                    *action = Some(Action::ClearApplied);
                }
            });
        });

        if let Some(install) = &install {
            ui.add_space(theme.metrics.gap_md);
            ui.horizontal(|ui| {
                let path = install.client_settings_file().display().to_string();
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(&path)
                            .font(egui::FontId::new(
                                theme::size::MICRO,
                                egui::FontFamily::Monospace,
                            ))
                            .color(theme.palette.text_faint),
                    )
                    .truncate(),
                )
                .on_hover_text(&path);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if applied_count.is_some()
                        && widgets::Button::new("Open folder")
                            .tone(widgets::Tone::Quiet)
                            .size(widgets::Size::Small)
                            .show(ui)
                            .clicked()
                    {
                        *action = Some(Action::OpenAppliedFile);
                    }
                });
            });
        } else {
            ui.add_space(theme.metrics.gap_sm);
            ui.label(
                egui::RichText::new(
                    "No Roblox installation is selected, so the profile cannot be written yet.",
                )
                .font(theme::text_style(theme::size::SMALL))
                .color(theme.palette.warning),
            );
        }
    });
}

const DIALOG_WIDTH: f32 = 520.0;

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
    let filter = ui_state.flag_filter.to_lowercase();
    let entries = state.flags.entries.clone();
    let visible: Vec<_> = entries
        .iter()
        .filter(|entry| filter.is_empty() || entry.key.to_lowercase().contains(&filter))
        .collect();

    widgets::section(ui, "Profile", None, |ui| {
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
                    egui::RichText::new("The profile is empty. Roblox will use its own defaults.")
                        .font(theme::text_style(theme::size::SMALL))
                        .color(theme.palette.text_muted),
                );
            });
            return;
        }

        if entries.len() > 6 {
            widgets::text_field(ui, &mut ui_state.flag_filter, "Filter flags", 240.0);
            ui.add_space(theme.metrics.gap_sm);
        }

        if visible.is_empty() {
            ui.label(
                egui::RichText::new("No flags match that filter.")
                    .font(theme::text_style(theme::size::SMALL))
                    .color(theme.palette.text_muted),
            );
            return;
        }

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
                            if flags::looks_unusual(&entry.key) {
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
    });
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
                    state.save_flags();
                }
                Err(err) => ui_state.flag_error = Some(err.to_string()),
            }
        }
        Action::Remove(key) => {
            state.flags.remove(&key);
            state.save_flags();
        }
        Action::TogglePreset(name) => {
            if let Some(preset) = flags::preset_named(name) {
                if state.flags.preset_applied(preset) {
                    state.flags.remove_preset(preset);
                } else {
                    state.flags.apply_preset(preset);
                }
                state.save_flags();
            }
        }
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
            state.save_flags();
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
        }
        Action::Save => state.save_flags(),
        Action::Apply => state.apply_flags_now(),
        Action::ClearApplied => state.clear_applied_flags(),
        Action::OpenRaw => {
            ui_state.raw_editor = Some(state.flags.to_pretty());
            ui_state.raw_error = None;
        }
        Action::CloseRaw => {
            ui_state.raw_editor = None;
            ui_state.raw_error = None;
        }
        Action::CommitRaw => {
            let text = ui_state.raw_editor.clone().unwrap_or_default();
            match flags::FlagProfile::parse(&text) {
                Ok(profile) => {
                    state.flags = profile;
                    ui_state.raw_editor = None;
                    ui_state.raw_error = None;
                    state.save_flags();
                }
                Err(err) => ui_state.raw_error = Some(err.to_string()),
            }
        }
        Action::OpenAppliedFile => {
            if let Some(install) = state.detection.active().cloned() {
                state.open_path(install.client_settings_dir());
            }
        }
    }
}

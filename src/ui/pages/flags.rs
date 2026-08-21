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
    AskReset,
    Reset,
    CancelReset,
    OpenRaw,
    CloseRaw,
    CommitRaw,
    Copied,
}

pub fn render(ui: &mut egui::Ui, state: &mut AppState, ui_state: &mut UiState) {
    let theme = Theme::get(ui.ctx());
    let mut action = None;

    widgets::page_header(
        ui,
        "Flags",
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

    widgets::banner(
        ui,
        feedback::Tone::Warning,
        "These are unofficial client settings",
        "Roblox does not document or support editing this file. Unknown values are ignored, bad values can stop the client from starting, and Roblox may change or remove any of them without notice. RustBlox keeps a timestamped copy of any file it replaces.");
    ui.add_space(theme.metrics.gap_lg);

    presets(ui, &theme, state, &mut action);
    ui.add_space(theme.metrics.gap_lg);

    editor(ui, &theme, state, ui_state, &mut action);

    let ctx = ui.ctx().clone();
    json_dialog(&ctx, &theme, ui_state, &mut action);
    reset_dialog(&ctx, &theme, ui_state, &mut action);

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
                    "Presets and custom flags both go, and the client goes back to its own defaults. A copy of the file it is using now is kept in the backup folder.",
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
                    state.commit_flags();
                }
                Err(err) => ui_state.flag_error = Some(err.to_string()),
            }
        }
        Action::Remove(key) => {
            state.flags.remove(&key);
            state.commit_flags();
        }
        Action::TogglePreset(name) => {
            if let Some(preset) = flags::preset_named(name) {
                if state.flags.preset_applied(preset) {
                    state.flags.remove_preset(preset);
                } else {
                    state.flags.apply_preset(preset);
                }
                state.commit_flags();
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
    }
}

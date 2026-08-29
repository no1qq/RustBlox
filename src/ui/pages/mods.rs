use egui::{Align, Layout};

use crate::app::AppState;
use crate::roblox::install::format_size;

use crate::ui::icons::Icon;
use crate::ui::theme::{self, Theme};
use crate::ui::widgets::{self, feedback};

const LISTED: usize = 24;

enum Action {
    Toggle,
    ApplyNow,
    OpenFolder,
    ChooseFont,
    ClearFont,
    SetDeathSound(crate::config::DeathSoundPreset),
    SetCursor(crate::config::CursorPreset),
}

pub fn render(ui: &mut egui::Ui, state: &mut AppState) {
    let theme = Theme::get(ui.ctx());
    state.refresh_mods(false);

    let mut action = None;

    widgets::page_header(
        ui,
        "Mods",
        "Files laid over RustBlox's own copy of Roblox, put back when you take them out.",
        |ui| {
            if widgets::Button::new("Reread")
                .icon(Icon::Refresh)
                .tone(widgets::Tone::Ghost)
                .show(ui)
                .clicked()
            {
                action = Some(Action::ApplyNow);
            }
        },
    );

    widgets::banner(
        ui,
        feedback::Tone::Warning,
        "Roblox does not support any of this",
        "A mod is a file copied over one of the client's own. A bad one can stop Roblox starting, and Roblox may move or rename anything it likes in an update. RustBlox keeps the original of every file it replaces and puts it back the moment the mod leaves the folder.");
    ui.add_space(theme.metrics.gap_lg);

    control(ui, &theme, state, &mut action);
    ui.add_space(theme.metrics.gap_lg);
    sound_preset(ui, &theme, state, &mut action);
    ui.add_space(theme.metrics.gap_lg);
    cursor_preset(ui, &theme, state, &mut action);
    ui.add_space(theme.metrics.gap_lg);
    font(ui, &theme, state, &mut action);
    ui.add_space(theme.metrics.gap_lg);
    contents(ui, &theme, state);

    match action {
        Some(Action::Toggle) => {
            state.settings.mods.enabled = !state.settings.mods.enabled;
            state.mark_settings_dirty();
            state.flush_settings();
            state.apply_mods_now();
        }
        Some(Action::ApplyNow) => state.apply_mods_now(),
        Some(Action::OpenFolder) => {
            let folder = state.store.paths().mods_dir();
            if let Err(err) = crate::util::fs::ensure_dir(&folder) {
                state.toasts.error(
                    "The mods folder could not be created",
                    Some(err.to_string()),
                );
            } else {
                state.open_path(folder);
            }
        }
        Some(Action::ChooseFont) => state.choose_font(),
        Some(Action::ClearFont) => state.clear_font(),
        Some(Action::SetDeathSound(crate::config::DeathSoundPreset::Custom)) => {
            state.choose_death_sound();
        }
        Some(Action::SetDeathSound(preset)) => {
            state.settings.mods.death_sound = preset;
            state.mark_settings_dirty();
            state.flush_settings();
            let _ = crate::roblox::mods::apply_death_sound_preset(
                &state.store.paths().mods_dir(),
                preset,
            );
            state.refresh_mods(true);
            state.apply_mods_now();
        }
        Some(Action::SetCursor(preset)) => {
            state.settings.mods.cursor = preset;
            state.mark_settings_dirty();
            state.flush_settings();
            state.refresh_mods(true);
            state.apply_mods_now();
        }
        None => {}
    }
}

fn control(ui: &mut egui::Ui, theme: &Theme, state: &AppState, action: &mut Option<Action>) {
    let mut enabled = state.settings.mods.enabled;
    let folder = state.store.paths().mods_dir().display().to_string();
    let client = state
        .detection
        .active()
        .map(|install| install.version_dir.display().to_string())
        .unwrap_or_else(|| "no Roblox is installed yet".into());

    widgets::section(
        ui,
        "How mods are applied",
        Some("Laid over the client on every launch, because a Roblox update replaces the folder."),
        |ui| {
            widgets::setting_row(
                ui,
                "Lay them over the client",
                "Off puts every original back and leaves the folder alone.",
                |ui| {
                    if widgets::toggle(ui, &mut enabled).changed() {
                        *action = Some(Action::Toggle);
                    }
                },
            );

            ui.add_space(theme.metrics.gap_md);
            widgets::detail_row(ui, "Mods folder", &folder, true);
            ui.add_space(theme.metrics.gap_xs);
            widgets::detail_row(ui, "Client folder", &client, true);

            ui.add_space(theme.metrics.gap_md);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = theme.metrics.gap_sm;
                if widgets::Button::new("Open mods folder")
                    .icon(Icon::Folder)
                    .size(widgets::Size::Small)
                    .show(ui)
                    .clicked()
                {
                    *action = Some(Action::OpenFolder);
                }
                if widgets::Button::new("Apply now")
                    .icon(Icon::Check)
                    .size(widgets::Size::Small)
                    .enabled(state.detection.active().is_some())
                    .show(ui)
                    .clicked()
                {
                    *action = Some(Action::ApplyNow);
                }
            });
        },
    );
}

fn contents(ui: &mut egui::Ui, theme: &Theme, state: &AppState) {
    let inventory = &state.mods;

    widgets::section(
        ui,
        "What is in the folder",
        Some("The folder mirrors the client, so content\\fonts\\x.ttf lands on the same path."),
        |ui| {
            if inventory.is_empty() {
                widgets::nested(ui, |ui| {
                    ui.label(
                        egui::RichText::new(
                            "Nothing yet. Put a file at the same path it has inside the client, \
                             so content\\textures\\Cursors\\KeyboardMouse\\ArrowFarCursor.png in \
                             the mods folder replaces exactly that one.",
                        )
                        .font(theme::text_style(theme::size::SMALL))
                        .color(theme.palette.text_muted),
                    );
                });
                return;
            }

            ui.horizontal(|ui| {
                widgets::badge(
                    ui,
                    &format!("{} files", inventory.files.len()),
                    feedback::Tone::Neutral,
                );
                widgets::badge(ui, &format_size(inventory.bytes), feedback::Tone::Neutral);
                if !state.settings.mods.enabled {
                    widgets::badge(ui, "not being applied", feedback::Tone::Warning);
                }
            });
            ui.add_space(theme.metrics.gap_md);

            for entry in inventory.files.iter().take(LISTED) {
                widgets::nested(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(entry.display())
                                .font(egui::FontId::new(
                                    theme::size::SMALL,
                                    egui::FontFamily::Monospace,
                                ))
                                .color(theme.palette.text),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(
                                egui::RichText::new(format_size(entry.bytes))
                                    .font(theme::text_style(theme::size::MICRO))
                                    .color(theme.palette.text_faint),
                            );
                        });
                    });
                });
                ui.add_space(4.0);
            }

            if inventory.files.len() > LISTED {
                ui.add_space(theme.metrics.gap_xs);
                ui.label(
                    egui::RichText::new(format!("and {} more", inventory.files.len() - LISTED))
                        .font(theme::text_style(theme::size::SMALL))
                        .color(theme.palette.text_muted),
                );
            }
        },
    );
}

fn font(ui: &mut egui::Ui, theme: &Theme, state: &AppState, action: &mut Option<Action>) {
    let current = state.mods.font().map(|entry| entry.display());
    let installed = state.detection.active().is_some();

    widgets::section(
        ui,
        "Font",
        Some("Points every font family the client ships at one file of your own."),
        |ui| {
            widgets::nested(ui, |ui| {
                ui.label(
                    egui::RichText::new(match &current {
                        Some(path) => format!("Roblox is drawing everything with {path}."),
                        None => "Roblox is using its own fonts.".to_owned(),
                    })
                    .font(theme::text_style(theme::size::SMALL))
                    .color(theme.palette.text_muted),
                );
            });

            ui.add_space(theme.metrics.gap_md);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = theme.metrics.gap_sm;
                if widgets::Button::new(if current.is_some() {
                    "Choose another font"
                } else {
                    "Choose a font"
                })
                .icon(Icon::Plus)
                .size(widgets::Size::Small)
                .enabled(installed)
                .show(ui)
                .clicked()
                {
                    *action = Some(Action::ChooseFont);
                }
                if widgets::Button::new("Back to the Roblox fonts")
                    .icon(Icon::Trash)
                    .tone(widgets::Tone::Ghost)
                    .size(widgets::Size::Small)
                    .enabled(current.is_some())
                    .show(ui)
                    .clicked()
                {
                    *action = Some(Action::ClearFont);
                }
            });

            if !installed {
                ui.add_space(theme.metrics.gap_sm);
                ui.label(
                    egui::RichText::new(
                        "Install Roblox first. The families are read from the copy on disk.",
                    )
                    .font(theme::text_style(theme::size::SMALL))
                    .color(theme.palette.text_faint),
                );
            }
        },
    );
}

fn sound_preset(ui: &mut egui::Ui, theme: &Theme, state: &AppState, action: &mut Option<Action>) {
    let current = state.settings.mods.death_sound;

    widgets::section(
        ui,
        "Death Sound",
        Some("Changes the player character reset/death sound effect."),
        |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = theme.metrics.gap_sm;
                for preset in crate::config::DeathSoundPreset::ALL {
                    let active = current == preset;
                    if widgets::Button::new(preset.label())
                        .tone(if active {
                            widgets::Tone::Primary
                        } else {
                            widgets::Tone::Neutral
                        })
                        .size(widgets::Size::Small)
                        .show(ui)
                        .clicked()
                    {
                        *action = Some(Action::SetDeathSound(preset));
                    }
                }
            });
        },
    );
}

fn cursor_preset(ui: &mut egui::Ui, theme: &Theme, state: &AppState, action: &mut Option<Action>) {
    let current = state.settings.mods.cursor;

    widgets::section(
        ui,
        "Mouse Cursor",
        Some("Replaces the in-game Roblox pointer and shift lock reticle."),
        |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = theme.metrics.gap_sm;
                for preset in crate::config::CursorPreset::ALL {
                    let active = current == preset;
                    if widgets::Button::new(preset.label())
                        .tone(if active {
                            widgets::Tone::Primary
                        } else {
                            widgets::Tone::Neutral
                        })
                        .size(widgets::Size::Small)
                        .show(ui)
                        .clicked()
                    {
                        *action = Some(Action::SetCursor(preset));
                    }
                }
            });
        },
    );
}

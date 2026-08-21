use egui::{Align, Layout};

use crate::app::AppState;
use crate::config::GameSettings;
use crate::roblox::gamesettings;

use crate::ui::icons::Icon;
use crate::ui::theme::{self, Theme};
use crate::ui::widgets::{self, feedback};

pub fn render(ui: &mut egui::Ui, state: &mut AppState) {
    let theme = Theme::get(ui.ctx());
    state.refresh_game_snapshot(false);

    let mut refresh = false;
    let mut open_folder = None;

    widgets::page_header(
        ui,
        "Game",
        "The settings Roblox keeps for itself, written before the client starts.",
        |ui| {
            refresh = widgets::Button::new("Reread")
                .icon(Icon::Refresh)
                .tone(widgets::Tone::Ghost)
                .show(ui)
                .clicked();
        },
    );

    if !state.game.found {
        widgets::banner(
            ui,
            feedback::Tone::Warning,
            "Roblox has not written its settings yet",
            "Start Roblox once and come back. Until that file exists there is nothing to change.",
        );
        ui.add_space(theme.metrics.gap_md);
    } else if state.game.locked {
        widgets::banner(
            ui,
            feedback::Tone::Accent,
            "The file is locked",
            "Roblox cannot write to it, so anything you change in game is forgotten when it closes.",
        );
        ui.add_space(theme.metrics.gap_md);
    }

    control(ui, &theme, state);
    ui.add_space(theme.metrics.gap_lg);

    if state.settings.game.manage {
        performance(ui, &theme, state);
        ui.add_space(theme.metrics.gap_lg);
        interface(ui, &theme, state);
        ui.add_space(theme.metrics.gap_lg);
        input(ui, &theme, state);
        ui.add_space(theme.metrics.gap_lg);
    }

    widgets::section(
        ui,
        "What Roblox has now",
        Some("Read straight back from the file on disk."),
        |ui| {
            let rows = [
                ("Frame rate limit", gamesettings::FRAMERATE_CAP),
                ("Graphics quality", gamesettings::QUALITY),
                ("Performance stats", gamesettings::PERFORMANCE_STATS),
                ("Interface transparency", gamesettings::TRANSPARENCY),
                ("Reduced motion", gamesettings::REDUCED_MOTION),
                ("Text size", gamesettings::TEXT_SIZE),
                ("Mouse sensitivity", gamesettings::MOUSE_SENSITIVITY),
                ("VR", gamesettings::VR_ENABLED),
            ];

            for (label, name) in rows {
                widgets::detail_row(ui, label, state.game.text(name).unwrap_or("not set"), true);
                ui.add_space(theme.metrics.gap_xs);
            }

            ui.add_space(theme.metrics.gap_sm);
            widgets::detail_row(
                ui,
                "File",
                &state
                    .game
                    .path
                    .as_ref()
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|| "not found on this system".into()),
                true,
            );

            ui.add_space(theme.metrics.gap_md);
            ui.horizontal(|ui| {
                if widgets::Button::new("Open the Roblox folder")
                    .icon(Icon::Folder)
                    .size(widgets::Size::Small)
                    .enabled(state.game.path.is_some())
                    .show(ui)
                    .clicked()
                {
                    open_folder = state
                        .game
                        .path
                        .as_ref()
                        .and_then(|path| path.parent())
                        .map(|parent| parent.to_path_buf());
                }
            });
        },
    );

    if refresh {
        state.refresh_game_snapshot(true);
    }
    if let Some(path) = open_folder {
        state.open_path(path);
    }
}

fn control(ui: &mut egui::Ui, theme: &Theme, state: &mut AppState) {
    let mut manage = state.settings.game.manage;
    let mut lock = state.settings.game.lock;
    let mut changed = false;

    widgets::section(
        ui,
        "How RustBlox writes them",
        Some("These live in Roblox's own file, shared by every Roblox on this PC."),
        |ui| {
            widgets::setting_row(
                ui,
                "Manage these settings",
                "RustBlox writes the values below into the file before each launch. \
                 The original is backed up the first time it changes anything.",
                |ui| {
                    changed |= widgets::toggle(ui, &mut manage).changed();
                },
            );
            ui.add_space(theme.metrics.gap_md);
            widgets::setting_row(
                ui,
                "Keep them locked",
                "Marks the file read only afterwards so Roblox cannot put its own values back \
                 when it closes. Settings you change in game stop sticking while this is on.",
                |ui| {
                    let response = widgets::toggle_enabled(ui, &mut lock, manage);
                    changed |= response.changed();
                },
            );

            if !manage {
                ui.add_space(theme.metrics.gap_md);
                widgets::nested(ui, |ui| {
                    ui.label(
                        egui::RichText::new(
                            "Nothing here is being written. Roblox keeps whatever it had.",
                        )
                        .font(theme::text_style(theme::size::SMALL))
                        .color(theme.palette.text_muted),
                    );
                });
            }
        },
    );

    if changed {
        state.settings.game.manage = manage;
        state.settings.game.lock = lock && manage;
        state.commit_game_settings();
    }
}

fn performance(ui: &mut egui::Ui, theme: &Theme, state: &mut AppState) {
    let cap_now = state
        .game
        .int(gamesettings::FRAMERATE_CAP)
        .unwrap_or(60)
        .clamp(
            GameSettings::MIN_FRAMERATE as i64,
            GameSettings::MAX_FRAMERATE as i64,
        ) as u32;
    let quality_now = state
        .game
        .int(gamesettings::QUALITY)
        .unwrap_or(0)
        .clamp(0, GameSettings::MAX_QUALITY as i64) as u8;
    let stats_now = state
        .game
        .flag(gamesettings::PERFORMANCE_STATS)
        .unwrap_or(false);

    let mut game = state.settings.game;
    let mut changed = false;
    let mut settled = false;

    widgets::section(
        ui,
        "Performance",
        Some("The frame rate and the quality Roblox renders at."),
        |ui| {
            managed(
                ui,
                theme,
                "Frame rate limit",
                "The cap Roblox schedules frames against. Above your monitor's refresh rate \
                 this only heats the card up.",
                &mut game.framerate_cap,
                cap_now,
                &mut changed,
                &mut settled,
                |ui, value| {
                    let mut shown = *value as f32;
                    let response = widgets::slider(
                        ui,
                        &mut shown,
                        GameSettings::MIN_FRAMERATE as f32..=360.0,
                        1.0,
                    );
                    if response.changed() {
                        *value = shown.round() as u32;
                    }
                    (
                        response.changed(),
                        response.drag_stopped() || response.clicked(),
                    )
                },
                |value| format!("{value} fps"),
            );

            ui.add_space(theme.metrics.gap_md);
            managed(
                ui,
                theme,
                "Graphics quality",
                "The same 1 to 10 slider Roblox shows in its own settings. \
                 Shadows and the heavier lighting drop out at the low end.",
                &mut game.quality,
                quality_now,
                &mut changed,
                &mut settled,
                |ui, value| {
                    let mut shown = *value as f32;
                    let response = widgets::slider(
                        ui,
                        &mut shown,
                        0.0..=GameSettings::MAX_QUALITY as f32,
                        1.0,
                    );
                    if response.changed() {
                        *value = shown.round() as u8;
                    }
                    (
                        response.changed(),
                        response.drag_stopped() || response.clicked(),
                    )
                },
                |value| gamesettings::quality_label(*value),
            );

            ui.add_space(theme.metrics.gap_md);
            managed(
                ui,
                theme,
                "Performance stats",
                "Turns on the stats panel the client draws over the game, frame rate included.",
                &mut game.performance_stats,
                stats_now,
                &mut changed,
                &mut settled,
                |ui, value| {
                    let response = widgets::toggle(ui, value);
                    (response.changed(), response.changed())
                },
                |value| if *value { "On".into() } else { "Off".into() },
            );
        },
    );

    apply(state, game, changed, settled);
}

fn interface(ui: &mut egui::Ui, theme: &Theme, state: &mut AppState) {
    let transparency_now = state
        .game
        .float(gamesettings::TRANSPARENCY)
        .unwrap_or(1.0)
        .clamp(0.0, 1.0);
    let motion_now = state
        .game
        .flag(gamesettings::REDUCED_MOTION)
        .unwrap_or(false);
    let text_now = state
        .game
        .int(gamesettings::TEXT_SIZE)
        .unwrap_or(GameSettings::MIN_TEXT_SIZE as i64)
        .clamp(
            GameSettings::MIN_TEXT_SIZE as i64,
            GameSettings::MAX_TEXT_SIZE as i64,
        ) as u8;

    let mut game = state.settings.game;
    let mut changed = false;
    let mut settled = false;

    widgets::section(
        ui,
        "Interface",
        Some("How the Roblox interface itself behaves in game."),
        |ui| {
            managed(
                ui,
                theme,
                "Interface transparency",
                "How solid the menus and the chat are. One is fully solid.",
                &mut game.transparency,
                transparency_now,
                &mut changed,
                &mut settled,
                |ui, value| {
                    let response = widgets::slider(ui, value, 0.0..=1.0, 0.05);
                    (
                        response.changed(),
                        response.drag_stopped() || response.clicked(),
                    )
                },
                |value| format!("{value:.2}"),
            );

            ui.add_space(theme.metrics.gap_md);
            managed(
                ui,
                theme,
                "Reduced motion",
                "Takes the animation off the escape menu and the rest of the client interface.",
                &mut game.reduced_motion,
                motion_now,
                &mut changed,
                &mut settled,
                |ui, value| {
                    let response = widgets::toggle(ui, value);
                    (response.changed(), response.changed())
                },
                |value| if *value { "On".into() } else { "Off".into() },
            );

            ui.add_space(theme.metrics.gap_md);
            managed(
                ui,
                theme,
                "Text size",
                "The accessibility text size Roblox offers. Anything but Default is untested here.",
                &mut game.text_size,
                text_now,
                &mut changed,
                &mut settled,
                |ui, value| {
                    let mut shown = *value as f32;
                    let response = widgets::slider(
                        ui,
                        &mut shown,
                        GameSettings::MIN_TEXT_SIZE as f32..=GameSettings::MAX_TEXT_SIZE as f32,
                        1.0,
                    );
                    if response.changed() {
                        *value = shown.round() as u8;
                    }
                    (
                        response.changed(),
                        response.drag_stopped() || response.clicked(),
                    )
                },
                |value| gamesettings::text_size_label(*value).to_owned(),
            );
        },
    );

    apply(state, game, changed, settled);
}

fn input(ui: &mut egui::Ui, theme: &Theme, state: &mut AppState) {
    let sensitivity_now = state
        .game
        .float(gamesettings::MOUSE_SENSITIVITY)
        .unwrap_or(1.0)
        .clamp(GameSettings::MIN_SENSITIVITY, GameSettings::MAX_SENSITIVITY);
    let vr_now = state.game.flag(gamesettings::VR_ENABLED).unwrap_or(false);

    let mut game = state.settings.game;
    let mut changed = false;
    let mut settled = false;

    widgets::section(
        ui,
        "Input and hardware",
        Some("Written for the third person, first person and default camera alike."),
        |ui| {
            managed(
                ui,
                theme,
                "Mouse sensitivity",
                "How far the camera swings for the same mouse movement.",
                &mut game.mouse_sensitivity,
                sensitivity_now,
                &mut changed,
                &mut settled,
                |ui, value| {
                    let response = widgets::slider(
                        ui,
                        value,
                        GameSettings::MIN_SENSITIVITY..=2.0,
                        GameSettings::MIN_SENSITIVITY,
                    );
                    (
                        response.changed(),
                        response.drag_stopped() || response.clicked(),
                    )
                },
                |value| format!("{value:.2}"),
            );

            ui.add_space(theme.metrics.gap_md);
            managed(
                ui,
                theme,
                "VR",
                "Whether Roblox looks for a headset when it starts.",
                &mut game.vr,
                vr_now,
                &mut changed,
                &mut settled,
                |ui, value| {
                    let response = widgets::toggle(ui, value);
                    (response.changed(), response.changed())
                },
                |value| if *value { "On".into() } else { "Off".into() },
            );
        },
    );

    apply(state, game, changed, settled);
}

fn apply(state: &mut AppState, game: GameSettings, changed: bool, settled: bool) {
    if !changed {
        return;
    }
    state.settings.game = game;
    if settled {
        state.commit_game_settings();
    } else {
        state.mark_game_dirty();
    }
}

#[allow(clippy::too_many_arguments)]
fn managed<T: Copy>(
    ui: &mut egui::Ui,
    theme: &Theme,
    title: &str,
    description: &str,
    slot: &mut Option<T>,
    seed: T,
    changed: &mut bool,
    settled: &mut bool,
    control: impl FnOnce(&mut egui::Ui, &mut T) -> (bool, bool),
    readout: impl Fn(&T) -> String,
) {
    let mut on = slot.is_some();

    widgets::setting_row(ui, title, description, |ui| {
        if widgets::toggle(ui, &mut on).changed() {
            *slot = if on { Some(seed) } else { None };
            *changed = true;
            *settled = true;
        }
    });

    let Some(value) = slot.as_mut() else {
        return;
    };

    ui.add_space(theme.metrics.gap_sm);
    widgets::nested(ui, |ui| {
        ui.horizontal(|ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let (moved, done) = control(ui, value);
                *changed |= moved;
                *settled |= done;

                ui.add_space(theme.metrics.gap_sm);
                ui.label(
                    egui::RichText::new(readout(value))
                        .font(theme::medium(theme::size::SMALL))
                        .color(theme.palette.text),
                );

                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new("RustBlox writes this one")
                            .font(theme::text_style(theme::size::MICRO))
                            .color(theme.palette.text_faint),
                    );
                });
            });
        });
    });
}

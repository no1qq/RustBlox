use egui::{Align, Layout, Sense, Vec2};

use crate::app::AppState;
use crate::config::{LaunchOutcome, QuickTarget};
use crate::roblox::launch::LaunchTarget;
use crate::roblox::uri;
use crate::util::format;

use crate::ui::chrome::request_launch;
use crate::ui::icons::{self, Icon};
use crate::ui::theme::{self, Theme};
use crate::ui::widgets::{self, feedback};
use crate::ui::{appicon, Page, UiState};

enum Alert {
    InstallRoblox,
    OpenFlags,
    OpenAbout,
    OpenGame,
    Unlock,
}

pub fn render(ui: &mut egui::Ui, state: &mut AppState, ui_state: &mut UiState) {
    let theme = Theme::get(ui.ctx());
    let installed = state.detection.active().is_some();
    state.refresh_denied_flags();
    state.refresh_game_snapshot(false);

    widgets::page_header(
        ui,
        "Home",
        "Everything RustBlox knows about your Roblox, in one place.",
        |_| {},
    );

    if !installed {
        missing_install(ui, &theme, state, ui_state);
        return;
    }

    hero(ui, &theme, state, ui_state);
    ui.add_space(theme.metrics.gap_lg);
    alerts(ui, &theme, state, ui_state);
    glance(ui, &theme, state, ui_state);
    ui.add_space(theme.metrics.gap_lg);
    quick_launch(ui, &theme, state, ui_state);
    ui.add_space(theme.metrics.gap_lg);
    activity(ui, &theme, state);
}

fn missing_install(ui: &mut egui::Ui, theme: &Theme, state: &mut AppState, ui_state: &mut UiState) {
    let scanning = state.tasks.is_scanning();
    let can_install = state.can_install();
    let advanced = state.settings.advanced_mode;
    let mut go_to_installation = false;
    let mut pick = false;
    let mut install = false;

    widgets::card(ui, |ui| {
        widgets::empty_state(
            ui,
            if scanning {
                Icon::Search
            } else {
                Icon::Package
            },
            if scanning {
                "Looking for Roblox"
            } else {
                "Roblox is not installed yet"
            },
            if scanning {
                "Checking the folder RustBlox installs into."
            } else {
                "Install it here and RustBlox keeps its own copy, separate from anything Roblox installed."
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
                    if advanced {
                        go_to_installation = widgets::Button::new("Details")
                            .tone(widgets::Tone::Ghost)
                            .show(ui)
                            .clicked();
                    }
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
    let target = state.default_target();
    let version = state
        .detection
        .active()
        .map(|install| install.display_version().to_owned())
        .unwrap_or_default();
    let update = state
        .update_available()
        .map(|deployment| deployment.version.clone());
    let mut launch = false;

    widgets::card(ui, |ui| {
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(46.0), Sense::hover());
            appicon::paint(ui, rect, theme.radius_md());
            ui.add_space(theme.metrics.gap_md);

            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    let (tone, label) = if running {
                        (feedback::Tone::Success, state.roblox.summary())
                    } else {
                        (feedback::Tone::Neutral, "Client idle".to_string())
                    };
                    widgets::status_pill(ui, &label, tone, running);
                    widgets::badge(ui, &version, feedback::Tone::Neutral);
                    if update.is_some() {
                        widgets::badge(ui, "update waiting", feedback::Tone::Accent);
                    }
                });

                ui.add_space(theme.metrics.gap_sm);
                ui.label(
                    egui::RichText::new(target.headline())
                        .font(theme::strong(theme::size::DISPLAY))
                        .color(palette.text),
                );
                ui.add_space(3.0);
                ui.label(
                    egui::RichText::new(target.detail())
                        .font(theme::text_style(theme::size::SMALL))
                        .color(palette.text_muted),
                );
            });
        });

        ui.add_space(theme.metrics.gap_lg);
        ui.horizontal(|ui| {
            launch = widgets::Button::primary("Launch Roblox")
                .icon(Icon::Rocket)
                .enabled(can_launch)
                .min_width(180.0)
                .show(ui)
                .clicked();

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(match &update {
                        Some(version) => format!("Roblox {version} will be installed first."),
                        None => "Checks for a Roblox update first.".to_owned(),
                    })
                    .font(theme::text_style(theme::size::MICRO))
                    .color(palette.text_faint),
                );
            });
        });
    });

    if launch {
        request_launch(state, ui_state, target);
    }
}

fn alerts(ui: &mut egui::Ui, theme: &Theme, state: &mut AppState, ui_state: &mut UiState) {
    let advanced = state.settings.advanced_mode;
    let update = state
        .update_available()
        .map(|deployment| deployment.version.clone());
    let app_update = state
        .app_update
        .offered()
        .map(|release| release.version.clone());
    let refused = state.denied_active_flags();
    let stray_lock = state.game.locked && !state.settings.game.manage;
    let unmanaged = !state.settings.game.manage && !state.persisted.seen_game_page;

    let nothing = update.is_none()
        && app_update.is_none()
        && (refused.is_empty() || !advanced)
        && !stray_lock
        && !unmanaged;
    if nothing {
        return;
    }

    let mut action = None;

    widgets::section(
        ui,
        "Worth a look",
        Some("Only what RustBlox can actually do something about."),
        |ui| {
            let mut first = true;
            let gap = |ui: &mut egui::Ui, first: &mut bool| {
                if *first {
                    *first = false;
                } else {
                    ui.add_space(theme.metrics.gap_sm);
                }
            };

            if let Some(version) = &update {
                gap(ui, &mut first);
                if alert(
                    ui,
                    theme,
                    feedback::Tone::Accent,
                    Icon::Package,
                    "Roblox has an update",
                    &format!(
                        "Version {version} is out. Launching installs it first, or take it now."
                    ),
                    "Install it",
                ) {
                    action = Some(Alert::InstallRoblox);
                }
            }

            if !refused.is_empty() && advanced {
                gap(ui, &mut first);
                if alert(
                    ui,
                    theme,
                    feedback::Tone::Danger,
                    Icon::Flag,
                    "The client refused some flags",
                    &format!(
                        "{} came back refused on the last launch, so they are doing nothing.",
                        refused.join(", ")
                    ),
                    "Open Flags",
                ) {
                    action = Some(Alert::OpenFlags);
                }
            }

            if stray_lock {
                gap(ui, &mut first);
                if alert(
                    ui,
                    theme,
                    feedback::Tone::Warning,
                    Icon::Warning,
                    "Roblox cannot save its own settings",
                    "Its settings file is read only and RustBlox is not managing it, so anything you change in game is forgotten.",
                    "Unlock it",
                ) {
                    action = Some(Alert::Unlock);
                }
            } else if unmanaged {
                gap(ui, &mut first);
                if alert(
                    ui,
                    theme,
                    feedback::Tone::Info,
                    Icon::Gauge,
                    "The frame rate limit is Roblox's own",
                    "RustBlox can set the frame rate, the graphics quality and the stats overlay for you before each launch.",
                    "Open Game",
                ) {
                    action = Some(Alert::OpenGame);
                }
            }

            if let Some(version) = &app_update {
                gap(ui, &mut first);
                if alert(
                    ui,
                    theme,
                    feedback::Tone::Info,
                    Icon::Refresh,
                    "RustBlox has an update",
                    &format!(
                        "Version {version} is on GitHub. Nothing is downloaded until you say so."
                    ),
                    "See it",
                ) {
                    action = Some(Alert::OpenAbout);
                }
            }
        },
    );
    ui.add_space(theme.metrics.gap_lg);

    match action {
        Some(Alert::InstallRoblox) => state.install_roblox(false),
        Some(Alert::OpenFlags) => ui_state.page = Page::Flags,
        Some(Alert::OpenAbout) => ui_state.page = Page::About,
        Some(Alert::OpenGame) => ui_state.page = Page::Game,
        Some(Alert::Unlock) => state.unlock_game_settings(),
        None => {}
    }
}

fn alert(
    ui: &mut egui::Ui,
    theme: &Theme,
    tone: feedback::Tone,
    icon: Icon,
    title: &str,
    body: &str,
    button: &str,
) -> bool {
    let mut clicked = false;

    widgets::nested(ui, |ui| {
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(18.0), Sense::hover());
            icons::draw(ui.painter(), icon, rect, tone.color(theme), 1.7);
            ui.add_space(theme.metrics.gap_xs);

            ui.vertical(|ui| {
                ui.set_width((ui.available_width() - 130.0).max(80.0));
                ui.label(
                    egui::RichText::new(title)
                        .font(theme::medium(theme::size::BODY))
                        .color(theme.palette.text),
                );
                ui.add_space(2.0);
                ui.label(
                    egui::RichText::new(body)
                        .font(theme::text_style(theme::size::SMALL))
                        .color(theme.palette.text_muted),
                );
            });

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                clicked = widgets::Button::new(button)
                    .size(widgets::Size::Small)
                    .show(ui)
                    .clicked();
            });
        });
    });

    clicked
}

fn glance(ui: &mut egui::Ui, theme: &Theme, state: &mut AppState, ui_state: &mut UiState) {
    let advanced = state.settings.advanced_mode;
    let version = state
        .detection
        .active()
        .map(|install| install.display_version().to_owned())
        .unwrap_or_else(|| "none".into());
    let flags = state.flags.active_count();
    let managed = state.settings.game.changes().len();
    let mut open_installation = false;

    widgets::section(
        ui,
        "At a glance",
        Some("What RustBlox has on disk and what it is writing."),
        |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = theme.metrics.gap_xl;
                widgets::stat(ui, "Roblox", &version, feedback::Tone::Accent);
                widgets::stat(
                    ui,
                    "Launches",
                    &state.persisted.launch_count.to_string(),
                    feedback::Tone::Neutral,
                );
                widgets::stat(
                    ui,
                    "Game settings",
                    &managed.to_string(),
                    if managed > 0 {
                        feedback::Tone::Success
                    } else {
                        feedback::Tone::Neutral
                    },
                );
                if advanced {
                    widgets::stat(
                        ui,
                        "Flags",
                        &flags.to_string(),
                        if flags > 0 {
                            feedback::Tone::Success
                        } else {
                            feedback::Tone::Neutral
                        },
                    );
                }

                if advanced {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        open_installation = widgets::Button::new("Installation")
                            .icon(Icon::ChevronRight)
                            .tone(widgets::Tone::Ghost)
                            .size(widgets::Size::Small)
                            .show(ui)
                            .clicked();
                    });
                }
            });
        },
    );

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
                            icons::draw(
                                ui.painter(),
                                Icon::Play,
                                icon_rect,
                                theme.palette.accent,
                                1.6,
                            );

                            ui.vertical(|ui| {
                                ui.set_width((ui.available_width() - 160.0).max(70.0));
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

            ui.add_space(theme.metrics.gap_sm);

            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = theme.metrics.gap_sm;
                let field_width = ((ui.available_width() - 200.0) * 0.5).clamp(70.0, 260.0);
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

    widgets::section(ui, "Last launch", None, |ui| {
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
        ui.horizontal(|ui| {
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

use std::path::PathBuf;

use egui::{Align, Layout};

use crate::app::{AppState, DEEPLINK_SCHEME, PLAYER_SCHEME};
use crate::platform::SchemeOwner;
use crate::roblox::deploy::Deployment;
use crate::roblox::install::{format_size, Installation};
use crate::util::format;

use crate::ui::icons::Icon;
use crate::ui::theme::{self, Theme};
use crate::ui::widgets::{self, feedback};
use crate::ui::UiState;

enum Action {
    Select(String),
    Pin(Option<String>),
    Remove(String),
    OpenPath(PathBuf),
    ClearCustomRoot,
    PickCustomRoot,
    Rescan,
    Register(&'static str),
    Restore(&'static str),
    Install { force: bool },
}

pub fn render(ui: &mut egui::Ui, state: &mut AppState, ui_state: &mut UiState) {
    let theme = Theme::get(ui.ctx());
    let mut action = None;

    widgets::page_header(
        ui,
        "Installation",
        "Where the client lives and how launches reach it.",
        |ui| {
            if widgets::Button::new("Rescan")
                .icon(Icon::Refresh)
                .tone(widgets::Tone::Ghost)
                .enabled(!state.tasks.is_scanning())
                .show(ui)
                .clicked()
            {
                action = Some(Action::Rescan);
            }
        },
    );

    match state.detection.active().cloned() {
        Some(install) => active_card(ui, &theme, state, &install, &mut action),
        None => {
            widgets::card(ui, |ui| {
                widgets::empty_state(
                    ui,
                    Icon::Search,
                    "No installation selected",
                    "Pick a folder holding RobloxPlayerBeta.exe, or one with a Versions subfolder.",
                    |_| {},
                );
            });
        }
    }

    ui.add_space(theme.metrics.gap_lg);
    managed_install(ui, &theme, state, &mut action);
    ui.add_space(theme.metrics.gap_lg);
    all_installs(ui, &theme, state, ui_state, &mut action);
    ui.add_space(theme.metrics.gap_lg);
    search_locations(ui, &theme, state, ui_state, &mut action);
    ui.add_space(theme.metrics.gap_lg);
    integrations(ui, &theme, state, &mut action);

    if let Some(action) = action {
        apply(state, action);
    }
}

fn managed_install(
    ui: &mut egui::Ui,
    theme: &Theme,
    state: &AppState,
    action: &mut Option<Action>,
) {
    let can_install = state.can_install();
    let channel = state.settings.advanced.channel.clone();
    let root = state.store.paths().versions_dir();
    let owned = state
        .detection
        .installations
        .iter()
        .filter(|install| install.source == crate::roblox::install::InstallSource::Ours)
        .count();
    let update = state.update_available().cloned();
    let checking = state.tasks.is_checking();
    let known = state.latest.clone();

    widgets::section(
        ui,
        "Download from Roblox",
        Some("Official packages, checksummed and unpacked by RustBlox."),
        |ui| {
            widgets::detail_row(ui, "Release channel", &channel, false);
            ui.add_space(theme.metrics.gap_sm);
            widgets::detail_row(ui, "Installs into", &root.display().to_string(), true);
            ui.add_space(theme.metrics.gap_sm);
            widgets::detail_row(ui, "Copies managed here", &owned.to_string(), false);
            ui.add_space(theme.metrics.gap_sm);
            widgets::detail_row(
                ui,
                "Latest on this channel",
                &latest_line(checking, known.as_ref()),
                false,
            );

            if let Some(update) = &update {
                ui.add_space(theme.metrics.gap_md);
                widgets::banner(
                    ui,
                    feedback::Tone::Accent,
                    "An update is available",
                    &format!(
                        "Roblox {} has been released. Installing it keeps your settings and flags.",
                        update.version
                    ),
                );
            }

            ui.add_space(theme.metrics.gap_md);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = theme.metrics.gap_sm;

                let label = match &update {
                    Some(update) if owned > 0 => format!("Update to {}", update.version),
                    _ => "Install or update".to_string(),
                };

                if widgets::Button::primary(&label)
                    .icon(Icon::Package)
                    .enabled(can_install)
                    .show(ui)
                    .clicked()
                {
                    *action = Some(Action::Install { force: false });
                }

                if widgets::Button::new("Reinstall")
                    .icon(Icon::Refresh)
                    .tone(widgets::Tone::Ghost)
                    .enabled(can_install)
                    .show(ui)
                    .clicked()
                {
                    *action = Some(Action::Install { force: true });
                }

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if owned > 0
                        && widgets::Button::new("Open folder")
                            .tone(widgets::Tone::Quiet)
                            .size(widgets::Size::Small)
                            .show(ui)
                            .clicked()
                    {
                        *action = Some(Action::OpenPath(root.clone()));
                    }
                });
            });

            ui.add_space(theme.metrics.gap_sm);
            widgets::banner(
                ui,
                feedback::Tone::Info,
                "This does not touch your other copies",
                "A copy installed by Roblox itself is read, never modified.",
            );
        },
    );
}

fn latest_line(checking: bool, latest: Option<&Deployment>) -> String {
    match (checking, latest) {
        (_, Some(latest)) => latest.version.clone(),
        (true, None) => "checking".into(),
        (false, None) => "not known yet".into(),
    }
}

fn active_card(
    ui: &mut egui::Ui,
    theme: &Theme,
    state: &AppState,
    install: &Installation,
    action: &mut Option<Action>,
) {
    let integrity = install.integrity();
    let size = install.size_on_disk();

    widgets::card(ui, |ui| {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.set_width((ui.available_width() - 200.0).max(240.0));
                ui.horizontal(|ui| {
                    widgets::badge(ui, "Active", feedback::Tone::Accent);
                    widgets::badge(ui, install.source.short(), feedback::Tone::Neutral);
                    if integrity.is_ok() {
                        widgets::badge(ui, "Verified", feedback::Tone::Success);
                    } else {
                        widgets::badge(ui, "Needs attention", feedback::Tone::Warning);
                    }
                });
                ui.add_space(theme.metrics.gap_sm);
                ui.label(
                    egui::RichText::new(format!("Roblox {}", install.display_version()))
                        .font(theme::strong(theme::size::TITLE))
                        .color(theme.palette.text),
                );
                ui.add_space(2.0);
                ui.label(
                    egui::RichText::new(install.version_dir.display().to_string())
                        .font(egui::FontId::new(
                            theme::size::MICRO,
                            egui::FontFamily::Monospace,
                        ))
                        .color(theme.palette.text_faint),
                );
            });

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if widgets::Button::new("Open folder")
                    .icon(Icon::Folder)
                    .tone(widgets::Tone::Ghost)
                    .show(ui)
                    .clicked()
                {
                    *action = Some(Action::OpenPath(install.version_dir.clone()));
                }
            });
        });

        ui.add_space(theme.metrics.gap_md);
        widgets::separator(ui);
        ui.add_space(theme.metrics.gap_md);

        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = theme.metrics.gap_sm;
            widgets::detail_row(
                ui,
                "Executable",
                &install.player.display().to_string(),
                true,
            );
            widgets::detail_row(ui, "Version folder", &install.folder_id, true);
            widgets::detail_row(ui, "Source", install.source.label(), false);
            widgets::detail_row(
                ui,
                "Crash handler",
                if install.has_crash_handler() {
                    "Present"
                } else {
                    "Missing"
                },
                false,
            );
            if let Some(size) = size {
                widgets::detail_row(ui, "Size on disk", &format_size(size), false);
            }
            widgets::detail_row(
                ui,
                "Last scan",
                &format::relative_time(state.detection.scanned_at),
                false,
            );
        });

        if !integrity.is_ok() {
            ui.add_space(theme.metrics.gap_md);
            widgets::banner(
                ui,
                feedback::Tone::Warning,
                "Some expected files are missing",
                &integrity.problems.join("; "),
            );
        }
    });
}

fn all_installs(
    ui: &mut egui::Ui,
    theme: &Theme,
    state: &AppState,
    ui_state: &mut UiState,
    action: &mut Option<Action>,
) {
    let installs = state.detection.installations.clone();
    let active = state
        .detection
        .active()
        .map(|install| install.folder_id.clone());
    let pinned = state.settings.advanced.pinned_version_folder.clone();
    let managed = state.managed_folders();
    let busy = state.tasks.is_sweeping();
    let mut arm: Option<Option<String>> = None;

    widgets::section(
        ui,
        "Detected installations",
        Some("Pinning survives updates. Removing deletes the folder."),
        |ui| {
            if installs.is_empty() {
                ui.label(
                    egui::RichText::new("Nothing detected yet.")
                        .font(theme::text_style(theme::size::SMALL))
                        .color(theme.palette.text_muted),
                );
                return;
            }

            for install in &installs {
                let is_active = active.as_deref() == Some(install.folder_id.as_str());
                let is_pinned = pinned.as_deref() == Some(install.folder_id.as_str());
                let is_ours = managed.iter().any(|folder| folder == &install.folder_id);
                let is_armed =
                    ui_state.pending_removal.as_deref() == Some(install.folder_id.as_str());

                widgets::nested(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.set_width((ui.available_width() - 250.0).max(150.0));
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(install.display_version())
                                        .font(theme::medium(theme::size::BODY))
                                        .color(theme.palette.text),
                                );
                                if is_active {
                                    widgets::badge(ui, "Active", feedback::Tone::Accent);
                                }
                                if is_pinned {
                                    widgets::badge(ui, "Pinned", feedback::Tone::Info);
                                }
                            });
                            ui.label(
                                egui::RichText::new(format!(
                                    "{} - {}",
                                    install.source.short(),
                                    install.folder_id
                                ))
                                .font(theme::text_style(theme::size::MICRO))
                                .color(theme.palette.text_faint),
                            );
                        });

                        if is_armed {
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if widgets::Button::new("Delete it")
                                    .icon(Icon::Trash)
                                    .tone(widgets::Tone::Danger)
                                    .size(widgets::Size::Small)
                                    .enabled(!busy)
                                    .show(ui)
                                    .clicked()
                                {
                                    *action = Some(Action::Remove(install.folder_id.clone()));
                                    arm = Some(None);
                                }

                                if widgets::Button::new("Cancel")
                                    .tone(widgets::Tone::Ghost)
                                    .size(widgets::Size::Small)
                                    .show(ui)
                                    .clicked()
                                {
                                    arm = Some(None);
                                }

                                ui.label(
                                    egui::RichText::new("Remove this folder from disk?")
                                        .font(theme::text_style(theme::size::SMALL))
                                        .color(theme.palette.text_muted),
                                );
                            });
                            return;
                        }

                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if is_ours
                                && !is_active
                                && widgets::icon_button(ui, Icon::Trash, "Remove", !busy).clicked()
                            {
                                arm = Some(Some(install.folder_id.clone()));
                            }

                            if widgets::icon_button(
                                ui,
                                Icon::Pin,
                                if is_pinned {
                                    "Unpin"
                                } else {
                                    "Pin this version"
                                },
                                true,
                            )
                            .clicked()
                            {
                                *action = Some(Action::Pin(if is_pinned {
                                    None
                                } else {
                                    Some(install.folder_id.clone())
                                }));
                            }

                            if widgets::icon_button(ui, Icon::Folder, "Open folder", true).clicked()
                            {
                                *action = Some(Action::OpenPath(install.version_dir.clone()));
                            }

                            if widgets::Button::new("Use")
                                .size(widgets::Size::Small)
                                .enabled(!is_active)
                                .show(ui)
                                .clicked()
                            {
                                *action = Some(Action::Select(install.folder_id.clone()));
                            }
                        });
                    });
                });
                ui.add_space(theme.metrics.gap_sm);
            }
        },
    );

    if let Some(next) = arm {
        ui_state.pending_removal = next;
    }
}

fn search_locations(
    ui: &mut egui::Ui,
    theme: &Theme,
    state: &AppState,
    ui_state: &mut UiState,
    action: &mut Option<Action>,
) {
    let custom = state.settings.advanced.custom_install_root.clone();
    let searched = state.detection.searched.clone();

    widgets::section(
        ui,
        "Search locations",
        Some("Checked in order, first match wins."),
        |ui| {
            widgets::setting_row(
                ui,
                "Custom install folder",
                &custom
                    .as_ref()
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|| "Not set".into()),
                |ui| {
                    if widgets::Button::new("Browse")
                        .icon(Icon::Folder)
                        .size(widgets::Size::Small)
                        .show(ui)
                        .clicked()
                    {
                        *action = Some(Action::PickCustomRoot);
                    }
                    if custom.is_some()
                        && widgets::Button::new("Clear")
                            .tone(widgets::Tone::Ghost)
                            .size(widgets::Size::Small)
                            .show(ui)
                            .clicked()
                    {
                        *action = Some(Action::ClearCustomRoot);
                    }
                },
            );

            ui.add_space(theme.metrics.gap_md);
            widgets::separator(ui);
            ui.add_space(theme.metrics.gap_sm);

            let label = if ui_state.show_searched_paths {
                "Hide checked folders"
            } else {
                "Show checked folders"
            };
            if widgets::Button::new(label)
                .icon(if ui_state.show_searched_paths {
                    Icon::ChevronDown
                } else {
                    Icon::ChevronRight
                })
                .tone(widgets::Tone::Quiet)
                .size(widgets::Size::Small)
                .show(ui)
                .clicked()
            {
                ui_state.show_searched_paths = !ui_state.show_searched_paths;
            }

            if ui_state.show_searched_paths {
                ui.add_space(theme.metrics.gap_sm);
                widgets::nested(ui, |ui| {
                    for path in &searched {
                        let exists = path.is_dir();
                        ui.horizontal(|ui| {
                            widgets::badge(
                                ui,
                                if exists { "found" } else { "absent" },
                                if exists {
                                    feedback::Tone::Success
                                } else {
                                    feedback::Tone::Neutral
                                },
                            );
                            ui.label(
                                egui::RichText::new(path.display().to_string())
                                    .font(egui::FontId::new(
                                        theme::size::MICRO,
                                        egui::FontFamily::Monospace,
                                    ))
                                    .color(theme.palette.text_muted),
                            );
                        });
                    }
                });
            }

            if !state.detection.notes.is_empty() {
                ui.add_space(theme.metrics.gap_sm);
                for note in &state.detection.notes {
                    ui.label(
                        egui::RichText::new(note)
                            .font(theme::text_style(theme::size::MICRO))
                            .color(theme.palette.text_faint),
                    );
                }
            }
        },
    );
}

fn integrations(ui: &mut egui::Ui, theme: &Theme, state: &AppState, action: &mut Option<Action>) {
    widgets::section(
        ui,
        "Launch link handling",
        Some(
            "Roblox registers itself for roblox-player links so the website can start the client. RustBlox can take that over and pass the link straight through.",
        ),
        |ui| {
            for (scheme, description) in [
                (
                    PLAYER_SCHEME,
                    "Used by the Roblox website when you press Play.",
                ),
                (
                    DEEPLINK_SCHEME,
                    "Used by deep links such as roblox://experiences/start.",
                ),
            ] {
                let registration = if scheme == PLAYER_SCHEME {
                    state.protocol.clone()
                } else {
                    state.deeplink.clone()
                };
                let owner = registration
                    .as_ref()
                    .map(|entry| entry.owner)
                    .unwrap_or(SchemeOwner::Unregistered);

                widgets::nested(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.set_width((ui.available_width() - 190.0).max(180.0));
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(format!("{scheme}:"))
                                        .font(theme::medium(theme::size::BODY))
                                        .color(theme.palette.text),
                                );
                                widgets::badge(
                                    ui,
                                    owner.label(),
                                    match owner {
                                        SchemeOwner::Ours => feedback::Tone::Accent,
                                        SchemeOwner::Roblox => feedback::Tone::Success,
                                        SchemeOwner::Other => feedback::Tone::Warning,
                                        SchemeOwner::Unregistered => feedback::Tone::Neutral,
                                    },
                                );
                            });
                            ui.label(
                                egui::RichText::new(description)
                                    .font(theme::text_style(theme::size::MICRO))
                                    .color(theme.palette.text_muted),
                            );
                            if let Some(command) =
                                registration.as_ref().and_then(|entry| entry.command.clone())
                            {
                                ui.label(
                                    egui::RichText::new(crate::util::format::truncate_middle(
                                        &command, 64,
                                    ))
                                    .font(egui::FontId::new(
                                        theme::size::MICRO,
                                        egui::FontFamily::Monospace,
                                    ))
                                    .color(theme.palette.text_faint),
                                )
                                .on_hover_text(command);
                            }
                        });

                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if owner == SchemeOwner::Ours {
                                if widgets::Button::new("Hand back")
                                    .size(widgets::Size::Small)
                                    .show(ui)
                                    .clicked()
                                {
                                    *action = Some(Action::Restore(scheme));
                                }
                            } else if widgets::Button::new("Take over")
                                .size(widgets::Size::Small)
                                .tone(widgets::Tone::Neutral)
                                .show(ui)
                                .clicked()
                            {
                                *action = Some(Action::Register(scheme));
                            }
                        });
                    });
                });
                ui.add_space(theme.metrics.gap_sm);
            }

            widgets::banner(
                ui,
                feedback::Tone::Info,
                "What taking over does",
                "The current handler is saved first. Links are passed to the client unchanged, so the sign-in ticket still works. Hand back restores what was there before.");
        },
    );
}

fn apply(state: &mut AppState, action: Action) {
    match action {
        Action::Rescan => {
            state.rescan();
            state.check_latest();
        }
        Action::Remove(folder) => state.remove_version(folder),
        Action::Select(folder) => {
            if state.detection.select_folder(&folder) {
                state.refresh_applied_flags();
            }
        }
        Action::Pin(folder) => {
            state.settings.advanced.pinned_version_folder = folder;
            state.mark_settings_dirty();
            state.flush_settings();
            state.rescan();
        }
        Action::OpenPath(path) => state.open_path(path),
        Action::ClearCustomRoot => {
            state.settings.advanced.custom_install_root = None;
            state.mark_settings_dirty();
            state.flush_settings();
            state.rescan();
        }
        Action::PickCustomRoot => {
            if let Some(folder) = state.pick_install_folder() {
                state.settings.advanced.custom_install_root = Some(folder);
                state.mark_settings_dirty();
                state.flush_settings();
                state.rescan();
            }
        }
        Action::Install { force } => state.install_roblox(force),
        Action::Register(scheme) => state.register_protocol(scheme),
        Action::Restore(scheme) => state.restore_protocol(scheme),
    }
}

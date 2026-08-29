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
    OpenPath(PathBuf),
    Register(&'static str),
    Restore(&'static str),
    Install { force: bool },
    CleanCache,
}

pub fn render(ui: &mut egui::Ui, state: &mut AppState, _ui_state: &mut UiState) {
    let theme = Theme::get(ui.ctx());
    let mut action = None;

    if state.latest.is_none() && state.latest_note.is_none() {
        state.check_latest();
    }

    widgets::page_header(
        ui,
        "Installation",
        "The copy of Roblox RustBlox keeps for itself.",
        |_| {},
    );

    match state.detection.active().cloned() {
        Some(install) => active_card(ui, &theme, state, &install, &mut action),
        None => {
            widgets::card(ui, |ui| {
                widgets::empty_state(
                    ui,
                    Icon::Package,
                    "Roblox is not installed yet",
                    "Install it below and RustBlox will keep its own copy, separate from anything Roblox installed.",
                    |_| {},
                );
            });
        }
    }

    ui.add_space(theme.metrics.gap_lg);
    managed_install(ui, &theme, state, &mut action);
    ui.add_space(theme.metrics.gap_lg);
    cleanup(ui, &theme, &mut action);
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
                "RustBlox only ever uses its own copy",
                "A Roblox install of its own is never read, launched or modified, so the two never get in each other's way.",
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
                ui.set_width((ui.available_width() - 200.0).max(80.0));
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
                            ui.set_width((ui.available_width() - 190.0).max(80.0));
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

fn cleanup(ui: &mut egui::Ui, _theme: &Theme, action: &mut Option<Action>) {
    widgets::section(
        ui,
        "Storage & Cache Cleaner",
        Some("Clears accumulated Roblox HTTP caches, log files, and old crash dumps."),
        |ui| {
            widgets::setting_row(
                ui,
                "Clean temporary caches",
                "Frees disk space by clearing temporary downloads, textures, and crash dumps without affecting settings.",
                |ui| {
                    if widgets::Button::new("Clean cache now")
                        .icon(Icon::Trash)
                        .tone(widgets::Tone::Neutral)
                        .size(widgets::Size::Small)
                        .show(ui)
                        .clicked()
                    {
                        *action = Some(Action::CleanCache);
                    }
                },
            );
        },
    );
}

fn apply(state: &mut AppState, action: Action) {
    match action {
        Action::OpenPath(path) => state.open_path(path),
        Action::Install { force } => state.install_roblox(force),
        Action::Register(scheme) => state.register_protocol(scheme),
        Action::Restore(scheme) => state.restore_protocol(scheme),
        Action::CleanCache => {
            let freed = state.clean_roblox_cache();
            state
                .toasts
                .success(format!("Freed {} of cache and dumps", format_size(freed)));
        }
    }
}

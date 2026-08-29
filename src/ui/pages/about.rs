use egui::{Align, Layout};

use crate::app::{AppState, UpdatePhase};
use crate::roblox::install::format_size;
use crate::selfupdate;
use crate::util::log;

use crate::ui::icons::Icon;
use crate::ui::theme::{self, Theme};
use crate::ui::widgets::{self, feedback};
use crate::ui::UiState;

const REPOSITORY: &str = selfupdate::REPOSITORY;

enum Action {
    Check,
    Download,
    Restart,
}

fn updates(
    ui: &mut egui::Ui,
    theme: &Theme,
    state: &AppState,
    open_url: &mut Option<String>,
    action: &mut Option<Action>,
) {
    let update = &state.app_update;
    let busy = state.tasks.is_app_busy() || update.phase.is_busy();

    widgets::section(
        ui,
        "Updates",
        Some("Checked on startup. Nothing installs until you ask."),
        |ui| {
            widgets::detail_row(ui, "This build", selfupdate::current_version(), false);
            ui.add_space(theme.metrics.gap_sm);
            widgets::detail_row(
                ui,
                "Newest release",
                &match (&update.available, update.phase) {
                    (Some(release), _) => release.version.clone(),
                    (None, UpdatePhase::Checking) => "checking".into(),
                    (None, _) => "up to date".into(),
                },
                false,
            );

            if update.phase == UpdatePhase::Downloading {
                ui.add_space(theme.metrics.gap_md);
                widgets::progress_bar(
                    ui,
                    update.fraction(),
                    "Downloading the new build",
                    &format!(
                        "{} of {}",
                        format_size(update.done),
                        format_size(update.total)
                    ),
                );
            }

            if update.phase == UpdatePhase::Ready {
                ui.add_space(theme.metrics.gap_md);
                widgets::banner(
                    ui,
                    feedback::Tone::Success,
                    "The new build is in place",
                    "Restart to start using it.",
                );
            } else if let Some(release) = update.offered() {
                ui.add_space(theme.metrics.gap_md);
                widgets::banner(
                    ui,
                    feedback::Tone::Accent,
                    &format!("RustBlox {} is out", release.version),
                    "Keeps your settings, flags and installed copies.",
                );
            }

            if let Some(message) = &update.message {
                ui.add_space(theme.metrics.gap_sm);
                widgets::banner(
                    ui,
                    feedback::Tone::Warning,
                    "The update check had a problem",
                    message,
                );
            }

            ui.add_space(theme.metrics.gap_md);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = theme.metrics.gap_sm;

                if update.phase == UpdatePhase::Ready {
                    if widgets::Button::primary("Restart now")
                        .icon(Icon::Refresh)
                        .show(ui)
                        .clicked()
                    {
                        *action = Some(Action::Restart);
                    }
                } else if update.offered().is_some() {
                    if widgets::Button::primary("Download and install")
                        .icon(Icon::Package)
                        .enabled(!busy)
                        .show(ui)
                        .clicked()
                    {
                        *action = Some(Action::Download);
                    }
                } else if widgets::Button::new("Check for updates")
                    .icon(Icon::Refresh)
                    .tone(widgets::Tone::Neutral)
                    .enabled(!busy)
                    .show(ui)
                    .clicked()
                {
                    *action = Some(Action::Check);
                }

                if let Some(release) = &update.available {
                    if widgets::Button::new("Release notes")
                        .icon(Icon::External)
                        .tone(widgets::Tone::Ghost)
                        .size(widgets::Size::Small)
                        .show(ui)
                        .clicked()
                    {
                        *open_url = Some(release.page.clone());
                    }
                }
            });
        },
    );
}

pub fn render(ui: &mut egui::Ui, state: &mut AppState, ui_state: &mut UiState) {
    let theme = Theme::get(ui.ctx());
    let mut open_url = None;
    let mut open_path = None;
    let mut action = None;

    widgets::page_header(ui, "About", "Build details and updates.", |_| {});

    widgets::card(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = theme.metrics.gap_lg;
            widgets::stat(
                ui,
                "Version",
                env!("CARGO_PKG_VERSION"),
                feedback::Tone::Accent,
            );
            widgets::stat(ui, "Interface", "egui / eframe", feedback::Tone::Neutral);
            widgets::stat(ui, "Target", std::env::consts::OS, feedback::Tone::Neutral);

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if widgets::Button::new("Repository")
                    .icon(Icon::External)
                    .tone(widgets::Tone::Ghost)
                    .size(widgets::Size::Small)
                    .show(ui)
                    .clicked()
                {
                    open_url = Some(REPOSITORY.to_owned());
                }
            });
        });
    });

    ui.add_space(theme.metrics.gap_lg);
    updates(ui, &theme, state, &mut open_url, &mut action);

    ui.add_space(theme.metrics.gap_lg);

    widgets::section(ui, "Diagnostics", None, |ui| {
        widgets::detail_row(
            ui,
            "Settings file",
            &state.store.paths().settings_file().display().to_string(),
            true,
        );
        ui.add_space(theme.metrics.gap_sm);
        widgets::detail_row(
            ui,
            "Log file",
            &state.store.paths().log_file().display().to_string(),
            true,
        );
        ui.add_space(theme.metrics.gap_sm);
        widgets::detail_row(
            ui,
            "Executable",
            &state
                .exe_path
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "unknown".into()),
            true,
        );

        if !state.startup_notes.is_empty() {
            ui.add_space(theme.metrics.gap_md);
            widgets::banner(
                ui,
                feedback::Tone::Warning,
                "Notes from this session",
                &state.startup_notes.join("  -  "),
            );
        }

        ui.add_space(theme.metrics.gap_md);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = theme.metrics.gap_sm;
            if widgets::Button::new(if ui_state.show_log {
                "Hide recent log"
            } else {
                "Show recent log"
            })
            .icon(if ui_state.show_log {
                Icon::ChevronDown
            } else {
                Icon::ChevronRight
            })
            .tone(widgets::Tone::Ghost)
            .size(widgets::Size::Small)
            .show(ui)
            .clicked()
            {
                ui_state.show_log = !ui_state.show_log;
            }

            if widgets::Button::new("Open log folder")
                .icon(Icon::Folder)
                .tone(widgets::Tone::Ghost)
                .size(widgets::Size::Small)
                .show(ui)
                .clicked()
            {
                open_path = state
                    .store
                    .paths()
                    .log_file()
                    .parent()
                    .map(|path| path.to_path_buf());
            }
        });

        if ui_state.show_log {
            ui.add_space(theme.metrics.gap_sm);
            let entries = log::recent(60);
            widgets::nested(ui, |ui| {
                if entries.is_empty() {
                    ui.label(
                        egui::RichText::new("Nothing logged yet.")
                            .font(theme::text_style(theme::size::SMALL))
                            .color(theme.palette.text_muted),
                    );
                }
                egui::ScrollArea::vertical()
                    .max_height(220.0)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        for entry in entries.iter().rev() {
                            let color = match entry.level {
                                log::Level::Info => theme.palette.text_muted,
                                log::Level::Warn => theme.palette.warning,
                                log::Level::Error => theme.palette.danger,
                            };
                            ui.label(
                                egui::RichText::new(format!(
                                    "{}  {}",
                                    entry.at.format("%H:%M:%S"),
                                    entry.message
                                ))
                                .font(egui::FontId::new(
                                    theme::size::MICRO,
                                    egui::FontFamily::Monospace,
                                ))
                                .color(color),
                            );
                        }
                    });
            });
        }
    });

    finish(state, open_url, open_path, action);
}

fn finish(
    state: &mut AppState,
    open_url: Option<String>,
    open_path: Option<std::path::PathBuf>,
    action: Option<Action>,
) {
    if let Some(url) = open_url {
        state.open_url(&url);
    }
    if let Some(path) = open_path {
        state.open_path(path);
    }
    match action {
        Some(Action::Check) => state.check_app_update(),
        Some(Action::Download) => state.start_app_update(),
        Some(Action::Restart) => state.restart_for_update(),
        None => {}
    }
}

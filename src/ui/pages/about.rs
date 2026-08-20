use egui::{Align, Layout};

use crate::app::AppState;
use crate::util::log;

use crate::ui::icons::Icon;
use crate::ui::theme::{self, Theme};
use crate::ui::widgets::{self, feedback};
use crate::ui::UiState;

const REPOSITORY: &str = "https://github.com/rustblox/rustblox";

pub fn render(ui: &mut egui::Ui, state: &mut AppState, ui_state: &mut UiState) {
    let theme = Theme::get(ui.ctx());
    let mut open_url = None;
    let mut open_path = None;

    widgets::page_header(
        ui,
        "About",
        "What this build is and what it can do.",
        |_| {},
    );

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
                    open_url = Some(REPOSITORY);
                }
            });
        });
    });

    ui.add_space(theme.metrics.gap_lg);

    widgets::section(ui, "How launching works", None, |ui| {
        for (title, body) in [
                (
                    "RustBlox starts the client directly",
                    "It locates RobloxPlayerBeta.exe and runs it as a detached process, then watches for the client to appear before reporting success. It never fakes progress it has not measured.",
                ),
                (
                    "Opening a place uses the deep link",
                    "Quick launch entries pass a roblox:// link to the client, which the client resolves using the account you are already signed into.",
                ),
                (
                    "Launch links from the website are passed through",
                    "If RustBlox is registered as the roblox-player handler, links arriving from the browser are validated and handed to the client unchanged, sign-in ticket included.",
                ),
            ] {
                widgets::nested(ui, |ui| {
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
                ui.add_space(theme.metrics.gap_sm);
            }
    });

    ui.add_space(theme.metrics.gap_lg);

    widgets::section(ui, "Known limits", None, |ui| {
        widgets::banner(
            ui,
            feedback::Tone::Info,
            "RustBlox cannot sign you in",
            "Joining a specific server needs an authentication ticket that only Roblox itself can mint from your web session. RustBlox never asks for your password or cookie. That is why launching goes through the client or a deep link rather than a private join API.");
        ui.add_space(theme.metrics.gap_sm);
        widgets::banner(
            ui,
            feedback::Tone::Warning,
            "Flags are unsupported by Roblox",
            "The Flags page writes a file the client happens to read at startup. Roblox does not document it and can change or ignore it at any time.");
    });

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

    if let Some(url) = open_url {
        state.open_url(url);
    }
    if let Some(path) = open_path {
        state.open_path(path);
    }
}

use crate::app::AppState;
use crate::shortcuts::{Kind, Where};

use crate::ui::icons::Icon;
use crate::ui::theme::{self, Theme};
use crate::ui::widgets::{self, feedback};

pub fn render(ui: &mut egui::Ui, state: &mut AppState) {
    let theme = Theme::get(ui.ctx());
    state.refresh_shortcuts(false);

    let mut toggled = None;
    let mut open = None;

    widgets::page_header(
        ui,
        "Shortcuts",
        "Ways to start RustBlox without going looking for it.",
        |ui| {
            if widgets::Button::new("Reread")
                .icon(Icon::Refresh)
                .tone(widgets::Tone::Ghost)
                .show(ui)
                .clicked()
            {
                state.refresh_shortcuts(true);
            }
            widgets::badge(
                ui,
                &format!(
                    "{} of {} in place",
                    state.shortcuts.count(),
                    Kind::ALL.len()
                ),
                feedback::Tone::Neutral,
            );
        },
    );

    if state.exe_path.is_none() {
        widgets::banner(
            ui,
            feedback::Tone::Danger,
            "RustBlox cannot find itself",
            "Windows did not say where this executable is, so no shortcut can point at it.",
        );
        ui.add_space(theme.metrics.gap_lg);
    }

    group(
        ui,
        &theme,
        state,
        "Open RustBlox",
        "Both of these open the small menu with Launch, Configure and Uninstall on it.",
        &[Kind::Desktop, Kind::StartMenu],
        &mut toggled,
    );
    ui.add_space(theme.metrics.gap_lg);

    group(
        ui,
        &theme,
        state,
        "Go straight there",
        "These skip the menu and do the one thing, both on the desktop.",
        &[Kind::LaunchRoblox, Kind::Settings],
        &mut toggled,
    );
    ui.add_space(theme.metrics.gap_lg);

    widgets::section(
        ui,
        "Where they point",
        Some("A shortcut holds the path RustBlox had when it was made. Move the executable and it needs making again."),
        |ui| {
            widgets::detail_row(
                ui,
                "This build",
                &state
                    .exe_path
                    .as_ref()
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|| "unknown".into()),
                true,
            );
            ui.add_space(theme.metrics.gap_xs);
            for place in [Where::Desktop, Where::StartMenu] {
                let (label, folder) = match place {
                    Where::Desktop => ("Desktop", Kind::Desktop.folder()),
                    Where::StartMenu => ("Start menu", Kind::StartMenu.folder()),
                };
                widgets::detail_row(
                    ui,
                    label,
                    &folder
                        .as_ref()
                        .map(|path| path.display().to_string())
                        .unwrap_or_else(|| "not found on this PC".into()),
                    true,
                );
                ui.add_space(theme.metrics.gap_xs);
            }

            ui.add_space(theme.metrics.gap_sm);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = theme.metrics.gap_sm;
                for place in [Where::Desktop, Where::StartMenu] {
                    let (label, folder) = match place {
                        Where::Desktop => ("Open desktop folder", Kind::Desktop.folder()),
                        Where::StartMenu => ("Open start menu folder", Kind::StartMenu.folder()),
                    };
                    if widgets::Button::new(label)
                        .icon(Icon::Folder)
                        .size(widgets::Size::Small)
                        .enabled(folder.is_some())
                        .show(ui)
                        .clicked()
                    {
                        open = folder;
                    }
                }
            });
        },
    );

    if let Some(kind) = toggled {
        state.toggle_shortcut(kind);
    }
    if let Some(path) = open {
        state.open_path(path);
    }
}

fn group(
    ui: &mut egui::Ui,
    theme: &Theme,
    state: &AppState,
    title: &str,
    subtitle: &str,
    kinds: &[Kind],
    toggled: &mut Option<Kind>,
) {
    let usable = state.exe_path.is_some();

    widgets::section(ui, title, Some(subtitle), |ui| {
        for (index, kind) in kinds.iter().enumerate() {
            if index > 0 {
                ui.add_space(theme.metrics.gap_md);
            }

            let mut on = state.shortcuts.has(*kind);
            let missing = kind.folder().is_none();
            widgets::setting_row(ui, kind.label(), kind.detail(), |ui| {
                if widgets::toggle_enabled(ui, &mut on, usable && !missing).changed() {
                    *toggled = Some(*kind);
                }
            });

            if missing {
                ui.add_space(theme.metrics.gap_xs);
                ui.label(
                    egui::RichText::new("Windows did not give a path for that folder.")
                        .font(theme::text_style(theme::size::SMALL))
                        .color(theme.palette.text_faint),
                );
            }
        }
    });
}

pub mod chrome;

pub mod appicon;
mod install_overlay;
pub mod launcher;
pub mod overlay;
mod pages;
pub mod splash;
mod toasts;

pub mod icons;
pub mod theme;
pub mod widgets;

use egui::{Align, Layout};

use crate::app::AppState;
use crate::cli::CommandKind;
use crate::config::WindowState;
use crate::roblox::launch::LaunchTarget;

use theme::Theme;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Page {
    #[default]
    Home,
    Game,
    Mods,
    Shortcuts,
    Installation,
    FFlags,
    Settings,
    About,
}

impl Page {
    pub const ALL: [Page; 8] = [
        Page::Home,
        Page::Game,
        Page::Mods,
        Page::Shortcuts,
        Page::Installation,
        Page::FFlags,
        Page::Settings,
        Page::About,
    ];

    const SIMPLE: [Page; 4] = [Page::Home, Page::Game, Page::Settings, Page::About];

    pub fn visible(advanced: bool) -> &'static [Page] {
        if advanced {
            &Self::ALL
        } else {
            &Self::SIMPLE
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Page::Home => "Home",
            Page::Game => "Game",
            Page::Mods => "Mods",
            Page::Shortcuts => "Shortcuts",
            Page::Installation => "Installation",
            Page::FFlags => "FFlags",
            Page::Settings => "Settings",
            Page::About => "About",
        }
    }

    pub fn icon(self) -> icons::Icon {
        match self {
            Page::Home => icons::Icon::Home,
            Page::Game => icons::Icon::Gauge,
            Page::Mods => icons::Icon::Layers,
            Page::Shortcuts => icons::Icon::External,
            Page::Installation => icons::Icon::Package,
            Page::FFlags => icons::Icon::Flag,
            Page::Settings => icons::Icon::Sliders,
            Page::About => icons::Icon::Info,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SettingsTab {
    #[default]
    General,
    Launch,
    Appearance,
    Advanced,
}

impl SettingsTab {
    pub const ALL: [SettingsTab; 4] = [
        SettingsTab::General,
        SettingsTab::Launch,
        SettingsTab::Appearance,
        SettingsTab::Advanced,
    ];

    const SIMPLE: [SettingsTab; 3] = [
        SettingsTab::General,
        SettingsTab::Launch,
        SettingsTab::Appearance,
    ];

    pub fn visible(advanced: bool) -> &'static [SettingsTab] {
        if advanced {
            &Self::ALL
        } else {
            &Self::SIMPLE
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SettingsTab::General => "General",
            SettingsTab::Launch => "Launch",
            SettingsTab::Appearance => "Appearance",
            SettingsTab::Advanced => "Advanced",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Shell {
    #[default]
    Launcher,
    Splash,
    Uninstall,
    Full,
}

impl Shell {
    pub fn size(self, window: WindowState) -> egui::Vec2 {
        match self {
            Shell::Launcher => egui::vec2(launcher::WIDTH, launcher::HEIGHT),
            Shell::Splash => egui::vec2(splash::WIDTH, splash::HEIGHT),
            Shell::Uninstall => egui::vec2(launcher::UNINSTALL_WIDTH, launcher::UNINSTALL_HEIGHT),
            Shell::Full => egui::vec2(window.width, window.height),
        }
    }

    pub fn is_small(self) -> bool {
        !matches!(self, Shell::Full)
    }
}

fn centre_on_screen(ctx: &egui::Context, size: egui::Vec2) {
    let Some(monitor) = ctx.input(|input| input.viewport().monitor_size) else {
        return;
    };
    if monitor.x <= 1.0 || monitor.y <= 1.0 {
        return;
    }

    let position = egui::pos2(
        ((monitor.x - size.x) / 2.0).max(0.0),
        ((monitor.y - size.y) / 2.0).max(0.0),
    );
    ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(position));
}

pub fn initial_shell(command: &CommandKind) -> Shell {
    match command {
        CommandKind::WindowOnSettings => Shell::Full,
        CommandKind::LaunchNow | CommandKind::Forward(_) => Shell::Splash,
        _ => Shell::Launcher,
    }
}

#[derive(Default)]
pub struct UiState {
    pub shell: Shell,
    pub return_shell: Shell,
    pub sidebar_collapsed: bool,
    pub uninstall_settings: bool,
    pub page: Page,
    pub settings_tab: SettingsTab,
    pub quick_input: String,
    pub quick_name: String,
    pub quick_error: Option<String>,
    pub flag_key: String,
    pub flag_value: String,
    pub flag_error: Option<String>,
    pub flag_filter: String,
    pub confirm_flag_reset: bool,
    pub raw_editor: Option<String>,
    pub confirm: Option<LaunchTarget>,
    pub extra_args_buffer: Option<String>,
    pub channel_buffer: Option<String>,
    pub scale_buffer: Option<String>,
    pub show_log: bool,
}

pub struct RustBloxApp {
    pub state: AppState,
    pub ui: UiState,
    fonts_ready: bool,
    applied_scale: f32,
    applied_theme: Option<Theme>,
    applied_shell: Option<Shell>,
    shown_tab: Option<(Page, SettingsTab)>,
    saved_window: WindowState,
}

impl RustBloxApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        mut state: AppState,
        command: &CommandKind,
    ) -> Self {
        theme::install_fonts(&cc.egui_ctx);
        state.attach(cc.egui_ctx.clone());

        let mut ui = UiState {
            shell: initial_shell(command),
            ..UiState::default()
        };
        match command {
            CommandKind::WindowOnSettings => ui.page = Page::Settings,
            CommandKind::Forward(uri) => {
                state.start_launch_flow(LaunchTarget::Forward(uri.clone()));
            }
            CommandKind::LaunchNow => {
                let target = state.default_target();
                state.start_launch_flow(target);
            }
            _ => {}
        }

        let saved_window = state.persisted.window.sanitised();
        let shell = ui.shell;

        Self {
            state,
            ui,
            fonts_ready: true,
            applied_scale: 0.0,
            applied_theme: None,
            applied_shell: Some(shell),
            shown_tab: None,
            saved_window,
        }
    }

    fn sync_theme(&mut self, ctx: &egui::Context) {
        let theme = Theme::from_settings(&self.state.settings.appearance, self.state.system_dark);
        let scale = self.state.settings.appearance.ui_scale;

        if self.applied_theme != Some(theme) || (self.applied_scale - scale).abs() > f32::EPSILON {
            theme.store(ctx);
            theme::apply_style(ctx, &theme, scale);
            self.applied_theme = Some(theme);
            self.applied_scale = scale;
        } else {
            theme.store(ctx);
        }
    }

    fn sync_shell(&mut self, ctx: &egui::Context) {
        if self.applied_shell == Some(self.ui.shell) {
            return;
        }
        self.applied_shell = Some(self.ui.shell);

        let shell = self.ui.shell;
        let window = self.saved_window.sanitised();
        let size = shell.size(window);
        let minimum = if shell.is_small() {
            size
        } else {
            egui::vec2(WindowState::MIN_WIDTH, WindowState::MIN_HEIGHT)
        };

        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(minimum));
        ctx.send_viewport_cmd(egui::ViewportCommand::Resizable(!shell.is_small()));
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
        centre_on_screen(ctx, size);

        if !shell.is_small() && window.maximized {
            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(true));
        }
    }

    fn small_window(
        &mut self,
        root: &mut egui::Ui,
        theme: &Theme,
        title: &str,
        body: impl FnOnce(&mut egui::Ui, &Theme, &mut AppState, &mut UiState),
    ) {
        let ctx = &root.ctx().clone();
        egui::Panel::top("small-titlebar")
            .exact_size(theme.metrics.titlebar_h)
            .resizable(false)
            .show_separator_line(false)
            .frame(egui::Frame::new().fill(theme.palette.sidebar))
            .show(root, |ui| {
                launcher::title_bar(ui, theme, title, &mut self.state, &mut self.ui);
            });

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(theme.palette.window)
                    .inner_margin(egui::Margin::same(14)),
            )
            .show(root, |ui| {
                body(ui, theme, &mut self.state, &mut self.ui);
            });

        if self.ui.shell != Shell::Splash {
            overlay::confirm_dialog(ctx, theme, &mut self.state, &mut self.ui);
            toasts::render(ctx, theme, &mut self.state);
        }
        self.pump_viewport(ctx);
    }

    fn pump_viewport(&mut self, ctx: &egui::Context) {
        if self.state.close_requested {
            self.state.close_requested = false;
            let _ = self.state.shutdown();
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if ctx.input(|input| input.viewport().close_requested()) {
            let _ = self.state.shutdown();
        }
    }

    fn remember_window(&mut self, ctx: &egui::Context) {
        if self.ui.shell != Shell::Full {
            return;
        }
        let maximized = ctx.input(|input| input.viewport().maximized.unwrap_or(false));
        let size = ctx.input(|input| input.viewport().inner_rect.map(|rect| rect.size()));

        let mut next = self.saved_window;
        next.maximized = maximized;
        if !maximized {
            if let Some(size) = size {
                if size.x >= WindowState::MIN_WIDTH && size.y >= WindowState::MIN_HEIGHT {
                    next.width = size.x;
                    next.height = size.y;
                }
            }
        }

        if next != self.saved_window {
            self.saved_window = next;
            self.state.persisted.window = next;
            self.state.mark_state_dirty();
        }
    }
}

impl eframe::App for RustBloxApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        let theme = self.applied_theme.unwrap_or_else(|| {
            Theme::from_settings(&self.state.settings.appearance, self.state.system_dark)
        });
        let color = theme.palette.window;
        [
            color.r() as f32 / 255.0,
            color.g() as f32 / 255.0,
            color.b() as f32 / 255.0,
            1.0,
        ]
    }

    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        debug_assert!(self.fonts_ready);

        let ctx = root.ctx().clone();
        self.sync_theme(&ctx);
        self.state.tick();
        self.remember_window(&ctx);

        let theme = Theme::get(&ctx);

        self.sync_shell(&ctx);

        match self.ui.shell {
            Shell::Launcher => {
                self.small_window(root, &theme, "RustBlox", launcher::render);
                return;
            }
            Shell::Splash => {
                self.small_window(root, &theme, "Starting Roblox", splash::render);
                return;
            }
            Shell::Uninstall => {
                self.small_window(root, &theme, "Uninstall", launcher::uninstall_panel);
                return;
            }
            Shell::Full => {}
        }

        chrome::resize_edges(&ctx, &theme);

        egui::Panel::top("titlebar")
            .exact_size(theme.metrics.titlebar_h)
            .resizable(false)
            .show_separator_line(false)
            .frame(egui::Frame::new().fill(theme.palette.sidebar))
            .show(root, |ui| {
                chrome::title_bar(ui, &theme, &mut self.state, &mut self.ui);
            });

        let sidebar_w = chrome::sidebar_width(&ctx, &theme, self.ui.sidebar_collapsed);
        let expansion = chrome::sidebar_expansion(&theme, sidebar_w);

        egui::Panel::left("sidebar")
            .exact_size(sidebar_w)
            .resizable(false)
            .drag_to_open(false)
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(theme.palette.sidebar)
                    .inner_margin(egui::Margin {
                        left: 6,
                        right: 6,
                        top: 8,
                        bottom: 10,
                    }),
            )
            .show(root, |ui| {
                chrome::sidebar(ui, &theme, &mut self.state, &mut self.ui, expansion);
            });

        let tab = (self.ui.page, self.ui.settings_tab);
        let switched = self.shown_tab != Some(tab);
        self.shown_tab = Some(tab);

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(theme.palette.window))
            .show(root, |ui| {
                let pad = theme.metrics.page_pad;
                let mut area = egui::ScrollArea::vertical().auto_shrink([false, false]);
                if switched {
                    area = area.vertical_scroll_offset(0.0);
                }
                area.show(ui, |ui| {
                    ui.allocate_ui_with_layout(
                        egui::vec2(ui.available_width(), 0.0),
                        Layout::top_down(Align::Min),
                        |ui| {
                            ui.add_space(pad * 0.9);
                            let content = (ui.available_width() - pad * 2.0).max(1.0);
                            ui.horizontal_top(|ui| {
                                ui.add_space(pad);
                                ui.allocate_ui_with_layout(
                                    egui::vec2(content, 0.0),
                                    Layout::top_down(Align::Min),
                                    |ui| {
                                        ui.set_width(content);
                                        pages::render(ui, &mut self.state, &mut self.ui);
                                    },
                                );
                            });
                            ui.add_space(pad);
                        },
                    );
                });
            });

        let ctx = &ctx;

        overlay::launch_overlay(ctx, &theme, &mut self.state, &mut self.ui);
        install_overlay::render(ctx, &theme, &mut self.state);
        overlay::confirm_dialog(ctx, &theme, &mut self.state, &mut self.ui);
        toasts::render(ctx, &theme, &mut self.state);

        self.pump_viewport(ctx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_launcher_is_what_a_bare_start_opens() {
        assert_eq!(initial_shell(&CommandKind::Window), Shell::Launcher);
        assert_eq!(
            initial_shell(&CommandKind::Print(String::new())),
            Shell::Launcher
        );
    }

    #[test]
    fn settings_opens_the_full_window_directly() {
        assert_eq!(initial_shell(&CommandKind::WindowOnSettings), Shell::Full);
    }

    #[test]
    fn launching_opens_the_splash_rather_than_the_menu() {
        assert_eq!(initial_shell(&CommandKind::LaunchNow), Shell::Splash);
        assert_eq!(
            initial_shell(&CommandKind::Forward("roblox-player:1".into())),
            Shell::Splash
        );
    }

    #[test]
    fn only_the_full_window_is_resizable() {
        assert!(Shell::Launcher.is_small());
        assert!(Shell::Splash.is_small());
        assert!(Shell::Uninstall.is_small());
        assert!(!Shell::Full.is_small());
    }

    #[test]
    fn the_splash_is_smaller_than_the_menu() {
        let window = WindowState::default();
        assert!(Shell::Splash.size(window).x < Shell::Launcher.size(window).x);
        assert!(Shell::Splash.size(window).y < Shell::Launcher.size(window).y);
    }

    #[test]
    fn the_full_window_uses_the_remembered_size() {
        let window = WindowState {
            width: 1234.0,
            height: 800.0,
            maximized: false,
        };
        assert_eq!(Shell::Full.size(window), egui::vec2(1234.0, 800.0));
    }

    #[test]
    fn the_uninstall_window_has_room_for_its_buttons() {
        let window = WindowState::default();
        assert!(Shell::Uninstall.size(window).y > Shell::Launcher.size(window).y);
    }

    #[test]
    fn flags_installs_and_the_advanced_tab_are_hidden_until_asked_for() {
        for page in [Page::FFlags, Page::Installation] {
            assert!(!Page::visible(false).contains(&page));
            assert!(Page::visible(true).contains(&page));
        }
        assert!(!SettingsTab::visible(false).contains(&SettingsTab::Advanced));
        assert!(SettingsTab::visible(true).contains(&SettingsTab::Advanced));
    }

    #[test]
    fn the_simple_view_still_reaches_every_everyday_page() {
        for page in [Page::Home, Page::Settings, Page::About] {
            assert!(Page::visible(false).contains(&page));
        }
    }
}

pub mod chrome;

pub mod appicon;
mod install_overlay;
pub mod launcher;
pub mod overlay;
mod pages;
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
    Installation,
    Flags,
    Settings,
    About,
}

impl Page {
    pub const ALL: [Page; 5] = [
        Page::Home,
        Page::Installation,
        Page::Flags,
        Page::Settings,
        Page::About,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Page::Home => "Home",
            Page::Installation => "Installation",
            Page::Flags => "Flags",
            Page::Settings => "Settings",
            Page::About => "About",
        }
    }

    pub fn icon(self) -> icons::Icon {
        match self {
            Page::Home => icons::Icon::Home,
            Page::Installation => icons::Icon::Package,
            Page::Flags => icons::Icon::Flag,
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

    pub fn label(self) -> &'static str {
        match self {
            SettingsTab::General => "General",
            SettingsTab::Launch => "Launch",
            SettingsTab::Appearance => "Appearance",
            SettingsTab::Advanced => "Advanced",
        }
    }
}

pub fn starts_in_launcher(command: &CommandKind) -> bool {
    !matches!(command, CommandKind::WindowOnSettings)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Shell {
    #[default]
    Launcher,
    Full,
}

#[derive(Default)]
pub struct UiState {
    pub shell: Shell,
    pub sidebar_collapsed: bool,
    pub uninstall: Option<bool>,
    pub page: Page,
    pub settings_tab: SettingsTab,
    pub quick_input: String,
    pub quick_name: String,
    pub quick_error: Option<String>,
    pub flag_key: String,
    pub flag_value: String,
    pub flag_error: Option<String>,
    pub flag_filter: String,
    pub raw_editor: Option<String>,
    pub raw_error: Option<String>,
    pub confirm: Option<LaunchTarget>,
    pub extra_args_buffer: Option<String>,
    pub channel_buffer: Option<String>,
    pub show_searched_paths: bool,
    pub show_log: bool,
    pub pending_removal: Option<String>,
}

pub struct RustBloxApp {
    pub state: AppState,
    pub ui: UiState,
    fonts_ready: bool,
    applied_scale: f32,
    applied_theme: Option<Theme>,
    applied_shell: Option<Shell>,
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

        let mut ui = UiState::default();
        match command {
            CommandKind::WindowOnSettings => {
                ui.shell = Shell::Full;
                ui.page = Page::Settings;
            }
            CommandKind::Forward(uri) => {
                state.forwarding = true;
                state.launch(LaunchTarget::Forward(uri.clone()));
            }
            CommandKind::LaunchNow => {
                let target = state.default_target();
                state.launch(target);
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

        match self.ui.shell {
            Shell::Launcher => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Resizable(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(egui::vec2(
                    launcher::WIDTH,
                    launcher::HEIGHT,
                )));
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                    launcher::WIDTH,
                    launcher::HEIGHT,
                )));
            }
            Shell::Full => {
                let window = self.saved_window.sanitised();
                ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(egui::vec2(
                    WindowState::MIN_WIDTH,
                    WindowState::MIN_HEIGHT,
                )));
                ctx.send_viewport_cmd(egui::ViewportCommand::Resizable(true));
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                    window.width,
                    window.height,
                )));
            }
        }
    }

    fn launcher(&mut self, root: &mut egui::Ui, theme: &Theme) {
        let ctx = &root.ctx().clone();
        egui::Panel::top("launcher-titlebar")
            .exact_size(theme.metrics.titlebar_h)
            .resizable(false)
            .show_separator_line(false)
            .frame(egui::Frame::new().fill(theme.palette.sidebar))
            .show(root, |ui| {
                launcher::title_bar(ui, theme, &mut self.state);
            });

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(theme.palette.window)
                    .inner_margin(egui::Margin::same(14)),
            )
            .show(root, |ui| {
                launcher::render(ui, theme, &mut self.state, &mut self.ui);
            });

        launcher::uninstall_dialog(ctx, theme, &mut self.state, &mut self.ui);
        overlay::launch_overlay(ctx, theme, &mut self.state, &mut self.ui);
        overlay::confirm_dialog(ctx, theme, &mut self.state, &mut self.ui);
        toasts::render(ctx, theme, &mut self.state);

        if self.state.minimize_requested {
            self.state.minimize_requested = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
        }
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

        if self.ui.shell == Shell::Launcher {
            self.launcher(root, &theme);
            return;
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

        let sidebar_w = if self.ui.sidebar_collapsed {
            56.0
        } else {
            theme.metrics.sidebar_w
        };

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
                chrome::sidebar(ui, &theme, &mut self.state, &mut self.ui);
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(theme.palette.window))
            .show(root, |ui| {
                let pad = theme.metrics.page_pad;
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.allocate_ui_with_layout(
                            egui::vec2(ui.available_width(), 0.0),
                            Layout::top_down(Align::Min),
                            |ui| {
                                ui.add_space(pad * 0.9);
                                let content = (ui.available_width() - pad * 2.0).max(320.0);
                                ui.horizontal(|ui| {
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

        if self.state.minimize_requested {
            self.state.minimize_requested = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
        }

        if self.state.close_requested {
            self.state.close_requested = false;
            let _ = self.state.shutdown();
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }

        if ctx.input(|input| input.viewport().close_requested()) {
            let _ = self.state.shutdown();
        }
    }
}

use egui::{
    Align, Align2, Layout, Rect, ResizeDirection, Sense, Stroke, Ui, Vec2, ViewportCommand,
};

use crate::app::AppState;
use crate::roblox::launch::LaunchTarget;

use super::icons::{self, Icon};
use super::theme::{self, Theme};
use super::widgets::{self, feedback};
use super::{Page, UiState};

const EDGE: f32 = 5.0;

pub fn resize_edges(ctx: &egui::Context, _theme: &Theme) {
    let maximized = ctx.input(|input| input.viewport().maximized.unwrap_or(false));
    if maximized {
        return;
    }

    let screen = ctx.viewport_rect();
    let corner = EDGE * 2.6;

    let vertical = egui::CursorIcon::ResizeVertical;
    let horizontal = egui::CursorIcon::ResizeHorizontal;
    let down_right = egui::CursorIcon::ResizeNwSe;
    let down_left = egui::CursorIcon::ResizeNeSw;

    let zones = [
        (
            Rect::from_min_max(
                screen.left_top(),
                egui::pos2(screen.right(), screen.top() + EDGE),
            ),
            ResizeDirection::North,
            vertical,
        ),
        (
            Rect::from_min_max(
                egui::pos2(screen.left(), screen.bottom() - EDGE),
                screen.right_bottom(),
            ),
            ResizeDirection::South,
            vertical,
        ),
        (
            Rect::from_min_max(
                screen.left_top(),
                egui::pos2(screen.left() + EDGE, screen.bottom()),
            ),
            ResizeDirection::West,
            horizontal,
        ),
        (
            Rect::from_min_max(
                egui::pos2(screen.right() - EDGE, screen.top()),
                screen.right_bottom(),
            ),
            ResizeDirection::East,
            horizontal,
        ),
        (
            Rect::from_min_size(screen.left_top(), Vec2::splat(corner)),
            ResizeDirection::NorthWest,
            down_right,
        ),
        (
            Rect::from_min_size(
                screen.right_top() - Vec2::new(corner, 0.0),
                Vec2::splat(corner),
            ),
            ResizeDirection::NorthEast,
            down_left,
        ),
        (
            Rect::from_min_size(
                screen.left_bottom() - Vec2::new(0.0, corner),
                Vec2::splat(corner),
            ),
            ResizeDirection::SouthWest,
            down_left,
        ),
        (
            Rect::from_min_size(
                screen.right_bottom() - Vec2::splat(corner),
                Vec2::splat(corner),
            ),
            ResizeDirection::SouthEast,
            down_right,
        ),
    ];

    for (index, (rect, direction, cursor)) in zones.into_iter().enumerate() {
        egui::Area::new(egui::Id::new(("resize-zone", index)))
            .order(egui::Order::Foreground)
            .fixed_pos(rect.min)
            .interactable(true)
            .show(ctx, |ui| {
                let (_, response) = ui.allocate_exact_size(rect.size(), Sense::click_and_drag());

                if response.hovered() || response.is_pointer_button_down_on() {
                    ui.ctx().set_cursor_icon(cursor);
                }
                if response.drag_started() {
                    ui.ctx()
                        .send_viewport_cmd(ViewportCommand::BeginResize(direction));
                }
            });
    }
}

pub fn title_bar(ui: &mut Ui, theme: &Theme, state: &mut AppState, ui_state: &mut UiState) {
    let palette = theme.palette;
    let rect = ui.max_rect();

    ui.painter().hline(
        rect.x_range(),
        rect.bottom() - 0.5,
        Stroke::new(1.0, palette.border),
    );

    let button_width = 46.0;
    let controls_width = button_width * 3.0;
    let drag_rect = Rect::from_min_max(
        rect.left_top(),
        egui::pos2(rect.right() - controls_width, rect.bottom()),
    );

    let drag = ui.interact(
        drag_rect,
        egui::Id::new("titlebar-drag"),
        Sense::click_and_drag(),
    );
    if drag.drag_started_by(egui::PointerButton::Primary) {
        ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
    }
    if drag.double_clicked() {
        let maximized = ui.input(|input| input.viewport().maximized.unwrap_or(false));
        ui.ctx()
            .send_viewport_cmd(ViewportCommand::Maximized(!maximized));
    }

    ui.painter().text(
        egui::pos2(rect.left() + 16.0, rect.center().y),
        Align2::LEFT_CENTER,
        "RustBlox",
        theme::medium(theme::size::SMALL),
        palette.text,
    );

    let page_label = ui_state.page.label();
    ui.painter().text(
        egui::pos2(rect.left() + 16.0 + 74.0, rect.center().y),
        Align2::LEFT_CENTER,
        format!("/  {page_label}"),
        theme::text_style(theme::size::SMALL),
        palette.text_faint,
    );

    if state.roblox.player_running() {
        let text = state.roblox.summary();
        let font = theme::medium(theme::size::MICRO);
        let galley =
            ui.painter()
                .layout_no_wrap(text.clone(), font.clone(), egui::Color32::PLACEHOLDER);
        let pill = Rect::from_center_size(
            egui::pos2(rect.center().x, rect.center().y),
            Vec2::new(galley.size().x + 30.0, 22.0),
        );
        ui.painter().rect_filled(
            pill,
            egui::CornerRadius::same(11),
            palette.success.gamma_multiply(0.14),
        );
        ui.painter().circle_filled(
            egui::pos2(pill.left() + 12.0, pill.center().y),
            3.5,
            palette.success,
        );
        ui.painter().text(
            egui::pos2(pill.left() + 21.0, pill.center().y),
            Align2::LEFT_CENTER,
            text,
            font,
            palette.success,
        );
    }

    ui.scope_builder(
        egui::UiBuilder::new().max_rect(Rect::from_min_max(
            egui::pos2(rect.right() - controls_width, rect.top()),
            rect.right_bottom(),
        )),
        |ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 0.0;

                if window_button(ui, theme, Icon::Close, true).clicked() {
                    state.close_requested = true;
                }

                let maximized = ui.input(|input| input.viewport().maximized.unwrap_or(false));
                let icon = if maximized {
                    Icon::Restore
                } else {
                    Icon::Maximize
                };
                if window_button(ui, theme, icon, false).clicked() {
                    ui.ctx()
                        .send_viewport_cmd(ViewportCommand::Maximized(!maximized));
                }

                if window_button(ui, theme, Icon::Minimize, false).clicked() {
                    ui.ctx().send_viewport_cmd(ViewportCommand::Minimized(true));
                }
            });
        },
    );
}

fn window_button(ui: &mut Ui, theme: &Theme, icon: Icon, danger: bool) -> egui::Response {
    let size = Vec2::new(46.0, theme.metrics.titlebar_h);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());

    let hover =
        ui.ctx()
            .animate_bool_with_time(response.id.with("hover"), response.hovered(), 0.09);

    if hover > 0.0 {
        let fill = if danger {
            theme.palette.danger
        } else {
            theme.palette.surface_hover
        };
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::ZERO, fill.gamma_multiply(hover));
    }

    let tint = if danger && hover > 0.5 {
        egui::Color32::WHITE
    } else {
        theme.palette.text_muted
    };

    icons::draw(
        ui.painter(),
        icon,
        rect.shrink2(Vec2::new(16.0, 13.0)),
        tint,
        1.5,
    );
    response
}

pub fn sidebar(ui: &mut Ui, theme: &Theme, state: &mut AppState, ui_state: &mut UiState) {
    widgets::brand(ui, "RustBlox", "Roblox client manager");
    ui.add_space(theme.metrics.gap_md);

    widgets::section_label(ui, "Client");
    for page in [Page::Home, Page::Installation, Page::Flags] {
        let badge = match page {
            Page::Flags if state.flags.active_count() > 0 => {
                Some(state.flags.active_count().to_string())
            }
            _ => None,
        };
        if widgets::nav_item(
            ui,
            page.icon(),
            page.label(),
            ui_state.page == page,
            badge.as_deref(),
        )
        .clicked()
        {
            ui_state.page = page;
        }
        ui.add_space(2.0);
    }

    ui.add_space(theme.metrics.gap_sm);
    widgets::section_label(ui, "Application");
    for page in [Page::Settings, Page::About] {
        if widgets::nav_item(ui, page.icon(), page.label(), ui_state.page == page, None).clicked() {
            ui_state.page = page;
        }
        ui.add_space(2.0);
    }

    let ready = state.detection.active().is_some();
    let running = state.roblox.player_running();
    let version = state
        .detection
        .active()
        .map(|install| install.display_version().to_owned());
    let (tone, label) = if !ready {
        (feedback::Tone::Warning, "Roblox not found".to_string())
    } else if running {
        (feedback::Tone::Success, state.roblox.summary())
    } else {
        (feedback::Tone::Neutral, "Ready to launch".to_string())
    };
    let can_launch = ready && state.can_launch();
    let mut launch_clicked = false;

    ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
        widgets::card(ui, |ui| {
            ui.with_layout(Layout::top_down(Align::Min), |ui| {
                ui.spacing_mut().item_spacing.y = theme.metrics.gap_sm;
                widgets::status_pill(ui, &label, tone, running);

                if let Some(version) = &version {
                    ui.label(
                        egui::RichText::new(format!("Version {version}"))
                            .font(theme::text_style(theme::size::MICRO))
                            .color(theme.palette.text_faint),
                    );
                }

                launch_clicked = widgets::Button::primary("Launch Roblox")
                    .icon(Icon::Rocket)
                    .fill_width(true)
                    .enabled(can_launch)
                    .show(ui)
                    .clicked();
            });
        });
    });

    if launch_clicked {
        let target = state.default_target();
        request_launch(state, ui_state, target);
    }
}

pub fn request_launch(state: &mut AppState, ui_state: &mut UiState, target: LaunchTarget) {
    if state.settings.launch.confirm_before_launch {
        ui_state.confirm = Some(target);
    } else {
        state.launch(target);
    }
}

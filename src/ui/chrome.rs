use egui::{
    Align, Align2, Layout, Rect, ResizeDirection, Sense, Stroke, Ui, Vec2, ViewportCommand,
};

use crate::app::AppState;
use crate::roblox::launch::LaunchTarget;

use super::appicon;
use super::icons::{self, Icon};
use super::theme::{self, Theme};
use super::widgets;
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

    let mark = Rect::from_min_size(
        egui::pos2(rect.left() + 12.0, rect.center().y - 8.0),
        Vec2::splat(16.0),
    );
    appicon::paint(ui, mark, theme.radius_sm());

    ui.painter().text(
        egui::pos2(mark.right() + 10.0, rect.center().y - 1.0),
        Align2::LEFT_CENTER,
        ui_state.page.label(),
        theme::medium(theme::size::SMALL),
        palette.text_muted,
    );

    if state.roblox.player_running() {
        let text = state.roblox.summary();
        let font = theme::medium(theme::size::MICRO);
        let galley =
            ui.painter()
                .layout_no_wrap(text.clone(), font.clone(), egui::Color32::PLACEHOLDER);
        let pill = Rect::from_center_size(
            egui::pos2(rect.center().x, rect.center().y),
            Vec2::new(galley.size().x + 26.0, 20.0),
        );
        ui.painter().circle_filled(
            egui::pos2(pill.left() + 8.0, pill.center().y),
            3.0,
            palette.success,
        );
        ui.painter().text(
            egui::pos2(pill.left() + 17.0, pill.center().y),
            Align2::LEFT_CENTER,
            text,
            font,
            palette.text_muted,
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

pub fn window_button(ui: &mut Ui, theme: &Theme, icon: Icon, danger: bool) -> egui::Response {
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
    let collapsed = ui_state.sidebar_collapsed;

    if collapse_button(ui, theme, collapsed).clicked() {
        ui_state.sidebar_collapsed = !collapsed;
    }
    ui.add_space(theme.metrics.gap_sm);

    for page in Page::ALL {
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
            collapsed,
        )
        .clicked()
        {
            ui_state.page = page;
        }
        ui.add_space(2.0);
    }
}

fn collapse_button(ui: &mut Ui, theme: &Theme, collapsed: bool) -> egui::Response {
    let height = theme.metrics.row_h;
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::click());

    let hover = ui
        .ctx()
        .animate_bool_with_time(response.id.with("hover"), response.hovered(), 0.1);
    if hover > 0.0 {
        ui.painter().rect_filled(
            rect,
            theme.radius_sm(),
            theme.palette.surface_hover.gamma_multiply(hover * 0.7),
        );
    }

    let x = if collapsed {
        rect.center().x
    } else {
        rect.left() + 22.0
    };
    let tint = theme.palette.text_muted;
    for offset in [-5.0, 0.0, 5.0] {
        ui.painter().hline(
            (x - 7.0)..=(x + 7.0),
            rect.center().y + offset,
            Stroke::new(1.6, tint),
        );
    }

    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    response.on_hover_text(if collapsed {
        "Show labels"
    } else {
        "Hide labels"
    })
}

pub fn request_launch(state: &mut AppState, ui_state: &mut UiState, target: LaunchTarget) {
    if state.settings.launch.confirm_before_launch {
        ui_state.confirm = Some(target);
    } else {
        state.launch(target);
    }
}

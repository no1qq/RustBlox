use egui::{Align, Frame, Layout, Response, Stroke, Ui, Vec2};

use crate::ui::icons::{self, Icon};
use crate::ui::theme::{self, Theme};

pub fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let theme = Theme::get(ui.ctx());
    Frame::new()
        .fill(theme.palette.surface)
        .stroke(theme.hairline())
        .corner_radius(theme.radius_lg())
        .inner_margin(theme.card_margin())
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

pub fn nested<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let theme = Theme::get(ui.ctx());
    Frame::new()
        .fill(theme.palette.surface_alt)
        .corner_radius(theme.radius_md())
        .inner_margin(egui::Margin::symmetric(
            theme.metrics.gap_md as i8,
            theme.metrics.gap_sm as i8,
        ))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

pub fn section<R>(
    ui: &mut Ui,
    title: &str,
    subtitle: Option<&str>,
    add: impl FnOnce(&mut Ui) -> R,
) -> R {
    let theme = Theme::get(ui.ctx());
    card(ui, |ui| {
        heading(ui, &theme, title, subtitle);
        ui.add_space(theme.metrics.gap_md);
        add(ui)
    })
}

pub fn heading(ui: &mut Ui, theme: &Theme, title: &str, subtitle: Option<&str>) {
    ui.label(
        egui::RichText::new(title)
            .font(theme::strong(theme::size::SECTION))
            .color(theme.palette.text),
    );
    if let Some(subtitle) = subtitle {
        ui.add_space(3.0);
        ui.label(
            egui::RichText::new(subtitle)
                .font(theme::text_style(theme::size::SMALL))
                .color(theme.palette.text_muted),
        );
    }
}

pub fn separator(ui: &mut Ui) {
    let theme = Theme::get(ui.ctx());
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 1.0), egui::Sense::hover());
    ui.painter().hline(
        rect.x_range(),
        rect.center().y,
        Stroke::new(1.0, theme.palette.border),
    );
}

pub fn page_header(
    ui: &mut Ui,
    title: &str,
    subtitle: &str,
    actions: impl FnOnce(&mut Ui),
) -> Response {
    let theme = Theme::get(ui.ctx());
    let response = ui
        .horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new(title)
                        .font(theme::strong(theme::size::DISPLAY))
                        .color(theme.palette.text),
                );
                if !subtitle.is_empty() {
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(subtitle)
                            .font(theme::text_style(theme::size::BODY))
                            .color(theme.palette.text_muted),
                    );
                }
            });
            ui.with_layout(Layout::right_to_left(Align::Center), actions);
        })
        .response;

    ui.add_space(theme.metrics.gap_lg);
    response
}

pub fn setting_row(ui: &mut Ui, title: &str, description: &str, control: impl FnOnce(&mut Ui)) {
    let theme = Theme::get(ui.ctx());
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = theme.metrics.gap_md;
        let total = ui.available_width();
        let control_width = (total * 0.42).clamp(120.0, 260.0);
        let label_width = (total - control_width - theme.metrics.gap_md).max(120.0);

        ui.allocate_ui_with_layout(
            Vec2::new(label_width, 0.0),
            Layout::top_down(Align::Min),
            |ui| {
                ui.set_width(label_width);
                ui.label(
                    egui::RichText::new(title)
                        .font(theme::medium(theme::size::BODY))
                        .color(theme.palette.text),
                );
                if !description.is_empty() {
                    ui.add_space(2.0);
                    ui.label(
                        egui::RichText::new(description)
                            .font(theme::text_style(theme::size::SMALL))
                            .color(theme.palette.text_muted),
                    );
                }
            },
        );

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.set_min_height(theme.metrics.control_h);
            control(ui);
        });
    });
}

pub fn detail_row(ui: &mut Ui, label: &str, value: &str, monospace: bool) {
    let theme = Theme::get(ui.ctx());
    ui.horizontal(|ui| {
        let total = ui.available_width();
        let label_width = (total * 0.32).clamp(96.0, 200.0);

        ui.allocate_ui_with_layout(
            Vec2::new(label_width, 0.0),
            Layout::top_down(Align::Min),
            |ui| {
                ui.set_width(label_width);
                ui.label(
                    egui::RichText::new(label)
                        .font(theme::text_style(theme::size::SMALL))
                        .color(theme.palette.text_muted),
                );
            },
        );

        let font = if monospace {
            egui::FontId::new(theme::size::SMALL, egui::FontFamily::Monospace)
        } else {
            theme::medium(theme::size::SMALL)
        };
        ui.label(
            egui::RichText::new(value)
                .font(font)
                .color(theme.palette.text),
        );
    });
}

pub fn empty_state(ui: &mut Ui, icon: Icon, title: &str, body: &str, action: impl FnOnce(&mut Ui)) {
    let theme = Theme::get(ui.ctx());
    ui.vertical_centered(|ui| {
        ui.add_space(theme.metrics.gap_xl);

        let (rect, _) = ui.allocate_exact_size(Vec2::splat(52.0), egui::Sense::hover());
        ui.painter()
            .rect_filled(rect, theme.radius_lg(), theme.palette.surface_alt);
        icons::draw(
            ui.painter(),
            icon,
            rect.shrink(15.0),
            theme.palette.text_faint,
            1.8,
        );

        ui.add_space(theme.metrics.gap_md);
        ui.label(
            egui::RichText::new(title)
                .font(theme::strong(theme::size::SECTION))
                .color(theme.palette.text),
        );
        ui.add_space(4.0);

        let width = ui.available_width().min(380.0);
        ui.allocate_ui(Vec2::new(width, 0.0), |ui| {
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new(body)
                        .font(theme::text_style(theme::size::SMALL))
                        .color(theme.palette.text_muted),
                );
            });
        });

        ui.add_space(theme.metrics.gap_md);
        action(ui);
        ui.add_space(theme.metrics.gap_xl);
    });
}

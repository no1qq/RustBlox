use egui::{Align2, Color32, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2};

use crate::ui::theme::{self, Theme};

use super::button::blend;

pub fn toggle(ui: &mut Ui, value: &mut bool) -> Response {
    toggle_enabled(ui, value, true)
}

pub fn toggle_enabled(ui: &mut Ui, value: &mut bool, enabled: bool) -> Response {
    let theme = Theme::get(ui.ctx());
    let size = Vec2::new(42.0, 23.0);
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(size, sense);

    let mut response = response;
    if enabled && response.clicked() {
        *value = !*value;
        response.mark_changed();
    }

    if ui.is_rect_visible(rect) {
        let palette = theme.palette;
        let on = ui
            .ctx()
            .animate_bool_with_time(response.id.with("on"), *value, theme.anim(0.13));
        let hover = ui.ctx().animate_bool_with_time(
            response.id.with("hover"),
            enabled && response.hovered(),
            theme.anim(0.11),
        );

        let off_fill = blend(palette.surface_press, palette.border_strong, hover);
        let on_fill = blend(palette.accent, palette.accent_hover, hover);
        let mut track = blend(off_fill, on_fill, on);
        let mut knob = blend(palette.text_muted, palette.on_accent, on);

        if !enabled {
            track = track.gamma_multiply(0.45);
            knob = knob.gamma_multiply(0.55);
        }

        ui.painter()
            .rect_filled(rect, egui::CornerRadius::same(12), track);

        if response.has_focus() {
            ui.painter().rect_stroke(
                rect.expand(2.0),
                egui::CornerRadius::same(14),
                Stroke::new(2.0, palette.accent),
                StrokeKind::Outside,
            );
        }

        let radius = rect.height() / 2.0 - 3.5;
        let travel = rect.width() - rect.height();
        let center = egui::pos2(
            rect.left() + rect.height() / 2.0 + travel * on,
            rect.center().y,
        );
        ui.painter().circle_filled(center, radius, knob);
    }

    if enabled && response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    response
}

pub struct Segmented<'a, T> {
    options: &'a [(T, &'a str)],
    min_width: f32,
}

impl<'a, T: Copy + PartialEq> Segmented<'a, T> {
    pub fn new(options: &'a [(T, &'a str)]) -> Self {
        Self {
            options,
            min_width: 0.0,
        }
    }

    pub fn show(self, ui: &mut Ui, value: &mut T) -> Response {
        let theme = Theme::get(ui.ctx());
        let font = theme::medium(theme::size::SMALL);
        let padding = 13.0;
        let height = theme.metrics.control_h;

        let widths: Vec<f32> = self
            .options
            .iter()
            .map(|(_, label)| {
                ui.painter()
                    .layout_no_wrap((*label).to_owned(), font.clone(), Color32::PLACEHOLDER)
                    .size()
                    .x
                    + padding * 2.0
            })
            .collect();

        let total = widths.iter().sum::<f32>().max(self.min_width) + 6.0;
        let (rect, response) = ui.allocate_exact_size(Vec2::new(total, height), Sense::click());
        let mut response = response;

        let palette = theme.palette;
        ui.painter()
            .rect_filled(rect, theme.radius_sm(), palette.surface_alt);
        ui.painter().rect_stroke(
            rect,
            theme.radius_sm(),
            Stroke::new(1.0, palette.border),
            StrokeKind::Inside,
        );

        let inner = rect.shrink(3.0);
        let scale = inner.width() / widths.iter().sum::<f32>().max(1.0);

        let selected_index = self
            .options
            .iter()
            .position(|(option, _)| option == value)
            .unwrap_or(0);

        let mut offsets = Vec::with_capacity(widths.len() + 1);
        let mut cursor = inner.left();
        for width in &widths {
            offsets.push(cursor);
            cursor += width * scale;
        }
        offsets.push(cursor);

        let target_left = offsets[selected_index];
        let target_right = offsets[selected_index + 1];
        let animated_left = ui.ctx().animate_value_with_time(
            response.id.with("left"),
            target_left,
            theme.anim(0.16),
        );
        let animated_right = ui.ctx().animate_value_with_time(
            response.id.with("right"),
            target_right,
            theme.anim(0.16),
        );

        let indicator = Rect::from_min_max(
            egui::pos2(animated_left, inner.top()),
            egui::pos2(animated_right, inner.bottom()),
        );
        ui.painter()
            .rect_filled(indicator, theme.radius_sm(), palette.surface);
        ui.painter().rect_stroke(
            indicator,
            theme.radius_sm(),
            Stroke::new(1.0, palette.border_strong),
            StrokeKind::Inside,
        );

        let pointer = ui.ctx().pointer_latest_pos();

        for (index, (option, label)) in self.options.iter().enumerate() {
            let segment = Rect::from_min_max(
                egui::pos2(offsets[index], inner.top()),
                egui::pos2(offsets[index + 1], inner.bottom()),
            );
            let hovered = pointer.map(|pos| segment.contains(pos)).unwrap_or(false);
            let is_selected = index == selected_index;

            if hovered && !is_selected {
                ui.painter().rect_filled(
                    segment.shrink(1.0),
                    theme.radius_sm(),
                    palette.surface_hover.gamma_multiply(0.6),
                );
            }

            let color = if is_selected || hovered {
                palette.text
            } else {
                palette.text_muted
            };

            ui.painter().text(
                segment.center(),
                Align2::CENTER_CENTER,
                label,
                font.clone(),
                color,
            );

            if response.clicked() && hovered && !is_selected {
                *value = *option;
                response.mark_changed();
            }
        }

        if response.has_focus() {
            ui.painter().rect_stroke(
                rect.expand(2.0),
                theme.radius_md(),
                Stroke::new(2.0, palette.accent),
                StrokeKind::Outside,
            );
        }

        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        response
    }
}

pub fn text_field(ui: &mut Ui, value: &mut String, placeholder: &str, width: f32) -> Response {
    let theme = Theme::get(ui.ctx());
    let edit = egui::TextEdit::singleline(value)
        .hint_text(
            egui::RichText::new(placeholder)
                .font(theme::text_style(theme::size::BODY))
                .color(theme.palette.text_faint),
        )
        .font(theme::text_style(theme::size::BODY))
        .text_color(theme.palette.text)
        .margin(egui::Margin::symmetric(10, 8))
        .background_color(theme.palette.surface_alt)
        .desired_width(width)
        .vertical_align(egui::Align::Center);

    let response = ui.add_sized(Vec2::new(width, theme.metrics.control_h), edit);

    let stroke = if response.has_focus() {
        Stroke::new(1.6, theme.palette.accent)
    } else if response.hovered() {
        Stroke::new(1.0, theme.palette.border_strong)
    } else {
        Stroke::new(1.0, theme.palette.border)
    };
    ui.painter()
        .rect_stroke(response.rect, theme.radius_sm(), stroke, StrokeKind::Inside);

    response
}

pub fn multiline_field(ui: &mut Ui, value: &mut String, rows: usize) -> Response {
    let theme = Theme::get(ui.ctx());
    let edit = egui::TextEdit::multiline(value)
        .font(egui::FontId::new(
            theme::size::SMALL,
            egui::FontFamily::Monospace,
        ))
        .text_color(theme.palette.text)
        .background_color(theme.palette.surface_alt)
        .margin(egui::Margin::same(10))
        .desired_width(f32::INFINITY)
        .desired_rows(rows);

    let response = ui.add(edit);
    ui.painter().rect_stroke(
        response.rect,
        theme.radius_sm(),
        Stroke::new(
            1.0,
            if response.has_focus() {
                theme.palette.accent
            } else {
                theme.palette.border
            },
        ),
        StrokeKind::Inside,
    );
    response
}

fn snap(value: f32, step: f32, range: &std::ops::RangeInclusive<f32>) -> f32 {
    let snapped = if step > 0.0 {
        range.start() + ((value - range.start()) / step).round() * step
    } else {
        value
    };
    snapped.clamp(*range.start(), *range.end())
}

pub fn slider(
    ui: &mut Ui,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    step: f32,
) -> Response {
    let theme = Theme::get(ui.ctx());
    let width = 190.0;
    let height = theme.metrics.control_h;
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(width, height), Sense::click_and_drag());
    let mut response = response;

    let track = Rect::from_center_size(rect.center(), Vec2::new(rect.width() - 20.0, 5.0));
    let span = (range.end() - range.start()).max(f32::EPSILON);
    let scale = ui.ctx().pixels_per_point();
    let anchor_id = response.id.with("anchor");

    let set = |next: f32, value: &mut f32, response: &mut Response| {
        if (next - *value).abs() > f32::EPSILON {
            *value = next;
            response.mark_changed();
        }
    };

    if response.drag_started() || response.clicked() {
        if let Some(pointer) = response.interact_pointer_pos() {
            let t = ((pointer.x - track.left()) / track.width()).clamp(0.0, 1.0);
            let next = snap(range.start() + t * span, step, &range);
            set(next, value, &mut response);
            ui.ctx().data_mut(|data| {
                data.insert_temp(anchor_id, (pointer.x * scale, next, scale));
            });
        }
    } else if response.dragged() {
        let anchor: Option<(f32, f32, f32)> = ui.ctx().data(|data| data.get_temp(anchor_id));
        if let (Some((from_x, from_value, from_scale)), Some(pointer)) =
            (anchor, ui.ctx().pointer_latest_pos())
        {
            let moved = (pointer.x * scale - from_x) / (track.width() * from_scale).max(1.0);
            let next = snap(from_value + moved * span, step, &range);
            set(next, value, &mut response);
        }
    }

    let t = ((*value - range.start()) / span).clamp(0.0, 1.0);
    let knob_x = track.left() + track.width() * t;
    let palette = theme.palette;

    ui.painter()
        .rect_filled(track, egui::CornerRadius::same(3), palette.surface_press);
    ui.painter().rect_filled(
        Rect::from_min_max(track.min, egui::pos2(knob_x, track.max.y)),
        egui::CornerRadius::same(3),
        palette.accent,
    );

    let hover = ui.ctx().animate_bool_with_time(
        response.id.with("hover"),
        response.hovered() || response.dragged(),
        theme.anim(0.11),
    );
    let radius = 8.0 + hover * 1.5;
    ui.painter().circle_filled(
        egui::pos2(knob_x, track.center().y),
        radius,
        if palette.is_dark {
            palette.text
        } else {
            Color32::WHITE
        },
    );
    ui.painter().circle_stroke(
        egui::pos2(knob_x, track.center().y),
        radius,
        Stroke::new(2.0, palette.accent),
    );

    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
    }

    response
}

pub fn stepper(
    ui: &mut Ui,
    value: &mut u64,
    range: std::ops::RangeInclusive<u64>,
    suffix: &str,
) -> bool {
    let theme = Theme::get(ui.ctx());
    let mut changed = false;

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;

        if super::button::Button::new("-")
            .tone(super::button::Tone::Neutral)
            .size(super::button::Size::Small)
            .min_width(30.0)
            .enabled(*value > *range.start())
            .show(ui)
            .clicked()
        {
            *value = value.saturating_sub(1).max(*range.start());
            changed = true;
        }

        let text = format!("{value}{suffix}");
        let font = theme::medium(theme::size::BODY);
        let width = 58.0;
        let (rect, _) = ui.allocate_exact_size(
            Vec2::new(width, theme.metrics.control_h - 6.0),
            Sense::hover(),
        );
        ui.painter()
            .rect_filled(rect, theme.radius_sm(), theme.palette.surface_alt);
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            text,
            font,
            theme.palette.text,
        );

        if super::button::Button::new("+")
            .tone(super::button::Tone::Neutral)
            .size(super::button::Size::Small)
            .min_width(30.0)
            .enabled(*value < *range.end())
            .show(ui)
            .clicked()
        {
            *value = (*value + 1).min(*range.end());
            changed = true;
        }
    });

    changed
}

#[cfg(test)]
mod tests {
    use super::snap;

    fn scale() -> std::ops::RangeInclusive<f32> {
        0.8..=1.6
    }

    #[test]
    fn snapping_lands_on_a_step() {
        assert_eq!(snap(1.0, 0.05, &scale()), 1.0);
        assert!((snap(1.02, 0.05, &scale()) - 1.0).abs() < 1e-5);
        assert!((snap(1.04, 0.05, &scale()) - 1.05).abs() < 1e-5);
    }

    #[test]
    fn snapping_never_leaves_the_range() {
        assert_eq!(snap(-4.0, 0.05, &scale()), 0.8);
        assert_eq!(snap(9.0, 0.05, &scale()), 1.6);
    }

    #[test]
    fn both_ends_of_the_range_are_reachable() {
        assert!((snap(0.81, 0.05, &scale()) - 0.8).abs() < 1e-5);
        assert!((snap(1.59, 0.05, &scale()) - 1.6).abs() < 1e-5);
    }

    #[test]
    fn a_zero_step_keeps_the_value_it_was_given() {
        assert!((snap(1.234, 0.0, &scale()) - 1.234).abs() < 1e-5);
    }
}

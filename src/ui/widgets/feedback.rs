use egui::{Align2, Color32, Rect, Sense, Stroke, Ui, Vec2};

use crate::ui::icons::{self, Icon};
use crate::ui::theme::{self, Theme};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Neutral,
    Accent,
    Success,
    Warning,
    Danger,
    Info,
}

impl Tone {
    pub fn color(self, theme: &Theme) -> Color32 {
        match self {
            Tone::Neutral => theme.palette.text_muted,
            Tone::Accent => theme.palette.accent,
            Tone::Success => theme.palette.success,
            Tone::Warning => theme.palette.warning,
            Tone::Danger => theme.palette.danger,
            Tone::Info => theme.palette.info,
        }
    }

    pub fn icon(self) -> Icon {
        match self {
            Tone::Neutral | Tone::Accent | Tone::Info => Icon::Info,
            Tone::Success => Icon::Check,
            Tone::Warning => Icon::Warning,
            Tone::Danger => Icon::Cross,
        }
    }
}

pub fn badge(ui: &mut Ui, text: &str, tone: Tone) {
    let theme = Theme::get(ui.ctx());
    let color = tone.color(&theme);
    let font = theme::medium(theme::size::MICRO);
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), font.clone(), Color32::PLACEHOLDER);

    let size = Vec2::new(galley.size().x + 14.0, 18.0);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());

    let fill = match tone {
        Tone::Neutral => theme.palette.surface_alt,
        _ => color.gamma_multiply(0.14),
    };
    let ink = match tone {
        Tone::Neutral => theme.palette.text_muted,
        _ => color,
    };

    ui.painter().rect_filled(rect, theme.radius_sm(), fill);
    ui.painter()
        .text(rect.center(), Align2::CENTER_CENTER, text, font, ink);
}

const PILL_PAD: f32 = 11.0;
const PILL_DOT: f32 = 4.0;
const PILL_GAP: f32 = 7.0;

pub fn status_pill(ui: &mut Ui, text: &str, tone: Tone, pulsing: bool) {
    let theme = Theme::get(ui.ctx());
    let color = tone.color(&theme);
    let font = theme::medium(theme::size::SMALL);
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), font.clone(), theme.palette.text);

    let lead = PILL_PAD + PILL_DOT * 2.0 + PILL_GAP;
    let size = Vec2::new(lead + galley.size().x + PILL_PAD, 24.0);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());

    ui.painter()
        .rect_filled(rect, theme.radius_sm(), theme.palette.surface_alt);

    let dot = egui::pos2(rect.left() + PILL_PAD + PILL_DOT, rect.center().y);
    if pulsing && theme.metrics.animations {
        let time = ui.input(|input| input.time);
        let wave = ((time * 2.0).sin() as f32 * 0.5 + 0.5).clamp(0.0, 1.0);
        ui.painter().circle_filled(
            dot,
            PILL_DOT + wave * 3.5,
            color.gamma_multiply(0.25 * (1.0 - wave)),
        );
        ui.ctx().request_repaint();
    }
    ui.painter().circle_filled(dot, PILL_DOT, color);

    ui.painter().galley(
        egui::pos2(
            rect.left() + lead,
            rect.center().y - galley.size().y / 2.0 + theme::optical_nudge(theme::size::SMALL),
        ),
        galley,
        theme.palette.text,
    );
}

pub fn banner(ui: &mut Ui, tone: Tone, title: &str, body: &str) {
    let theme = Theme::get(ui.ctx());
    let color = tone.color(&theme);

    egui::Frame::new()
        .fill(color.gamma_multiply(0.08))
        .stroke(Stroke::new(1.0, color.gamma_multiply(0.24)))
        .corner_radius(theme.radius_sm())
        .inner_margin(egui::Margin::symmetric(
            theme.metrics.gap_md as i8,
            theme.metrics.gap_md as i8,
        ))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = theme.metrics.gap_md;

                let (icon_rect, _) = ui.allocate_exact_size(Vec2::splat(19.0), Sense::hover());
                icons::draw(ui.painter(), tone.icon(), icon_rect, color, 1.8);

                ui.vertical(|ui| {
                    ui.set_width(ui.available_width());
                    ui.label(
                        egui::RichText::new(title)
                            .font(theme::medium(theme::size::BODY))
                            .color(theme.palette.text),
                    );
                    if !body.is_empty() {
                        ui.add_space(2.0);
                        ui.label(
                            egui::RichText::new(body)
                                .font(theme::text_style(theme::size::SMALL))
                                .color(theme.palette.text_muted),
                        );
                    }
                });
            });
        });
}

pub fn stat(ui: &mut Ui, label: &str, value: &str, tone: Tone) {
    let theme = Theme::get(ui.ctx());
    ui.vertical(|ui| {
        ui.label(
            egui::RichText::new(value)
                .font(theme::strong(theme::size::TITLE))
                .color(tone.color(&theme)),
        );
        ui.add_space(1.0);
        ui.label(
            egui::RichText::new(label)
                .font(theme::text_style(theme::size::MICRO))
                .color(theme.palette.text_muted),
        );
    });
}

pub fn turning(ui: &Ui, theme: &Theme) -> f64 {
    if !theme.metrics.animations {
        return 0.0;
    }
    ui.ctx().request_repaint();
    ui.input(|input| input.time)
}

pub fn step_marker(ui: &mut Ui, rect: Rect, state: MarkerState, index: usize) {
    let theme = Theme::get(ui.ctx());
    let palette = theme.palette;

    match state {
        MarkerState::Pending => {
            ui.painter().circle_stroke(
                rect.center(),
                rect.width() / 2.0 - 1.0,
                Stroke::new(1.5, palette.border_strong),
            );
            ui.painter().text(
                rect.center(),
                Align2::CENTER_CENTER,
                format!("{index}"),
                theme::medium(theme::size::MICRO),
                palette.text_faint,
            );
        }
        MarkerState::Active => {
            ui.painter().circle_filled(
                rect.center(),
                rect.width() / 2.0,
                palette.accent.gamma_multiply(0.16),
            );
            icons::spinner(
                ui.painter(),
                rect.shrink(2.0),
                palette.accent,
                2.0,
                turning(ui, &theme),
            );
        }
        MarkerState::Done => {
            ui.painter().circle_filled(
                rect.center(),
                rect.width() / 2.0,
                palette.success.gamma_multiply(0.18),
            );
            icons::draw(
                ui.painter(),
                Icon::Check,
                rect.shrink(5.0),
                palette.success,
                2.0,
            );
        }
        MarkerState::Skipped => {
            ui.painter().circle_stroke(
                rect.center(),
                rect.width() / 2.0 - 1.0,
                Stroke::new(1.5, palette.border_strong),
            );
            ui.painter().line_segment(
                [
                    egui::pos2(rect.center().x - 4.0, rect.center().y),
                    egui::pos2(rect.center().x + 4.0, rect.center().y),
                ],
                Stroke::new(1.8, palette.text_faint),
            );
        }
        MarkerState::Failed => {
            ui.painter().circle_filled(
                rect.center(),
                rect.width() / 2.0,
                palette.danger.gamma_multiply(0.18),
            );
            icons::draw(
                ui.painter(),
                Icon::Cross,
                rect.shrink(6.0),
                palette.danger,
                2.0,
            );
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkerState {
    Pending,
    Active,
    Done,
    Skipped,
    Failed,
}

pub fn progress_bar(ui: &mut Ui, fraction: f32, label: &str, trailing: &str) {
    let theme = Theme::get(ui.ctx());
    let palette = theme.palette;
    let fraction = fraction.clamp(0.0, 1.0);

    if !label.is_empty() || !trailing.is_empty() {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(label)
                    .font(theme::medium(theme::size::SMALL))
                    .color(palette.text),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(trailing)
                        .font(theme::text_style(theme::size::MICRO))
                        .color(palette.text_muted),
                );
            });
        });
        ui.add_space(6.0);
    }

    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 8.0), Sense::hover());

    ui.painter()
        .rect_filled(rect, egui::CornerRadius::same(4), palette.surface_press);

    let eased =
        ui.ctx()
            .animate_value_with_time(ui.id().with("progress"), fraction, theme.anim(0.18));

    if eased > 0.0 {
        let filled = Rect::from_min_size(
            rect.min,
            Vec2::new((rect.width() * eased).max(8.0), rect.height()),
        );
        ui.painter()
            .rect_filled(filled, egui::CornerRadius::same(4), palette.accent);
    }
}

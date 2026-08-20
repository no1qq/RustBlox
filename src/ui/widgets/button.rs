use egui::{Align2, Color32, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2};

use crate::ui::icons::{self, Icon};
use crate::ui::theme::{self, Theme};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Primary,
    Neutral,
    Ghost,
    Danger,
    Quiet,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Size {
    Small,
    Regular,
    Large,
}

impl Size {
    fn height(self, theme: &Theme) -> f32 {
        match self {
            Size::Small => theme.metrics.button_h - 6.0,
            Size::Regular => theme.metrics.button_h,
            Size::Large => theme.metrics.button_h + 10.0,
        }
    }

    fn font(self) -> f32 {
        match self {
            Size::Small => theme::size::SMALL,
            Size::Regular => theme::size::BODY,
            Size::Large => theme::size::SECTION,
        }
    }

    fn padding(self) -> f32 {
        match self {
            Size::Small => 11.0,
            Size::Regular => 15.0,
            Size::Large => 21.0,
        }
    }

    fn icon(self) -> f32 {
        match self {
            Size::Small => 14.0,
            Size::Regular => 16.0,
            Size::Large => 19.0,
        }
    }
}

pub struct Button<'a> {
    label: &'a str,
    icon: Option<Icon>,
    tone: Tone,
    size: Size,
    enabled: bool,
    min_width: f32,
    fill_width: bool,
}

impl<'a> Button<'a> {
    pub fn new(label: &'a str) -> Self {
        Self {
            label,
            icon: None,
            tone: Tone::Neutral,
            size: Size::Regular,
            enabled: true,
            min_width: 0.0,
            fill_width: false,
        }
    }

    pub fn primary(label: &'a str) -> Self {
        Self::new(label).tone(Tone::Primary)
    }

    pub fn danger(label: &'a str) -> Self {
        Self::new(label).tone(Tone::Danger)
    }

    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn min_width(mut self, width: f32) -> Self {
        self.min_width = width;
        self
    }

    pub fn fill_width(mut self, fill: bool) -> Self {
        self.fill_width = fill;
        self
    }

    pub fn show(self, ui: &mut Ui) -> Response {
        let theme = Theme::get(ui.ctx());
        let font = theme::medium(self.size.font());
        let galley =
            ui.painter()
                .layout_no_wrap(self.label.to_owned(), font.clone(), Color32::PLACEHOLDER);

        let icon_size = self.size.icon();
        let gap = if self.icon.is_some() && !self.label.is_empty() {
            8.0
        } else {
            0.0
        };
        let icon_width = if self.icon.is_some() { icon_size } else { 0.0 };
        let height = self.size.height(&theme);
        let natural = galley.size().x + icon_width + gap + self.size.padding() * 2.0;

        let width = if self.fill_width {
            ui.available_width().max(natural)
        } else {
            natural.max(self.min_width)
        };

        let sense = if self.enabled {
            Sense::click()
        } else {
            Sense::hover()
        };
        let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), sense);
        if ui.is_rect_visible(rect) {
            paint(
                ui,
                &theme,
                rect,
                &response,
                self.tone,
                self.enabled,
                self.icon,
                icon_size,
                gap,
                &galley,
                font,
            );
        }

        if self.enabled && response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        response
    }
}

#[allow(clippy::too_many_arguments)]
fn paint(
    ui: &Ui,
    theme: &Theme,
    rect: Rect,
    response: &Response,
    tone: Tone,
    enabled: bool,
    icon: Option<Icon>,
    icon_size: f32,
    gap: f32,
    galley: &std::sync::Arc<egui::Galley>,
    font: egui::FontId,
) {
    let palette = theme.palette;
    let id = response.id;

    let hover =
        ui.ctx()
            .animate_bool_with_time(id.with("hover"), enabled && response.hovered(), 0.11);
    let press = ui.ctx().animate_bool_with_time(
        id.with("press"),
        enabled && response.is_pointer_button_down_on(),
        0.06,
    );

    let (mut fill, mut border, mut text) = match tone {
        Tone::Primary => (palette.accent, Color32::TRANSPARENT, palette.on_accent),
        Tone::Neutral => (palette.surface_alt, palette.border_strong, palette.text),
        Tone::Ghost => (Color32::TRANSPARENT, palette.border, palette.text_muted),
        Tone::Quiet => (
            Color32::TRANSPARENT,
            Color32::TRANSPARENT,
            palette.text_muted,
        ),
        Tone::Danger => (palette.danger, Color32::TRANSPARENT, Color32::WHITE),
    };

    if hover > 0.0 {
        fill = match tone {
            Tone::Primary => blend(fill, palette.accent_hover, hover),
            Tone::Danger => blend(fill, lighten(palette.danger), hover),
            Tone::Ghost | Tone::Quiet => blend(palette.surface_alt, palette.surface_hover, hover)
                .gamma_multiply(hover.min(1.0)),
            Tone::Neutral => blend(fill, palette.surface_hover, hover),
        };
        if matches!(tone, Tone::Ghost | Tone::Quiet) {
            text = blend(text, palette.text, hover);
        }
        if tone == Tone::Ghost {
            border = blend(border, palette.border_strong, hover);
        }
    }

    if press > 0.0 {
        fill = match tone {
            Tone::Primary => blend(fill, palette.accent_press, press),
            Tone::Danger => blend(fill, darken(palette.danger), press),
            _ => blend(fill, palette.surface_press, press),
        };
    }

    if !enabled {
        fill = fill.gamma_multiply(0.42);
        border = border.gamma_multiply(0.5);
        text = text.gamma_multiply(0.42);
    }

    let radius = theme.radius_sm();
    if fill.a() > 0 {
        ui.painter().rect_filled(rect, radius, fill);
    }
    if border.a() > 0 {
        ui.painter()
            .rect_stroke(rect, radius, Stroke::new(1.0, border), StrokeKind::Inside);
    }

    if response.has_focus() {
        ui.painter().rect_stroke(
            rect.expand(2.0),
            theme.radius_md(),
            Stroke::new(2.0, palette.accent),
            StrokeKind::Outside,
        );
    }

    let content_width = galley.size().x + if icon.is_some() { icon_size + gap } else { 0.0 };
    let mut cursor = rect.center().x - content_width / 2.0;

    if let Some(icon) = icon {
        let icon_rect = Rect::from_min_size(
            egui::pos2(cursor, rect.center().y - icon_size / 2.0),
            Vec2::splat(icon_size),
        );
        icons::draw(ui.painter(), icon, icon_rect, text, 1.7);
        cursor += icon_size + gap;
    }

    ui.painter().text(
        egui::pos2(cursor, rect.center().y),
        Align2::LEFT_CENTER,
        galley.text(),
        font,
        text,
    );
}

pub fn icon_button(ui: &mut Ui, icon: Icon, tooltip: &str, enabled: bool) -> Response {
    let theme = Theme::get(ui.ctx());
    let side = theme.metrics.control_h;
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(side), sense);
    if ui.is_rect_visible(rect) {
        let palette = theme.palette;
        let hover = ui.ctx().animate_bool_with_time(
            response.id.with("hover"),
            enabled && response.hovered(),
            0.11,
        );
        let press = ui.ctx().animate_bool_with_time(
            response.id.with("press"),
            enabled && response.is_pointer_button_down_on(),
            0.06,
        );

        let fill = blend(palette.surface_alt, palette.surface_hover, hover)
            .gamma_multiply((hover * 0.9 + press * 0.1).clamp(0.0, 1.0));
        let mut tint = blend(palette.text_muted, palette.text, hover);
        if !enabled {
            tint = tint.gamma_multiply(0.4);
        }

        if hover > 0.0 {
            ui.painter().rect_filled(rect, theme.radius_sm(), fill);
        }
        if response.has_focus() {
            ui.painter().rect_stroke(
                rect,
                theme.radius_sm(),
                Stroke::new(2.0, palette.accent),
                StrokeKind::Inside,
            );
        }

        icons::draw(ui.painter(), icon, rect.shrink(side * 0.26), tint, 1.7);
    }

    if enabled && response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    if tooltip.is_empty() {
        response
    } else {
        response.on_hover_text(tooltip)
    }
}

pub fn blend(from: Color32, to: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let mix = |a: u8, b: u8| ((a as f32) + ((b as f32) - (a as f32)) * t).round() as u8;
    Color32::from_rgba_premultiplied(
        mix(from.r(), to.r()),
        mix(from.g(), to.g()),
        mix(from.b(), to.b()),
        mix(from.a(), to.a()),
    )
}

fn lighten(color: Color32) -> Color32 {
    blend(color, Color32::WHITE, 0.14)
}

fn darken(color: Color32) -> Color32 {
    blend(color, Color32::BLACK, 0.14)
}

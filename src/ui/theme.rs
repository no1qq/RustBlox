use std::sync::Arc;

use egui::{Color32, CornerRadius, FontFamily, FontId, Margin, Stroke};

use crate::config::{Accent, AppearanceSettings, Density, Theme as ThemeChoice};

pub const FAMILY_MEDIUM: &str = "rustblox-medium";
pub const FAMILY_STRONG: &str = "rustblox-strong";

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub window: Color32,
    pub sidebar: Color32,
    pub surface: Color32,
    pub surface_alt: Color32,
    pub surface_hover: Color32,
    pub surface_press: Color32,
    pub border: Color32,
    pub border_strong: Color32,
    pub text: Color32,
    pub text_muted: Color32,
    pub text_faint: Color32,
    pub accent: Color32,
    pub accent_hover: Color32,
    pub accent_press: Color32,
    pub accent_soft: Color32,
    pub on_accent: Color32,
    pub success: Color32,
    pub warning: Color32,
    pub danger: Color32,
    pub info: Color32,
    pub scrim: Color32,
    pub is_dark: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Metrics {
    pub gap_xs: f32,
    pub gap_sm: f32,
    pub gap_md: f32,
    pub gap_lg: f32,
    pub gap_xl: f32,
    pub card_pad: f32,
    pub page_pad: f32,
    pub radius_sm: u8,
    pub radius_md: u8,
    pub radius_lg: u8,
    pub control_h: f32,
    pub button_h: f32,
    pub row_h: f32,
    pub sidebar_w: f32,
    pub titlebar_h: f32,
    pub animations: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    pub palette: Palette,
    pub metrics: Metrics,
}

impl Theme {
    pub fn from_settings(settings: &AppearanceSettings) -> Self {
        Self {
            palette: palette_for(settings.theme, settings.accent),
            metrics: metrics_for(settings.density, settings.animations),
        }
    }

    pub fn store(self, ctx: &egui::Context) {
        ctx.data_mut(|data| data.insert_temp(theme_key(), self));
    }

    pub fn get(ctx: &egui::Context) -> Self {
        ctx.data(|data| data.get_temp::<Theme>(theme_key()))
            .unwrap_or_else(|| Theme::from_settings(&AppearanceSettings::default()))
    }

    pub fn radius_sm(&self) -> CornerRadius {
        CornerRadius::same(self.metrics.radius_sm)
    }

    pub fn radius_md(&self) -> CornerRadius {
        CornerRadius::same(self.metrics.radius_md)
    }

    pub fn radius_lg(&self) -> CornerRadius {
        CornerRadius::same(self.metrics.radius_lg)
    }

    pub fn hairline(&self) -> Stroke {
        Stroke::new(1.0, self.palette.border)
    }

    pub fn card_margin(&self) -> Margin {
        Margin::same(self.metrics.card_pad as i8)
    }
}

fn theme_key() -> egui::Id {
    egui::Id::new("rustblox-theme")
}

fn rgb(value: u32) -> Color32 {
    Color32::from_rgb(
        ((value >> 16) & 0xFF) as u8,
        ((value >> 8) & 0xFF) as u8,
        (value & 0xFF) as u8,
    )
}

fn lighten(color: Color32, amount: f32) -> Color32 {
    let mix = |channel: u8| -> u8 {
        let value = channel as f32;
        (value + (255.0 - value) * amount).clamp(0.0, 255.0) as u8
    };
    Color32::from_rgb(mix(color.r()), mix(color.g()), mix(color.b()))
}

fn darken(color: Color32, amount: f32) -> Color32 {
    let mix = |channel: u8| -> u8 { (channel as f32 * (1.0 - amount)).clamp(0.0, 255.0) as u8 };
    Color32::from_rgb(mix(color.r()), mix(color.g()), mix(color.b()))
}

fn readable_on(color: Color32) -> Color32 {
    let luminance =
        0.2126 * color.r() as f32 + 0.7152 * color.g() as f32 + 0.0722 * color.b() as f32;
    if luminance > 150.0 {
        rgb(0x10131A)
    } else {
        Color32::WHITE
    }
}

fn palette_for(choice: ThemeChoice, accent: Accent) -> Palette {
    let [r, g, b] = accent.rgb();
    let accent = Color32::from_rgb(r, g, b);

    match choice {
        ThemeChoice::Midnight => Palette {
            window: rgb(0x0E1015),
            sidebar: rgb(0x12141B),
            surface: rgb(0x171A22),
            surface_alt: rgb(0x1D212B),
            surface_hover: rgb(0x232834),
            surface_press: rgb(0x2A2F3D),
            border: rgb(0x252A35),
            border_strong: rgb(0x353C4C),
            text: rgb(0xE9EBF1),
            text_muted: rgb(0x99A1B3),
            text_faint: rgb(0x6A7387),
            accent,
            accent_hover: lighten(accent, 0.12),
            accent_press: darken(accent, 0.12),
            accent_soft: accent.gamma_multiply(0.16),
            on_accent: readable_on(accent),
            success: rgb(0x4FC48D),
            warning: rgb(0xE9B457),
            danger: rgb(0xF06B62),
            info: rgb(0x5EA8F0),
            scrim: Color32::from_black_alpha(190),
            is_dark: true,
        },
        ThemeChoice::Graphite => Palette {
            window: rgb(0x18181B),
            sidebar: rgb(0x1D1D21),
            surface: rgb(0x232327),
            surface_alt: rgb(0x2A2A2F),
            surface_hover: rgb(0x313137),
            surface_press: rgb(0x38383F),
            border: rgb(0x32323A),
            border_strong: rgb(0x45454F),
            text: rgb(0xEDEDF0),
            text_muted: rgb(0xA2A2AC),
            text_faint: rgb(0x74747F),
            accent,
            accent_hover: lighten(accent, 0.12),
            accent_press: darken(accent, 0.12),
            accent_soft: accent.gamma_multiply(0.16),
            on_accent: readable_on(accent),
            success: rgb(0x54C793),
            warning: rgb(0xE7B45C),
            danger: rgb(0xEE6E66),
            info: rgb(0x62A9EE),
            scrim: Color32::from_black_alpha(190),
            is_dark: true,
        },
        ThemeChoice::Daylight => Palette {
            window: rgb(0xF3F5F9),
            sidebar: rgb(0xFFFFFF),
            surface: rgb(0xFFFFFF),
            surface_alt: rgb(0xF1F3F8),
            surface_hover: rgb(0xE8EBF2),
            surface_press: rgb(0xDDE2EC),
            border: rgb(0xE1E5EE),
            border_strong: rgb(0xC7CDDA),
            text: rgb(0x141821),
            text_muted: rgb(0x5B6478),
            text_faint: rgb(0x8A92A4),
            accent: darken(accent, 0.12),
            accent_hover: darken(accent, 0.02),
            accent_press: darken(accent, 0.26),
            accent_soft: accent.gamma_multiply(0.18),
            on_accent: readable_on(darken(accent, 0.12)),
            success: rgb(0x1B8B5F),
            warning: rgb(0xA97614),
            danger: rgb(0xC93F3B),
            info: rgb(0x1F6FBE),
            scrim: Color32::from_black_alpha(120),
            is_dark: false,
        },
    }
}

fn metrics_for(density: Density, animations: bool) -> Metrics {
    let scale = density.scale();
    let round = |value: f32| (value * scale).round();

    Metrics {
        gap_xs: round(4.0).max(3.0),
        gap_sm: round(8.0).max(6.0),
        gap_md: round(13.0).max(9.0),
        gap_lg: round(20.0).max(14.0),
        gap_xl: round(28.0).max(20.0),
        card_pad: round(18.0).max(13.0),
        page_pad: round(26.0).max(18.0),
        radius_sm: 6,
        radius_md: 10,
        radius_lg: 14,
        control_h: round(34.0).max(28.0),
        button_h: round(36.0).max(30.0),
        row_h: round(44.0).max(36.0),
        sidebar_w: round(236.0).max(196.0),
        titlebar_h: 40.0,
        animations,
    }
}

pub fn text_style(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}

pub fn medium(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(Arc::from(FAMILY_MEDIUM)))
}

pub fn strong(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(Arc::from(FAMILY_STRONG)))
}

pub mod size {
    pub const DISPLAY: f32 = 25.0;
    pub const TITLE: f32 = 17.0;
    pub const SECTION: f32 = 14.5;
    pub const BODY: f32 = 13.5;
    pub const SMALL: f32 = 12.5;
    pub const MICRO: f32 = 11.5;
}

pub fn install_fonts(ctx: &egui::Context) -> bool {
    let mut definitions = egui::FontDefinitions::default();
    let mut loaded_any = false;

    let regular = load_system_font(&["segoeui.ttf", "arial.ttf"]);
    let medium = load_system_font(&["seguisb.ttf", "segoeui.ttf", "arialbd.ttf"]);
    let strong = load_system_font(&["segoeuib.ttf", "seguisb.ttf", "arialbd.ttf"]);

    if let Some(bytes) = regular {
        definitions.font_data.insert(
            "rustblox-regular".into(),
            Arc::new(egui::FontData::from_owned(bytes)),
        );
        definitions
            .families
            .entry(FontFamily::Proportional)
            .or_default()
            .insert(0, "rustblox-regular".into());
        loaded_any = true;
    }

    for (key, bytes, family) in [
        ("rustblox-medium", medium, FAMILY_MEDIUM),
        ("rustblox-strong", strong, FAMILY_STRONG),
    ] {
        let mut names = Vec::new();
        if let Some(bytes) = bytes {
            definitions
                .font_data
                .insert(key.into(), Arc::new(egui::FontData::from_owned(bytes)));
            names.push(key.to_string());
            loaded_any = true;
        }
        names.extend(
            definitions
                .families
                .get(&FontFamily::Proportional)
                .cloned()
                .unwrap_or_default(),
        );
        definitions
            .families
            .insert(FontFamily::Name(Arc::from(family)), names);
    }

    ctx.set_fonts(definitions);
    loaded_any
}

fn load_system_font(candidates: &[&str]) -> Option<Vec<u8>> {
    let roots = font_directories();
    for name in candidates {
        for root in &roots {
            let path = root.join(name);
            if let Ok(bytes) = std::fs::read(&path) {
                if !bytes.is_empty() {
                    return Some(bytes);
                }
            }
        }
    }
    None
}

fn font_directories() -> Vec<std::path::PathBuf> {
    let mut roots = Vec::new();

    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        roots.push(
            std::path::PathBuf::from(local)
                .join("Microsoft")
                .join("Windows")
                .join("Fonts"),
        );
    }
    if let Some(windir) = std::env::var_os("SystemRoot").or_else(|| std::env::var_os("windir")) {
        roots.push(std::path::PathBuf::from(windir).join("Fonts"));
    }

    roots
}

pub fn apply_style(ctx: &egui::Context, theme: &Theme, scale: f32) {
    let palette = theme.palette;
    let egui_theme = if palette.is_dark {
        egui::Theme::Dark
    } else {
        egui::Theme::Light
    };
    ctx.set_theme(if palette.is_dark {
        egui::ThemePreference::Dark
    } else {
        egui::ThemePreference::Light
    });
    let mut style = (*ctx.style_of(egui_theme)).clone();

    style.visuals.dark_mode = palette.is_dark;
    style.visuals.override_text_color = Some(palette.text);
    style.visuals.panel_fill = palette.window;
    style.visuals.window_fill = palette.surface;
    style.visuals.extreme_bg_color = palette.surface_alt;
    style.visuals.faint_bg_color = palette.surface_alt;
    style.visuals.window_stroke = Stroke::new(1.0, palette.border);
    style.visuals.window_corner_radius = theme.radius_lg();
    style.visuals.selection.bg_fill = palette.accent.gamma_multiply(0.34);
    style.visuals.selection.stroke = Stroke::new(1.0, palette.accent);
    style.visuals.hyperlink_color = palette.accent;
    style.visuals.text_cursor.stroke = Stroke::new(1.5, palette.accent);
    style.visuals.window_shadow = egui::epaint::Shadow {
        offset: [0, 12],
        blur: 32,
        spread: 0,
        color: Color32::from_black_alpha(if palette.is_dark { 140 } else { 46 }),
    };
    style.visuals.popup_shadow = style.visuals.window_shadow;

    for widget in [
        &mut style.visuals.widgets.noninteractive,
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
        &mut style.visuals.widgets.open,
    ] {
        widget.corner_radius = theme.radius_sm();
        widget.fg_stroke = Stroke::new(1.0, palette.text);
        widget.bg_stroke = Stroke::new(1.0, palette.border);
        widget.expansion = 0.0;
    }

    style.visuals.widgets.noninteractive.bg_fill = palette.surface;
    style.visuals.widgets.noninteractive.weak_bg_fill = palette.surface;
    style.visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, palette.text_muted);
    style.visuals.widgets.inactive.bg_fill = palette.surface_alt;
    style.visuals.widgets.inactive.weak_bg_fill = palette.surface_alt;
    style.visuals.widgets.hovered.bg_fill = palette.surface_hover;
    style.visuals.widgets.hovered.weak_bg_fill = palette.surface_hover;
    style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, palette.border_strong);
    style.visuals.widgets.active.bg_fill = palette.surface_press;
    style.visuals.widgets.active.weak_bg_fill = palette.surface_press;
    style.visuals.widgets.active.bg_stroke = Stroke::new(1.0, palette.accent);

    style.spacing.item_spacing = egui::vec2(theme.metrics.gap_sm, theme.metrics.gap_sm);
    style.spacing.button_padding = egui::vec2(12.0, 6.0);
    style.spacing.interact_size = egui::vec2(40.0, theme.metrics.control_h);
    style.spacing.scroll.bar_width = 8.0;
    style.spacing.scroll.floating = true;
    style.spacing.scroll.bar_inner_margin = 2.0;
    style.spacing.menu_margin = Margin::same(6);
    style.spacing.slider_width = 190.0;

    style.text_styles = [
        (egui::TextStyle::Heading, strong(size::TITLE)),
        (egui::TextStyle::Body, text_style(size::BODY)),
        (egui::TextStyle::Button, medium(size::BODY)),
        (egui::TextStyle::Small, text_style(size::SMALL)),
        (
            egui::TextStyle::Monospace,
            FontId::new(size::SMALL, FontFamily::Monospace),
        ),
    ]
    .into();

    style.interaction.selectable_labels = false;
    style.animation_time = if theme.metrics.animations { 0.14 } else { 0.0 };

    ctx.set_style_of(egui::Theme::Dark, style.clone());
    ctx.set_style_of(egui::Theme::Light, style);
    ctx.set_pixels_per_point(scale);
}

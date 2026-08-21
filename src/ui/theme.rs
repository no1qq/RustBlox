use std::sync::Arc;

use egui::{Color32, CornerRadius, FontFamily, FontId, Margin, Stroke};

use crate::config::{Accent, AppearanceSettings, Density};

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
    pub fn from_settings(settings: &AppearanceSettings, system_is_dark: bool) -> Self {
        Self {
            palette: palette_for(settings.mode.is_dark(system_is_dark), settings.accent),
            metrics: metrics_for(settings.density, settings.animations),
        }
    }

    pub fn store(self, ctx: &egui::Context) {
        ctx.data_mut(|data| data.insert_temp(theme_key(), self));
    }

    pub fn get(ctx: &egui::Context) -> Self {
        ctx.data(|data| data.get_temp::<Theme>(theme_key()))
            .unwrap_or_else(|| Theme::from_settings(&AppearanceSettings::default(), true))
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

    pub fn anim(&self, seconds: f32) -> f32 {
        if self.metrics.animations {
            seconds
        } else {
            0.0
        }
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

const INK: Color32 = Color32::from_rgb(0x17, 0x18, 0x1B);
const LIGHT_WINDOW: Color32 = Color32::from_rgb(0xF4, 0xF4, 0xF5);

fn relative_luminance(color: Color32) -> f32 {
    let channel = |value: u8| {
        let value = value as f32 / 255.0;
        if value <= 0.03928 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(color.r()) + 0.7152 * channel(color.g()) + 0.0722 * channel(color.b())
}

fn contrast_ratio(a: Color32, b: Color32) -> f32 {
    let (x, y) = (relative_luminance(a), relative_luminance(b));
    let (high, low) = if x > y { (x, y) } else { (y, x) };
    (high + 0.05) / (low + 0.05)
}

fn darkened_until_readable(color: Color32, against: Color32) -> Color32 {
    let mut amount = 0.16;
    let mut result = darken(color, amount);
    while contrast_ratio(against, result) < 4.6 && amount < 0.8 {
        amount += 0.04;
        result = darken(color, amount);
    }
    result
}

fn readable_on(color: Color32) -> Color32 {
    if contrast_ratio(INK, color) >= contrast_ratio(Color32::WHITE, color) {
        INK
    } else {
        Color32::WHITE
    }
}

fn palette_for(is_dark: bool, accent: Accent) -> Palette {
    let [r, g, b] = accent.rgb();
    let raw = Color32::from_rgb(r, g, b);

    if is_dark {
        Palette {
            window: rgb(0x161719),
            sidebar: rgb(0x111214),
            surface: rgb(0x1C1D20),
            surface_alt: rgb(0x232427),
            surface_hover: rgb(0x2A2B2F),
            surface_press: rgb(0x313236),
            border: rgb(0x27282C),
            border_strong: rgb(0x3B3C42),
            text: rgb(0xEDEEF0),
            text_muted: rgb(0x9A9CA3),
            text_faint: rgb(0x6C6E76),
            accent: raw,
            accent_hover: lighten(raw, 0.14),
            accent_press: darken(raw, 0.14),
            accent_soft: raw.gamma_multiply(0.14),
            on_accent: readable_on(raw),
            success: rgb(0x53C08A),
            warning: rgb(0xE0A845),
            danger: rgb(0xE8645C),
            info: rgb(0x5FA3E8),
            scrim: Color32::from_black_alpha(185),
            is_dark: true,
        }
    } else {
        let ink = darkened_until_readable(raw, LIGHT_WINDOW);
        Palette {
            window: LIGHT_WINDOW,
            sidebar: rgb(0xFFFFFF),
            surface: rgb(0xFFFFFF),
            surface_alt: rgb(0xF1F1F3),
            surface_hover: rgb(0xE9E9EC),
            surface_press: rgb(0xDEDEE2),
            border: rgb(0xE3E3E6),
            border_strong: rgb(0xC5C5CB),
            text: rgb(0x17181B),
            text_muted: rgb(0x5B5D64),
            text_faint: rgb(0x8A8C93),
            accent: ink,
            accent_hover: lighten(ink, 0.10),
            accent_press: darken(ink, 0.18),
            accent_soft: raw.gamma_multiply(0.16),
            on_accent: readable_on(ink),
            success: rgb(0x18784F),
            warning: rgb(0x8F6410),
            danger: rgb(0xB93732),
            info: rgb(0x1C5FA6),
            scrim: Color32::from_black_alpha(110),
            is_dark: false,
        }
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
        radius_sm: 3,
        radius_md: 5,
        radius_lg: 7,
        control_h: round(34.0).max(28.0),
        button_h: round(36.0).max(30.0),
        row_h: round(44.0).max(36.0),
        sidebar_w: round(236.0).max(196.0),
        titlebar_h: 40.0,
        animations,
    }
}

pub fn optical_nudge(size: f32) -> f32 {
    size * 0.08
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
    ctx.tessellation_options_mut(|options| {
        options.feathering = true;
        options.feathering_size_in_pixels = 1.0;
    });
    ctx.set_zoom_factor(scale);
}

#[cfg(test)]
mod tests {
    use super::contrast_ratio as contrast;
    use super::*;
    use crate::config::ThemeMode;

    fn settings(mode: ThemeMode) -> AppearanceSettings {
        AppearanceSettings {
            mode,
            ..AppearanceSettings::default()
        }
    }

    #[test]
    fn automatic_follows_windows_both_ways() {
        assert!(
            Theme::from_settings(&settings(ThemeMode::Auto), true)
                .palette
                .is_dark
        );
        assert!(
            !Theme::from_settings(&settings(ThemeMode::Auto), false)
                .palette
                .is_dark
        );
    }

    #[test]
    fn a_manual_choice_ignores_windows() {
        for system_is_dark in [true, false] {
            assert!(
                Theme::from_settings(&settings(ThemeMode::Dark), system_is_dark)
                    .palette
                    .is_dark
            );
            assert!(
                !Theme::from_settings(&settings(ThemeMode::Light), system_is_dark)
                    .palette
                    .is_dark
            );
        }
    }

    #[test]
    fn the_default_is_to_follow_windows() {
        assert_eq!(AppearanceSettings::default().mode, ThemeMode::Auto);
    }

    #[test]
    fn text_stays_readable_against_the_background_in_both_modes() {
        for is_dark in [true, false] {
            let palette = palette_for(is_dark, Accent::Ember);
            assert!(
                contrast(palette.text, palette.window) >= 7.0,
                "body text contrast is too low in {}",
                if is_dark { "dark" } else { "light" }
            );
            assert!(
                contrast(palette.text_muted, palette.window) >= 4.5,
                "muted text contrast is too low in {}",
                if is_dark { "dark" } else { "light" }
            );
            assert!(
                contrast(palette.accent, palette.window) >= 4.5,
                "accent contrast is too low in {}",
                if is_dark { "dark" } else { "light" }
            );
        }
    }

    #[test]
    fn a_label_on_the_accent_stays_readable() {
        for is_dark in [true, false] {
            let palette = palette_for(is_dark, Accent::Ember);
            assert!(contrast(palette.on_accent, palette.accent) >= 4.5);
        }
    }

    #[test]
    fn every_accent_gets_a_readable_label_on_top_of_it() {
        for accent in Accent::ALL {
            for is_dark in [true, false] {
                let palette = palette_for(is_dark, accent);
                assert!(
                    contrast(palette.on_accent, palette.accent) >= 4.5,
                    "{} is unreadable in {} mode",
                    accent.label(),
                    if is_dark { "dark" } else { "light" }
                );
            }
        }
    }

    #[test]
    fn the_default_accent_is_the_logo_orange() {
        assert_eq!(Accent::Ember.rgb(), [251, 86, 6]);
        assert_eq!(Accent::default(), Accent::Ember);
    }
}

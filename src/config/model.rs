use std::path::PathBuf;

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

use crate::roblox::gamesettings::{self, Change};

use super::migrate::CURRENT_VERSION;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Settings {
    pub version: u32,
    pub advanced_mode: bool,
    pub launch: LaunchSettings,
    pub game: GameSettings,
    pub mods: ModSettings,
    pub appearance: AppearanceSettings,
    pub advanced: AdvancedSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: CURRENT_VERSION,
            advanced_mode: false,
            launch: LaunchSettings::default(),
            game: GameSettings::default(),
            mods: ModSettings::default(),
            appearance: AppearanceSettings::default(),
            advanced: AdvancedSettings::default(),
        }
    }
}

impl Settings {
    pub fn validate(&mut self) -> Vec<String> {
        let mut notes = Vec::new();
        notes.extend(self.launch.validate());
        notes.extend(self.game.validate());
        notes.extend(self.appearance.validate());
        notes.extend(self.advanced.validate());
        self.version = CURRENT_VERSION;
        notes
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct LaunchSettings {
    pub startup_target: StartupTarget,
    pub confirm_before_launch: bool,
    pub warn_when_already_running: bool,
    pub update_roblox_on_launch: bool,
    pub launch_timeout_secs: u64,
    pub quick_targets: Vec<QuickTarget>,
    pub integrations: Vec<Integration>,
}

impl Default for LaunchSettings {
    fn default() -> Self {
        Self {
            startup_target: StartupTarget::App,
            confirm_before_launch: false,
            warn_when_already_running: true,
            update_roblox_on_launch: true,
            launch_timeout_secs: 30,
            quick_targets: Vec::new(),
            integrations: Vec::new(),
        }
    }
}

impl LaunchSettings {
    fn validate(&mut self) -> Vec<String> {
        let mut notes = Vec::new();

        if self.launch_timeout_secs < 5 || self.launch_timeout_secs > 300 {
            notes.push(format!(
                "launch timeout of {}s is out of range, reset to 30s",
                self.launch_timeout_secs
            ));
            self.launch_timeout_secs = 30;
        }

        let before = self.quick_targets.len();
        self.quick_targets.retain(|target| target.place_id != 0);
        let mut seen = std::collections::HashSet::new();
        self.quick_targets
            .retain(|target| seen.insert(target.place_id));
        if self.quick_targets.len() != before {
            notes.push("removed invalid or duplicate quick launch entries".into());
        }
        if self.quick_targets.len() > 32 {
            self.quick_targets.truncate(32);
            notes.push("quick launch list trimmed to 32 entries".into());
        }

        for target in &mut self.quick_targets {
            if target.name.trim().is_empty() {
                target.name = format!("Place {}", target.place_id);
            }
            if target.name.chars().count() > 60 {
                target.name = target.name.chars().take(60).collect();
            }
        }

        let before = self.integrations.len();
        self.integrations
            .retain(|entry| !entry.program.as_os_str().is_empty());
        if self.integrations.len() != before {
            notes.push("removed programs that had no path".into());
        }
        if self.integrations.len() > Integration::MAX {
            self.integrations.truncate(Integration::MAX);
            notes.push(format!(
                "the list of programs was trimmed to {}",
                Integration::MAX
            ));
        }
        for entry in &mut self.integrations {
            if entry.name.chars().count() > 60 {
                entry.name = entry.name.chars().take(60).collect();
            }
            if entry.arguments.chars().count() > 512 {
                entry.arguments = entry.arguments.chars().take(512).collect();
                notes.push(format!(
                    "arguments for {} were trimmed",
                    entry.display_name()
                ));
            }
        }

        notes
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(default)]
pub struct QuickTarget {
    pub name: String,
    pub place_id: u64,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum StartupTarget {
    #[default]
    App,
    LastPlayed,
}

impl StartupTarget {
    pub const ALL: [StartupTarget; 2] = [StartupTarget::App, StartupTarget::LastPlayed];

    pub fn label(self) -> &'static str {
        match self {
            StartupTarget::App => "Roblox home",
            StartupTarget::LastPlayed => "Last quick launch entry",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            StartupTarget::App => "Opens the Roblox app to its own home screen.",
            StartupTarget::LastPlayed => {
                "Rejoins the most recent quick launch entry, falling back to the home screen."
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct GameSettings {
    pub manage: bool,
    pub lock: bool,
    pub framerate_cap: Option<u32>,
    pub quality: Option<u8>,
    pub performance_stats: Option<bool>,
    pub transparency: Option<f32>,
    pub reduced_motion: Option<bool>,
    pub text_size: Option<u8>,
    pub mouse_sensitivity: Option<f32>,
    pub vr: Option<bool>,
}

impl GameSettings {
    pub const MIN_FRAMERATE: u32 = 24;
    pub const MAX_FRAMERATE: u32 = 1000;
    pub const MAX_QUALITY: u8 = 10;
    pub const MIN_TEXT_SIZE: u8 = 1;
    pub const MAX_TEXT_SIZE: u8 = 4;
    pub const MIN_SENSITIVITY: f32 = 0.05;
    pub const MAX_SENSITIVITY: f32 = 4.0;

    pub fn changes(&self) -> Vec<Change> {
        let mut changes = Vec::new();
        if !self.manage {
            return changes;
        }

        if let Some(value) = self.framerate_cap {
            changes.push(Change::int(gamesettings::FRAMERATE_CAP, value.into()));
        }
        if let Some(value) = self.quality {
            changes.push(Change::token(gamesettings::QUALITY, value.into()));
        }
        if let Some(value) = self.performance_stats {
            changes.push(Change::flag(gamesettings::PERFORMANCE_STATS, value));
        }
        if let Some(value) = self.transparency {
            changes.push(Change::float(gamesettings::TRANSPARENCY, value));
        }
        if let Some(value) = self.reduced_motion {
            changes.push(Change::flag(gamesettings::REDUCED_MOTION, value));
        }
        if let Some(value) = self.text_size {
            changes.push(Change::token(gamesettings::TEXT_SIZE, value.into()));
        }
        if let Some(value) = self.mouse_sensitivity {
            changes.push(Change::float(gamesettings::MOUSE_SENSITIVITY, value));
            changes.push(Change::point(gamesettings::MOUSE_FIRST_PERSON, value));
            changes.push(Change::point(gamesettings::MOUSE_THIRD_PERSON, value));
        }
        if let Some(value) = self.vr {
            changes.push(Change::flag(gamesettings::VR_ENABLED, value));
        }

        changes
    }

    fn validate(&mut self) -> Vec<String> {
        let mut notes = Vec::new();

        if let Some(value) = self.framerate_cap {
            if !(Self::MIN_FRAMERATE..=Self::MAX_FRAMERATE).contains(&value) {
                notes.push(format!(
                    "frame rate limit of {value} is out of range, cleared"
                ));
                self.framerate_cap = None;
            }
        }
        if let Some(value) = self.quality {
            if value > Self::MAX_QUALITY {
                notes.push(format!(
                    "graphics quality of {value} is out of range, cleared"
                ));
                self.quality = None;
            }
        }
        if let Some(value) = self.text_size {
            if !(Self::MIN_TEXT_SIZE..=Self::MAX_TEXT_SIZE).contains(&value) {
                notes.push(format!("text size of {value} is out of range, cleared"));
                self.text_size = None;
            }
        }
        if let Some(value) = self.transparency {
            if !value.is_finite() || !(0.0..=1.0).contains(&value) {
                notes.push(format!("transparency of {value} is out of range, cleared"));
                self.transparency = None;
            }
        }
        if let Some(value) = self.mouse_sensitivity {
            if !value.is_finite()
                || !(Self::MIN_SENSITIVITY..=Self::MAX_SENSITIVITY).contains(&value)
            {
                notes.push(format!(
                    "mouse sensitivity of {value} is out of range, cleared"
                ));
                self.mouse_sensitivity = None;
            }
        }

        if !self.manage && self.lock {
            self.lock = false;
        }

        notes
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(default)]
pub struct ModSettings {
    pub enabled: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(default)]
pub struct Integration {
    pub name: String,
    pub program: PathBuf,
    pub arguments: String,
    pub enabled: bool,
}

impl Integration {
    pub const MAX: usize = 16;

    pub fn is_usable(&self) -> bool {
        self.enabled && !self.program.as_os_str().is_empty()
    }

    pub fn display_name(&self) -> String {
        if !self.name.trim().is_empty() {
            return self.name.trim().to_owned();
        }
        self.program
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Unnamed program".into())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AppearanceSettings {
    pub mode: ThemeMode,
    pub accent: Accent,
    pub density: Density,
    pub ui_scale: f32,
    pub animations: bool,
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            mode: ThemeMode::Auto,
            accent: Accent::Ember,
            density: Density::Comfortable,
            ui_scale: 1.0,
            animations: true,
        }
    }
}

impl AppearanceSettings {
    pub const MIN_SCALE: f32 = 0.8;
    pub const MAX_SCALE: f32 = 1.6;

    fn validate(&mut self) -> Vec<String> {
        let mut notes = Vec::new();
        if !self.ui_scale.is_finite()
            || self.ui_scale < Self::MIN_SCALE
            || self.ui_scale > Self::MAX_SCALE
        {
            notes.push(format!(
                "interface scale of {:.2} is out of range, reset to 1.00",
                self.ui_scale
            ));
            self.ui_scale = 1.0;
        }
        notes
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum ThemeMode {
    #[default]
    Auto,
    Light,
    Dark,
}

impl ThemeMode {
    pub const ALL: [ThemeMode; 3] = [ThemeMode::Auto, ThemeMode::Light, ThemeMode::Dark];

    pub fn label(self) -> &'static str {
        match self {
            ThemeMode::Auto => "Automatic",
            ThemeMode::Light => "Light",
            ThemeMode::Dark => "Dark",
        }
    }

    pub fn detail(self) -> &'static str {
        match self {
            ThemeMode::Auto => "Follows Windows",
            ThemeMode::Light => "Always light",
            ThemeMode::Dark => "Always dark",
        }
    }

    pub fn is_dark(self, system_is_dark: bool) -> bool {
        match self {
            ThemeMode::Auto => system_is_dark,
            ThemeMode::Light => false,
            ThemeMode::Dark => true,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum Accent {
    #[default]
    Ember,
    Aurora,
    Lagoon,
    Violet,
    Lime,
}

impl Accent {
    pub const ALL: [Accent; 5] = [
        Accent::Ember,
        Accent::Aurora,
        Accent::Lagoon,
        Accent::Violet,
        Accent::Lime,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Accent::Ember => "Ember",
            Accent::Aurora => "Aurora",
            Accent::Lagoon => "Lagoon",
            Accent::Violet => "Violet",
            Accent::Lime => "Lime",
        }
    }

    pub fn rgb(self) -> [u8; 3] {
        match self {
            Accent::Ember => [251, 86, 6],
            Accent::Aurora => [86, 204, 157],
            Accent::Lagoon => [77, 154, 255],
            Accent::Violet => [163, 129, 255],
            Accent::Lime => [176, 209, 71],
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum Density {
    #[default]
    Comfortable,
    Compact,
}

impl Density {
    pub const ALL: [Density; 2] = [Density::Comfortable, Density::Compact];

    pub fn label(self) -> &'static str {
        match self {
            Density::Comfortable => "Comfortable",
            Density::Compact => "Compact",
        }
    }

    pub fn scale(self) -> f32 {
        match self {
            Density::Comfortable => 1.0,
            Density::Compact => 0.84,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AdvancedSettings {
    pub channel: String,
    pub keep_downloads: bool,
    pub custom_install_root: Option<PathBuf>,
    pub pinned_version_folder: Option<String>,
    pub verify_before_launch: bool,
    pub apply_flag_profile: bool,
    pub keep_launch_logs: bool,
    pub extra_player_arguments: String,
}

impl Default for AdvancedSettings {
    fn default() -> Self {
        Self {
            channel: crate::roblox::deploy::DEFAULT_CHANNEL.to_owned(),
            keep_downloads: false,
            custom_install_root: None,
            pinned_version_folder: None,
            verify_before_launch: true,
            apply_flag_profile: true,
            keep_launch_logs: true,
            extra_player_arguments: String::new(),
        }
    }
}

impl AdvancedSettings {
    fn validate(&mut self) -> Vec<String> {
        let mut notes = Vec::new();

        if !crate::roblox::deploy::is_valid_channel(&self.channel) {
            notes.push(format!(
                "release channel {:?} is not usable, reset to LIVE",
                self.channel
            ));
            self.channel = crate::roblox::deploy::DEFAULT_CHANNEL.to_owned();
        }

        if let Some(root) = &self.custom_install_root {
            if root.as_os_str().is_empty() {
                self.custom_install_root = None;
                notes.push("cleared an empty custom install path".into());
            }
        }

        if let Some(folder) = &self.pinned_version_folder {
            let valid = !folder.is_empty()
                && folder
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
            if !valid {
                notes.push(format!("unpinned invalid version folder {folder:?}"));
                self.pinned_version_folder = None;
            }
        }

        if self.extra_player_arguments.chars().count() > 512 {
            self.extra_player_arguments = self.extra_player_arguments.chars().take(512).collect();
            notes.push("extra player arguments trimmed to 512 characters".into());
        }

        notes
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct State {
    pub launch_count: u64,
    pub last_launch: Option<LaunchRecord>,
    pub last_quick_target: Option<u64>,
    pub window: WindowState,
    pub seen_welcome: bool,
    pub seen_game_page: bool,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct WindowState {
    pub width: f32,
    pub height: f32,
    pub maximized: bool,
}

impl Default for WindowState {
    fn default() -> Self {
        Self {
            width: 1100.0,
            height: 720.0,
            maximized: false,
        }
    }
}

impl WindowState {
    pub const MIN_WIDTH: f32 = 940.0;
    pub const MIN_HEIGHT: f32 = 620.0;

    pub fn sanitised(&self) -> Self {
        Self {
            width: clamp_dimension(self.width, Self::MIN_WIDTH, 6000.0, 1100.0),
            height: clamp_dimension(self.height, Self::MIN_HEIGHT, 4000.0, 720.0),
            maximized: self.maximized,
        }
    }
}

fn clamp_dimension(value: f32, min: f32, max: f32, fallback: f32) -> f32 {
    if !value.is_finite() || value < min || value > max {
        fallback
    } else {
        value
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct LaunchRecord {
    pub at: DateTime<Local>,
    pub outcome: LaunchOutcome,
    pub target: String,
    pub version: Option<String>,
    pub detail: Option<String>,
}

impl Default for LaunchRecord {
    fn default() -> Self {
        Self {
            at: Local::now(),
            outcome: LaunchOutcome::Succeeded,
            target: String::new(),
            version: None,
            detail: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum LaunchOutcome {
    #[default]
    Succeeded,
    Failed,
    Cancelled,
}

impl LaunchOutcome {
    pub fn label(self) -> &'static str {
        match self {
            LaunchOutcome::Succeeded => "Succeeded",
            LaunchOutcome::Failed => "Failed",
            LaunchOutcome::Cancelled => "Cancelled",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_program_without_a_path_is_dropped_on_validation() {
        let mut launch = LaunchSettings {
            integrations: vec![
                Integration {
                    name: "Nothing".into(),
                    ..Integration::default()
                },
                Integration {
                    name: "Real".into(),
                    program: PathBuf::from("C:/thing.exe"),
                    enabled: true,
                    ..Integration::default()
                },
            ],
            ..LaunchSettings::default()
        };

        let notes = launch.validate();

        assert_eq!(launch.integrations.len(), 1);
        assert_eq!(launch.integrations[0].name, "Real");
        assert!(notes.iter().any(|note| note.contains("no path")));
    }

    #[test]
    fn a_program_is_only_started_when_it_is_on_and_has_a_path() {
        let off = Integration {
            program: PathBuf::from("C:/thing.exe"),
            enabled: false,
            ..Integration::default()
        };
        let pathless = Integration {
            enabled: true,
            ..Integration::default()
        };
        let good = Integration {
            program: PathBuf::from("C:/thing.exe"),
            enabled: true,
            ..Integration::default()
        };

        assert!(!off.is_usable());
        assert!(!pathless.is_usable());
        assert!(good.is_usable());
    }

    #[test]
    fn an_unnamed_program_falls_back_to_its_file_name() {
        let entry = Integration {
            program: PathBuf::from("C:/tools/OBS.exe"),
            ..Integration::default()
        };
        assert_eq!(entry.display_name(), "OBS.exe");

        let named = Integration {
            name: "  Recorder  ".into(),
            ..entry.clone()
        };
        assert_eq!(named.display_name(), "Recorder");
    }

    #[test]
    fn nothing_is_written_while_the_page_is_switched_off() {
        let game = GameSettings {
            manage: false,
            framerate_cap: Some(240),
            ..GameSettings::default()
        };
        assert!(game.changes().is_empty());
    }

    #[test]
    fn only_the_values_that_are_managed_are_written() {
        let game = GameSettings {
            manage: true,
            framerate_cap: Some(240),
            performance_stats: Some(true),
            ..GameSettings::default()
        };

        let changes = game.changes();
        let names: Vec<&str> = changes.iter().map(|change| change.name).collect();

        assert_eq!(names, vec!["FramerateCap", "PerformanceStatsVisible"]);
        assert_eq!(changes[0].value, "240");
        assert_eq!(changes[1].value, "true");
    }

    #[test]
    fn mouse_sensitivity_covers_all_three_properties() {
        let game = GameSettings {
            manage: true,
            mouse_sensitivity: Some(0.5),
            ..GameSettings::default()
        };

        let names: Vec<&str> = game.changes().iter().map(|change| change.name).collect();
        assert_eq!(
            names,
            vec![
                "MouseSensitivity",
                "MouseSensitivityFirstPerson",
                "MouseSensitivityThirdPerson"
            ]
        );
    }

    #[test]
    fn values_out_of_range_are_cleared_rather_than_written() {
        let mut game = GameSettings {
            manage: true,
            framerate_cap: Some(5),
            quality: Some(40),
            transparency: Some(4.0),
            mouse_sensitivity: Some(f32::NAN),
            text_size: Some(9),
            ..GameSettings::default()
        };

        let notes = game.validate();

        assert_eq!(notes.len(), 5, "{notes:?}");
        assert!(game.changes().is_empty());
    }

    #[test]
    fn the_lock_cannot_stay_on_once_nothing_is_managed() {
        let mut game = GameSettings {
            manage: false,
            lock: true,
            ..GameSettings::default()
        };
        game.validate();
        assert!(!game.lock);
    }
}

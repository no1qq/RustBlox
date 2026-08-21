use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use crate::error::{Context, Error, Result};
use crate::util::fs;

use super::install::Installation;

const KNOWN_PREFIXES: [&str; 8] = [
    "FFlag", "DFFlag", "FInt", "DFInt", "FString", "DFString", "FLog", "DFLog",
];

#[derive(Clone, Debug, PartialEq)]
pub enum FlagValue {
    Bool(bool),
    Number(i64),
    Text(String),
}

impl FlagValue {
    pub fn as_json(&self) -> Value {
        match self {
            FlagValue::Bool(value) => Value::Bool(*value),
            FlagValue::Number(value) => Value::from(*value),
            FlagValue::Text(value) => Value::String(value.clone()),
        }
    }

    pub fn display(&self) -> String {
        match self {
            FlagValue::Bool(value) => value.to_string(),
            FlagValue::Number(value) => value.to_string(),
            FlagValue::Text(value) => value.clone(),
        }
    }

    pub fn from_input(raw: &str) -> Self {
        let trimmed = raw.trim();
        match trimmed.to_ascii_lowercase().as_str() {
            "true" => return FlagValue::Bool(true),
            "false" => return FlagValue::Bool(false),
            _ => {}
        }
        if let Ok(number) = trimmed.parse::<i64>() {
            return FlagValue::Number(number);
        }
        FlagValue::Text(trimmed.to_owned())
    }

    fn from_json(value: &Value) -> Self {
        match value {
            Value::Bool(value) => FlagValue::Bool(*value),
            Value::Number(number) => number
                .as_i64()
                .map(FlagValue::Number)
                .unwrap_or_else(|| FlagValue::Text(number.to_string())),
            Value::String(text) => FlagValue::from_input(text),
            other => FlagValue::Text(other.to_string()),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FlagEntry {
    pub key: String,
    pub value: FlagValue,
    pub enabled: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FlagProfile {
    pub entries: Vec<FlagEntry>,
}

impl FlagProfile {
    pub fn active(&self) -> impl Iterator<Item = &FlagEntry> {
        self.entries.iter().filter(|entry| entry.enabled)
    }

    pub fn active_count(&self) -> usize {
        self.active().count()
    }

    pub fn contains(&self, key: &str) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.key.eq_ignore_ascii_case(key))
    }

    pub fn set(&mut self, key: String, value: FlagValue) {
        match self
            .entries
            .iter_mut()
            .find(|entry| entry.key.eq_ignore_ascii_case(&key))
        {
            Some(entry) => {
                entry.value = value;
                entry.enabled = true;
            }
            None => self.entries.push(FlagEntry {
                key,
                value,
                enabled: true,
            }),
        }
    }

    pub fn remove(&mut self, key: &str) {
        self.entries
            .retain(|entry| !entry.key.eq_ignore_ascii_case(key));
    }

    pub fn sort(&mut self) {
        self.entries.sort_by_key(|entry| entry.key.to_lowercase());
    }

    pub fn to_json(&self) -> Value {
        let mut map = Map::new();
        for entry in self.active() {
            map.insert(entry.key.clone(), entry.value.as_json());
        }
        Value::Object(map)
    }

    pub fn to_pretty(&self) -> String {
        serde_json::to_string_pretty(&self.to_json()).unwrap_or_else(|_| "{}".into())
    }

    pub fn from_json(value: &Value) -> Result<Self> {
        let map = value
            .as_object()
            .ok_or_else(|| Error::invalid("flag documents must be a JSON object"))?;

        let mut entries: Vec<FlagEntry> = map
            .iter()
            .map(|(key, value)| FlagEntry {
                key: key.clone(),
                value: FlagValue::from_json(value),
                enabled: true,
            })
            .collect();
        entries.sort_by_key(|entry| entry.key.to_lowercase());

        Ok(Self { entries })
    }

    pub fn parse(text: &str) -> Result<Self> {
        if text.trim().is_empty() {
            return Ok(Self::default());
        }
        let value: Value = serde_json::from_str(text).map_err(|source| Error::Malformed {
            file: "flag document".into(),
            source,
        })?;
        Self::from_json(&value)
    }
}

pub struct Preset {
    pub name: &'static str,
    pub detail: &'static str,
    pub pairs: &'static [(&'static str, &'static str)],
}

pub const PRESETS: [Preset; 7] = [
    Preset {
        name: "Show FPS",
        detail: "Draws the frame counter Roblox uses internally in the corner of the client.",
        pairs: &[("FFlagDebugDisplayFPS", "true")],
    },
    Preset {
        name: "Unlock frame rate",
        detail: "Raises the frame cap the client schedules against to 240 instead of 60.",
        pairs: &[("DFIntTaskSchedulerTargetFps", "240")],
    },
    Preset {
        name: "Prefer Vulkan",
        detail: "Asks the client to render through Vulkan and turns the other two off.",
        pairs: &[
            ("FFlagDebugGraphicsPreferVulkan", "true"),
            ("FFlagDebugGraphicsPreferD3D11", "false"),
            ("FFlagDebugGraphicsPreferOpenGL", "false"),
        ],
    },
    Preset {
        name: "Prefer Direct3D 11",
        detail: "Asks the client to render through Direct3D 11 and turns the other two off.",
        pairs: &[
            ("FFlagDebugGraphicsPreferD3D11", "true"),
            ("FFlagDebugGraphicsPreferVulkan", "false"),
            ("FFlagDebugGraphicsPreferOpenGL", "false"),
        ],
    },
    Preset {
        name: "Prefer OpenGL",
        detail: "Asks the client to render through OpenGL and turns the other two off.",
        pairs: &[
            ("FFlagDebugGraphicsPreferOpenGL", "true"),
            ("FFlagDebugGraphicsPreferVulkan", "false"),
            ("FFlagDebugGraphicsPreferD3D11", "false"),
        ],
    },
    Preset {
        name: "Future lighting",
        detail: "Forces the newest lighting technology instead of whatever the place picked.",
        pairs: &[("FFlagDebugForceFutureIsBrightPhase3", "true")],
    },
    Preset {
        name: "No player shadows",
        detail: "Drops the shadow intensity to zero, which usually helps on weak hardware.",
        pairs: &[("FIntRenderShadowIntensity", "0")],
    },
];

pub fn preset_named(name: &str) -> Option<&'static Preset> {
    PRESETS.iter().find(|preset| preset.name == name)
}

impl FlagProfile {
    pub fn preset_applied(&self, preset: &Preset) -> bool {
        preset.pairs.iter().all(|(key, value)| {
            let wanted = FlagValue::from_input(value);
            self.entries.iter().any(|entry| {
                entry.enabled && entry.key.eq_ignore_ascii_case(key) && entry.value == wanted
            })
        })
    }

    pub fn apply_preset(&mut self, preset: &Preset) {
        for (key, value) in preset.pairs {
            self.set((*key).to_owned(), FlagValue::from_input(value));
        }
        self.sort();
    }

    pub fn remove_preset(&mut self, preset: &Preset) {
        for (key, _) in preset.pairs {
            self.remove(key);
        }
    }
}

pub fn validate_key(key: &str) -> Result<()> {
    let trimmed = key.trim();
    if trimmed.is_empty() {
        return Err(Error::invalid("a flag name is required"));
    }
    if trimmed.len() > 128 {
        return Err(Error::invalid("flag names are limited to 128 characters"));
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return Err(Error::invalid(
            "flag names may only contain letters, digits and underscores",
        ));
    }
    Ok(())
}

pub fn looks_unusual(key: &str) -> bool {
    !KNOWN_PREFIXES.iter().any(|prefix| key.starts_with(prefix))
}

pub fn profile_path(dir: &Path) -> PathBuf {
    dir.join("default.json")
}

pub fn load_profile(dir: &Path) -> Result<FlagProfile> {
    let path = profile_path(dir);
    match fs::read_to_string_if_exists(&path)? {
        Some(text) => FlagProfile::parse(&text),
        None => Ok(FlagProfile::default()),
    }
}

pub fn save_profile(dir: &Path, profile: &FlagProfile) -> Result<()> {
    let mut text = profile.to_pretty();
    text.push('\n');
    fs::write_atomic(&profile_path(dir), text.as_bytes())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplyReport {
    pub written: PathBuf,
    pub backup: Option<PathBuf>,
    pub count: usize,
}

pub fn apply_to(
    install: &Installation,
    profile: &FlagProfile,
    backup_dir: &Path,
) -> Result<ApplyReport> {
    let target = install.client_settings_file();
    fs::ensure_dir(&install.client_settings_dir())?;

    let backup = back_up_existing(&target, backup_dir)?;

    let mut text = profile.to_pretty();
    text.push('\n');
    fs::write_atomic(&target, text.as_bytes())?;

    Ok(ApplyReport {
        written: target,
        backup,
        count: profile.active_count(),
    })
}

pub fn read_applied(install: &Installation) -> Result<Option<FlagProfile>> {
    let path = install.client_settings_file();
    match fs::read_to_string_if_exists(&path)? {
        Some(text) => FlagProfile::parse(&text).map(Some),
        None => Ok(None),
    }
}

pub fn clear_applied(install: &Installation, backup_dir: &Path) -> Result<Option<PathBuf>> {
    let target = install.client_settings_file();
    if !target.is_file() {
        return Ok(None);
    }
    let backup = back_up_existing(&target, backup_dir)?;
    std::fs::remove_file(&target).ctx_path("could not remove", &target)?;
    Ok(backup)
}

fn back_up_existing(target: &Path, backup_dir: &Path) -> Result<Option<PathBuf>> {
    if !target.is_file() {
        return Ok(None);
    }
    let contents = std::fs::read(target).ctx_path("could not read", target)?;
    fs::ensure_dir(backup_dir)?;
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let path = backup_dir.join(format!("ClientAppSettings-{stamp}.json"));
    fs::write_atomic(&path, &contents)?;
    Ok(Some(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infers_value_types_from_text() {
        assert_eq!(FlagValue::from_input(" true "), FlagValue::Bool(true));
        assert_eq!(FlagValue::from_input("False"), FlagValue::Bool(false));
        assert_eq!(FlagValue::from_input("-12"), FlagValue::Number(-12));
        assert_eq!(
            FlagValue::from_input("Vulkan"),
            FlagValue::Text("Vulkan".into())
        );
    }

    #[test]
    fn parses_and_serialises_a_profile() {
        let profile =
            FlagProfile::parse(r#"{"FFlagA": true, "DFIntB": 7, "FStringC": "x"}"#).unwrap();
        assert_eq!(profile.entries.len(), 3);
        assert_eq!(profile.active_count(), 3);

        let json = profile.to_json();
        assert_eq!(json["FFlagA"], serde_json::json!(true));
        assert_eq!(json["DFIntB"], serde_json::json!(7));
        assert_eq!(json["FStringC"], serde_json::json!("x"));
    }

    #[test]
    fn empty_text_is_an_empty_profile() {
        assert_eq!(FlagProfile::parse("   ").unwrap(), FlagProfile::default());
    }

    #[test]
    fn rejects_documents_that_are_not_objects() {
        assert!(FlagProfile::parse("[1, 2]").is_err());
        assert!(FlagProfile::parse("{oops").is_err());
    }

    #[test]
    fn disabled_entries_are_not_written() {
        let mut profile = FlagProfile::default();
        profile.set("FFlagOne".into(), FlagValue::Bool(true));
        profile.set("FFlagTwo".into(), FlagValue::Bool(false));
        profile.entries[1].enabled = false;

        assert_eq!(profile.active_count(), 1);
        assert_eq!(profile.to_json().as_object().unwrap().len(), 1);
    }

    #[test]
    fn setting_an_existing_key_replaces_and_re_enables_it() {
        let mut profile = FlagProfile::default();
        profile.set("FFlagOne".into(), FlagValue::Number(1));
        profile.entries[0].enabled = false;
        profile.set("fflagone".into(), FlagValue::Number(2));

        assert_eq!(profile.entries.len(), 1);
        assert_eq!(profile.entries[0].value, FlagValue::Number(2));
        assert!(profile.entries[0].enabled);
    }

    #[test]
    fn validates_flag_names() {
        assert!(validate_key("FFlagDebugDisplayFPS").is_ok());
        assert!(validate_key("").is_err());
        assert!(validate_key("has space").is_err());
        assert!(validate_key("has-dash").is_err());
        assert!(validate_key(&"a".repeat(200)).is_err());
    }

    #[test]
    fn flags_the_unfamiliar_prefixes() {
        assert!(!looks_unusual("FFlagSomething"));
        assert!(!looks_unusual("DFIntSomething"));
        assert!(looks_unusual("MyOwnSetting"));
    }

    #[test]
    fn profiles_round_trip_through_disk() {
        let dir = tempfile::tempdir().unwrap();
        let mut profile = FlagProfile::default();
        profile.set("FFlagAlpha".into(), FlagValue::Bool(true));
        profile.set("DFIntBeta".into(), FlagValue::Number(42));

        save_profile(dir.path(), &profile).unwrap();
        let loaded = load_profile(dir.path()).unwrap();

        assert_eq!(loaded.to_json(), profile.to_json());
    }

    #[test]
    fn a_preset_reads_as_off_until_every_pair_matches() {
        let preset = preset_named("Prefer Vulkan").unwrap();
        let mut profile = FlagProfile::default();
        assert!(!profile.preset_applied(preset));

        profile.set(
            "FFlagDebugGraphicsPreferVulkan".into(),
            FlagValue::Bool(true),
        );
        assert!(!profile.preset_applied(preset));

        profile.apply_preset(preset);
        assert!(profile.preset_applied(preset));
    }

    #[test]
    fn the_rendering_presets_turn_each_other_off() {
        let mut profile = FlagProfile::default();
        profile.apply_preset(preset_named("Prefer Vulkan").unwrap());
        profile.apply_preset(preset_named("Prefer OpenGL").unwrap());

        assert!(profile.preset_applied(preset_named("Prefer OpenGL").unwrap()));
        assert!(!profile.preset_applied(preset_named("Prefer Vulkan").unwrap()));
    }

    #[test]
    fn removing_a_preset_takes_its_flags_with_it() {
        let preset = preset_named("Show FPS").unwrap();
        let mut profile = FlagProfile::default();
        profile.apply_preset(preset);
        profile.remove_preset(preset);

        assert!(profile.entries.is_empty());
        assert!(!profile.preset_applied(preset));
    }

    #[test]
    fn a_disabled_entry_does_not_count_as_a_preset_being_on() {
        let preset = preset_named("Show FPS").unwrap();
        let mut profile = FlagProfile::default();
        profile.apply_preset(preset);
        profile.entries[0].enabled = false;
        assert!(!profile.preset_applied(preset));
    }

    #[test]
    fn every_preset_uses_names_the_editor_would_accept() {
        for preset in &PRESETS {
            assert!(!preset.pairs.is_empty(), "{} has no flags", preset.name);
            for (key, _) in preset.pairs {
                assert!(validate_key(key).is_ok(), "{key} is not a usable flag name");
                assert!(!looks_unusual(key), "{key} has an unfamiliar prefix");
            }
        }
    }

    #[test]
    fn a_missing_profile_loads_as_empty() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(load_profile(dir.path()).unwrap(), FlagProfile::default());
    }
}

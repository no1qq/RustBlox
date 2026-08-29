use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use crate::error::{Error, Result};
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
            FlagValue::Number(value) => Value::Number((*value).into()),
            FlagValue::Text(value) => Value::String(value.clone()),
        }
    }

    pub fn display(&self) -> String {
        match self {
            FlagValue::Bool(true) => "true".into(),
            FlagValue::Bool(false) => "false".into(),
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

pub const BACKUPS_KEPT: usize = 10;

const DENIED_MARKER: &str = "Denied local configuration for: ";
const LOG_SCAN_BYTES: usize = 128 * 1024;

pub fn client_log_dir() -> Option<PathBuf> {
    super::log_dir()
}

pub fn denied_by_client(log_dir: &Path) -> Vec<String> {
    let Some(log) = newest_log(log_dir) else {
        return Vec::new();
    };
    denied_in(&head_of(&log))
}

fn newest_log(dir: &Path) -> Option<PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("log"))
        })
        .filter_map(|entry| {
            let modified = entry.metadata().ok()?.modified().ok()?;
            Some((modified, entry.path()))
        })
        .max_by_key(|(modified, _)| *modified)
        .map(|(_, path)| path)
}

fn head_of(path: &Path) -> String {
    use std::io::Read;

    let Ok(mut file) = std::fs::File::open(path) else {
        return String::new();
    };
    let mut buffer = Vec::new();
    if file
        .by_ref()
        .take(LOG_SCAN_BYTES as u64)
        .read_to_end(&mut buffer)
        .is_err()
    {
        return String::new();
    }
    String::from_utf8_lossy(&buffer).into_owned()
}

fn denied_in(log: &str) -> Vec<String> {
    let mut names: Vec<String> = log
        .lines()
        .filter_map(|line| line.split(DENIED_MARKER).nth(1))
        .map(|name| name.trim().to_owned())
        .filter(|name| !name.is_empty())
        .collect();
    names.sort();
    names.dedup();
    names
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplyReport {
    pub written: PathBuf,
    pub backup: Option<PathBuf>,
    pub count: usize,
    pub unchanged: bool,
}

pub fn apply_to(
    install: &Installation,
    profile: &FlagProfile,
    backup_dir: &Path,
) -> Result<ApplyReport> {
    let target = install.client_settings_file();
    let mut text = profile.to_pretty();
    text.push('\n');

    if fs::read_to_string_if_exists(&target)?.as_deref() == Some(text.as_str()) {
        return Ok(ApplyReport {
            written: target,
            backup: None,
            count: profile.active_count(),
            unchanged: true,
        });
    }

    fs::ensure_dir(&install.client_settings_dir())?;
    let backup = fs::back_up(&target, backup_dir, BACKUPS_KEPT)?;
    fs::write_atomic(&target, text.as_bytes())?;

    Ok(ApplyReport {
        written: target,
        backup,
        count: profile.active_count(),
        unchanged: false,
    })
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
    fn the_client_file_holds_typed_json_values() {
        let mut profile = FlagProfile::default();
        profile.set("FFlagOne".into(), FlagValue::Bool(true));
        profile.set("FFlagTwo".into(), FlagValue::Bool(false));
        profile.set("DFIntThree".into(), FlagValue::Number(240));
        profile.set("FStringFour".into(), FlagValue::Text("test".into()));

        let json = profile.to_json();
        assert_eq!(json["FFlagOne"], serde_json::json!(true));
        assert_eq!(json["FFlagTwo"], serde_json::json!(false));
        assert_eq!(json["DFIntThree"], serde_json::json!(240));
        assert_eq!(json["FStringFour"], serde_json::json!("test"));
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
    fn the_client_log_names_what_it_refused() {
        let log = concat!(
            "2026-08-21T04:06:06.533Z,0930,6,Warning [FLog::FlagFetchingStarterModule] Successfully loaded flags\n",
            "2026-08-21T04:06:06.536Z,0930,6,Warning [FLog::FlagFetchingStarterModule] Denied local configuration for: FFlagDebugDisplayFPS\n",
            "2026-08-21T04:06:06.536Z,0930,6,Warning [FLog::FlagFetchingStarterModule] Denied local configuration for: DFIntTaskSchedulerTargetFps\n",
            "2026-08-21T04:06:06.536Z,0930,6,Warning [FLog::FlagFetchingStarterModule] Denied local configuration for: FFlagDebugDisplayFPS\n",
        );

        assert_eq!(
            denied_in(log),
            vec![
                "DFIntTaskSchedulerTargetFps".to_string(),
                "FFlagDebugDisplayFPS".to_string()
            ]
        );
    }

    #[test]
    fn a_log_without_refusals_names_nothing() {
        assert!(denied_in("Successfully loaded flags\n").is_empty());
        assert!(denied_in("").is_empty());
    }

    #[test]
    fn a_missing_profile_loads_as_empty() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(load_profile(dir.path()).unwrap(), FlagProfile::default());
    }
}

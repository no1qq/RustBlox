use std::path::Path;

use serde::{de::DeserializeOwned, Serialize};

use crate::error::{Error, Result};
use crate::util::fs;

use super::migrate::{migrate, Migration};
use super::model::{Settings, State};
use super::paths::Paths;

#[derive(Debug, Default)]
pub struct Loaded<T> {
    pub value: T,
    pub notes: Vec<String>,
    pub recovered: bool,
}

#[derive(Clone, Debug)]
pub struct Store {
    paths: Paths,
}

impl Store {
    pub fn new(paths: Paths) -> Self {
        Self { paths }
    }

    pub fn paths(&self) -> &Paths {
        &self.paths
    }

    pub fn load_settings(&self) -> Loaded<Settings> {
        let path = self.paths.settings_file();
        let mut notes = Vec::new();
        let mut recovered = false;

        let text = match fs::read_to_string_if_exists(&path) {
            Ok(Some(text)) => text,
            Ok(None) => {
                let mut value = Settings::default();
                notes.extend(value.validate());
                return Loaded {
                    value,
                    notes,
                    recovered: false,
                };
            }
            Err(err) => {
                notes.push(err.to_string());
                let mut value = Settings::default();
                notes.extend(value.validate());
                return Loaded {
                    value,
                    notes,
                    recovered: true,
                };
            }
        };

        let mut value = match serde_json::from_str::<serde_json::Value>(&text) {
            Ok(value) => value,
            Err(err) => {
                notes.push(format!("settings.json could not be parsed: {err}"));
                if let Some(saved) = fs::quarantine(&path) {
                    notes.push(format!("damaged file kept at {}", saved.display()));
                }
                let mut value = Settings::default();
                notes.extend(value.validate());
                return Loaded {
                    value,
                    notes,
                    recovered: true,
                };
            }
        };

        let migration = migrate(&mut value);
        if let Some(note) = migration.note() {
            notes.push(note);
        }

        if matches!(migration, Migration::TooNew { .. }) {
            let mut value = Settings::default();
            notes.extend(value.validate());
            return Loaded {
                value,
                notes,
                recovered: true,
            };
        }

        let mut settings = match serde_json::from_value::<Settings>(value) {
            Ok(settings) => settings,
            Err(err) => {
                notes.push(format!("settings.json had unusable values: {err}"));
                if let Some(saved) = fs::quarantine(&path) {
                    notes.push(format!("damaged file kept at {}", saved.display()));
                }
                recovered = true;
                Settings::default()
            }
        };

        notes.extend(settings.validate());
        Loaded {
            value: settings,
            notes,
            recovered,
        }
    }

    pub fn save_settings(&self, settings: &Settings) -> Result<()> {
        write_json(&self.paths.settings_file(), settings)
    }

    pub fn load_state(&self) -> Loaded<State> {
        let path = self.paths.state_file();
        let mut notes = Vec::new();

        match read_json::<State>(&path) {
            Ok(Some(mut state)) => {
                state.window = state.window.sanitised();
                Loaded {
                    value: state,
                    notes,
                    recovered: false,
                }
            }
            Ok(None) => Loaded {
                value: State::default(),
                notes,
                recovered: false,
            },
            Err(err) => {
                notes.push(err.to_string());
                if let Some(saved) = fs::quarantine(&path) {
                    notes.push(format!("damaged file kept at {}", saved.display()));
                }
                Loaded {
                    value: State::default(),
                    notes,
                    recovered: true,
                }
            }
        }
    }

    pub fn save_state(&self, state: &State) -> Result<()> {
        write_json(&self.paths.state_file(), state)
    }
}

pub fn read_json<T: DeserializeOwned>(path: &Path) -> Result<Option<T>> {
    let Some(text) = fs::read_to_string_if_exists(path)? else {
        return Ok(None);
    };
    if text.trim().is_empty() {
        return Ok(None);
    }
    serde_json::from_str(&text)
        .map(Some)
        .map_err(|source| Error::Malformed {
            file: path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.display().to_string()),
            source,
        })
}

pub fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let mut text = serde_json::to_string_pretty(value).map_err(|source| Error::Malformed {
        file: path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string()),
        source,
    })?;
    text.push('\n');
    fs::write_atomic(path, text.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::model::{Accent, QuickTarget, ThemeMode, WindowState};

    fn store() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(Paths::rooted(dir.path()));
        (dir, store)
    }

    #[test]
    fn a_fresh_profile_loads_defaults_without_notes() {
        let (_dir, store) = store();
        let loaded = store.load_settings();
        assert_eq!(loaded.value, Settings::default());
        assert!(!loaded.recovered);
        assert!(loaded.notes.is_empty());
    }

    #[test]
    fn settings_survive_a_save_and_reload() {
        let (_dir, store) = store();

        let mut settings = Settings::default();
        settings.appearance.mode = ThemeMode::Light;
        settings.appearance.accent = Accent::Lagoon;
        settings.launch.launch_timeout_secs = 45;
        settings.launch.quick_targets.push(QuickTarget {
            name: "Test".into(),
            place_id: 1818,
        });

        store.save_settings(&settings).unwrap();
        let loaded = store.load_settings();

        assert_eq!(loaded.value, settings);
        assert!(!loaded.recovered);
    }

    #[test]
    fn a_file_from_the_named_theme_era_keeps_its_other_settings() {
        let (_dir, store) = store();
        let path = store.paths().settings_file();
        crate::util::fs::write_atomic(
            &path,
            br#"{"version": 1, "appearance": {"theme": "Daylight", "accent": "Lagoon"},
                 "launch": {"launch_timeout_secs": 45}}"#,
        )
        .unwrap();

        let loaded = store.load_settings();

        assert_eq!(loaded.value.appearance.mode, ThemeMode::Light);
        assert_eq!(loaded.value.appearance.accent, Accent::Lagoon);
        assert_eq!(loaded.value.launch.launch_timeout_secs, 45);
        assert!(!loaded.recovered);
    }

    #[test]
    fn a_damaged_file_is_quarantined_and_defaults_are_used() {
        let (_dir, store) = store();
        let path = store.paths().settings_file();
        crate::util::fs::write_atomic(&path, b"{ not json at all").unwrap();

        let loaded = store.load_settings();

        assert!(loaded.recovered);
        assert_eq!(loaded.value, Settings::default());
        assert!(loaded.notes.iter().any(|note| note.contains("kept at")));
        assert!(!path.exists());
    }

    #[test]
    fn unknown_keys_are_ignored_and_missing_ones_defaulted() {
        let (_dir, store) = store();
        let path = store.paths().settings_file();
        crate::util::fs::write_atomic(
            &path,
            br#"{"version": 1, "somethingNew": 5, "launch": {"launch_timeout_secs": 20}}"#,
        )
        .unwrap();

        let loaded = store.load_settings();

        assert!(!loaded.recovered);
        assert_eq!(loaded.value.launch.launch_timeout_secs, 20);
        assert_eq!(loaded.value.appearance, Settings::default().appearance);
    }

    #[test]
    fn out_of_range_values_are_repaired_with_a_note() {
        let (_dir, store) = store();
        let path = store.paths().settings_file();
        let body = format!(
            r#"{{"version": {}, "launch": {{"launch_timeout_secs": 9000}},
                 "appearance": {{"ui_scale": 12.0}}}}"#,
            crate::config::migrate::CURRENT_VERSION
        );
        crate::util::fs::write_atomic(&path, body.as_bytes()).unwrap();

        let loaded = store.load_settings();

        assert_eq!(loaded.value.launch.launch_timeout_secs, 30);
        assert_eq!(loaded.value.appearance.ui_scale, 1.0);
        assert_eq!(loaded.notes.len(), 2);
    }

    #[test]
    fn duplicate_and_empty_quick_targets_are_cleaned_up() {
        let (_dir, store) = store();
        let path = store.paths().settings_file();
        crate::util::fs::write_atomic(
            &path,
            br#"{"version": 1, "launch": {"quick_targets": [
                {"name": "A", "place_id": 1},
                {"name": "B", "place_id": 1},
                {"name": "", "place_id": 2},
                {"name": "C", "place_id": 0}
            ]}}"#,
        )
        .unwrap();

        let loaded = store.load_settings();
        let targets = &loaded.value.launch.quick_targets;

        assert_eq!(targets.len(), 2);
        assert_eq!(targets[0].place_id, 1);
        assert_eq!(targets[1].name, "Place 2");
    }

    #[test]
    fn a_newer_format_is_left_alone_and_defaults_are_used() {
        let (_dir, store) = store();
        let path = store.paths().settings_file();
        let original = br#"{"version": 9999, "launch": {"launch_timeout_secs": 77}}"#;
        crate::util::fs::write_atomic(&path, original).unwrap();

        let loaded = store.load_settings();

        assert!(loaded.recovered);
        assert_eq!(loaded.value, Settings::default());
        assert_eq!(std::fs::read(&path).unwrap(), original);
    }

    #[test]
    fn an_unversioned_file_is_migrated_forward() {
        let (_dir, store) = store();
        let path = store.paths().settings_file();
        crate::util::fs::write_atomic(&path, br#"{"launch": {"close_after_launch": true}}"#)
            .unwrap();

        let loaded = store.load_settings();

        assert_eq!(
            loaded.value.version,
            crate::config::migrate::CURRENT_VERSION
        );
        assert!(loaded.value.launch.close_after_launch);
        assert!(loaded
            .notes
            .iter()
            .any(|note| note.contains("upgraded from format version 0")));
    }

    #[test]
    fn state_round_trips_and_clamps_window_size() {
        let (_dir, store) = store();

        let mut state = State {
            launch_count: 12,
            last_quick_target: Some(1818),
            ..State::default()
        };
        state.window.width = 3.0;
        state.window.height = 4.0;

        store.save_state(&state).unwrap();
        let loaded = store.load_state();

        assert_eq!(loaded.value.launch_count, 12);
        assert_eq!(loaded.value.last_quick_target, Some(1818));
        assert_eq!(loaded.value.window, WindowState::default());
    }

    #[test]
    fn a_damaged_state_file_is_recovered() {
        let (_dir, store) = store();
        crate::util::fs::write_atomic(&store.paths().state_file(), b"nonsense").unwrap();

        let loaded = store.load_state();

        assert!(loaded.recovered);
        assert_eq!(loaded.value, State::default());
    }
}

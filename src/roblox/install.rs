use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::platform;

pub const PLAYER_EXE: &str = "RobloxPlayerBeta.exe";
pub const CRASH_HANDLER_EXE: &str = "RobloxCrashHandler.exe";
pub const APP_SETTINGS: &str = "AppSettings.xml";
pub const CLIENT_SETTINGS_DIR: &str = "ClientSettings";
pub const CLIENT_SETTINGS_FILE: &str = "ClientAppSettings.json";
pub const INCOMPLETE_SUFFIX: &str = ".incomplete";
pub const PREVIOUS_SUFFIX: &str = ".previous";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InstallSource {
    Ours,
    Custom,
}

impl InstallSource {
    pub fn rank(&self) -> u8 {
        match self {
            InstallSource::Ours => 0,
            InstallSource::Custom => 1,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            InstallSource::Ours => "Installed by RustBlox",
            InstallSource::Custom => "Folder you chose yourself",
        }
    }

    pub fn short(&self) -> &'static str {
        match self {
            InstallSource::Ours => "RustBlox",
            InstallSource::Custom => "Custom",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Installation {
    pub source: InstallSource,
    pub version_dir: PathBuf,
    pub player: PathBuf,
    pub folder_id: String,
    pub version: Option<String>,
    pub modified: Option<SystemTime>,
}

impl Installation {
    pub fn from_version_dir(dir: &Path, source: InstallSource) -> Option<Self> {
        let player = dir.join(PLAYER_EXE);
        if !player.is_file() {
            return None;
        }

        let folder_id = dir
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| dir.display().to_string());

        let version = platform::file_version(&player)
            .best()
            .map(normalise_version);

        let modified = std::fs::metadata(&player)
            .ok()
            .and_then(|meta| meta.modified().ok());

        Some(Self {
            source,
            version_dir: dir.to_path_buf(),
            player,
            folder_id,
            version,
            modified,
        })
    }

    pub fn version_parts(&self) -> Vec<u64> {
        self.version
            .as_deref()
            .map(parse_version)
            .unwrap_or_default()
    }

    pub fn display_version(&self) -> &str {
        self.version.as_deref().unwrap_or("unknown")
    }

    pub fn client_settings_dir(&self) -> PathBuf {
        self.version_dir.join(CLIENT_SETTINGS_DIR)
    }

    pub fn client_settings_file(&self) -> PathBuf {
        self.client_settings_dir().join(CLIENT_SETTINGS_FILE)
    }

    pub fn integrity(&self) -> Integrity {
        let mut problems = Vec::new();

        if !self.version_dir.is_dir() {
            problems.push(format!(
                "the version folder {} no longer exists",
                self.version_dir.display()
            ));
            return Integrity { problems };
        }
        if !self.player.is_file() {
            problems.push(format!("{PLAYER_EXE} is missing"));
        }
        if !self.version_dir.join(APP_SETTINGS).is_file() {
            problems.push(format!("{APP_SETTINGS} is missing"));
        }
        for folder in ["content", "ExtraContent", "shaders"] {
            if !self.version_dir.join(folder).is_dir() {
                problems.push(format!("the {folder} folder is missing"));
            }
        }

        Integrity { problems }
    }

    pub fn has_crash_handler(&self) -> bool {
        self.version_dir.join(CRASH_HANDLER_EXE).is_file()
    }

    pub fn size_on_disk(&self) -> Option<u64> {
        directory_size(&self.version_dir, 0)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Integrity {
    pub problems: Vec<String>,
}

impl Integrity {
    pub fn is_ok(&self) -> bool {
        self.problems.is_empty()
    }
}

pub use crate::util::version::{compare as compare_versions, parse as parse_version};

pub fn normalise_version(text: &str) -> String {
    let parts: Vec<String> = text
        .split(['.', ','])
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_owned)
        .collect();

    if parts.is_empty() {
        text.trim().to_owned()
    } else {
        parts.join(".")
    }
}

pub fn directory_size(dir: &Path, depth: u32) -> Option<u64> {
    if depth > 6 {
        return Some(0);
    }
    let entries = std::fs::read_dir(dir).ok()?;
    let mut total = 0u64;
    for entry in entries.flatten() {
        let Ok(meta) = entry.metadata() else { continue };
        if meta.is_file() {
            total = total.saturating_add(meta.len());
        } else if meta.is_dir() {
            total = total.saturating_add(directory_size(&entry.path(), depth + 1).unwrap_or(0));
        }
    }
    Some(total)
}

pub fn format_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_comma_versions_are_shown_with_dots() {
        assert_eq!(normalise_version("0, 735, 0, 7351131"), "0.735.0.7351131");
        assert_eq!(normalise_version("0.735.0.7351131"), "0.735.0.7351131");
        assert_eq!(normalise_version("  1, 2  "), "1.2");
        assert_eq!(normalise_version("WindowsPlayer"), "WindowsPlayer");
        assert_eq!(normalise_version(""), "");
    }

    #[test]
    fn parses_version_strings() {
        assert_eq!(parse_version("0.680.0.6800542"), vec![0, 680, 0, 6800542]);
        assert_eq!(parse_version("version-ce0bcd0f"), vec![0, 0]);
        assert!(parse_version("").is_empty());
    }

    #[test]
    fn compares_versions_by_component() {
        use std::cmp::Ordering;
        let older = parse_version("0.679.1.1");
        let newer = parse_version("0.680.0.0");
        assert_eq!(compare_versions(&older, &newer), Ordering::Less);
        assert_eq!(compare_versions(&newer, &older), Ordering::Greater);
        assert_eq!(
            compare_versions(&parse_version("1.2"), &parse_version("1.2.0")),
            Ordering::Equal
        );
    }

    #[test]
    fn formats_sizes() {
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(2048), "2.0 KB");
        assert_eq!(format_size(3 * 1024 * 1024), "3.0 MB");
    }

    #[test]
    fn reports_missing_files_for_an_absent_folder() {
        let install = Installation {
            source: InstallSource::Custom,
            version_dir: std::path::PathBuf::from("does-not-exist-anywhere"),
            player: std::path::PathBuf::from("does-not-exist-anywhere/RobloxPlayerBeta.exe"),
            folder_id: "does-not-exist-anywhere".into(),
            version: None,
            modified: None,
        };
        let integrity = install.integrity();
        assert!(!integrity.is_ok());
        assert_eq!(integrity.problems.len(), 1);
    }
}

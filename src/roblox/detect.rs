use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Local};

use super::install::{
    compare_versions, InstallSource, Installation, INCOMPLETE_SUFFIX, PREVIOUS_SUFFIX,
};

const VERSIONS_DIR: &str = "Versions";
const MAX_VERSION_ENTRIES: usize = 64;

#[derive(Clone, Debug)]
pub struct Detection {
    pub installations: Vec<Installation>,
    pub selected: Option<usize>,
    pub searched: Vec<PathBuf>,
    pub notes: Vec<String>,
    pub scanned_at: DateTime<Local>,
}

impl Default for Detection {
    fn default() -> Self {
        Self {
            installations: Vec::new(),
            selected: None,
            searched: Vec::new(),
            notes: Vec::new(),
            scanned_at: Local::now(),
        }
    }
}

impl Detection {
    pub fn active(&self) -> Option<&Installation> {
        self.selected
            .and_then(|index| self.installations.get(index))
    }

    pub fn select_folder(&mut self, folder_id: &str) -> bool {
        match self
            .installations
            .iter()
            .position(|install| install.folder_id == folder_id)
        {
            Some(index) => {
                self.selected = Some(index);
                true
            }
            None => false,
        }
    }
}

struct Candidate {
    root: PathBuf,
    source: InstallSource,
}

fn env_path(key: &str) -> Option<PathBuf> {
    std::env::var_os(key)
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
}

#[derive(Clone, Debug, Default)]
pub struct ScanOptions {
    pub managed_root: Option<PathBuf>,
    pub custom_root: Option<PathBuf>,
    pub pinned: Option<String>,
}

fn candidates(options: &ScanOptions) -> Vec<Candidate> {
    let mut list = Vec::new();

    if let Some(managed) = &options.managed_root {
        list.push(Candidate {
            root: managed.clone(),
            source: InstallSource::Ours,
        });
    }

    if let Some(custom) = &options.custom_root {
        list.push(Candidate {
            root: custom.clone(),
            source: InstallSource::Custom,
        });
    }

    if let Some(local) = env_path("LOCALAPPDATA") {
        list.push(Candidate {
            root: local.join("Roblox"),
            source: InstallSource::UserLocal,
        });
        list.push(Candidate {
            root: local.join("Programs").join("Roblox"),
            source: InstallSource::UserLocal,
        });
    }

    for key in ["ProgramFiles(x86)", "ProgramFiles", "ProgramW6432"] {
        if let Some(root) = env_path(key) {
            list.push(Candidate {
                root: root.join("Roblox"),
                source: InstallSource::MachineWide,
            });
        }
    }

    list
}

fn is_staging(path: &Path) -> bool {
    path.file_name()
        .map(|name| name.to_string_lossy().to_lowercase())
        .map(|name| name.ends_with(INCOMPLETE_SUFFIX) || name.ends_with(PREVIOUS_SUFFIX))
        .unwrap_or(false)
}

fn collect_from_root(root: &Path, source: &InstallSource, out: &mut Vec<Installation>) {
    if let Some(install) = Installation::from_version_dir(root, source.clone()) {
        out.push(install);
    }

    let versions = root.join(VERSIONS_DIR);
    let Ok(entries) = std::fs::read_dir(&versions) else {
        return;
    };

    let mut seen = 0usize;
    for entry in entries.flatten() {
        if seen >= MAX_VERSION_ENTRIES {
            break;
        }
        let path = entry.path();
        if !path.is_dir() || is_staging(&path) {
            continue;
        }
        seen += 1;
        if let Some(install) = Installation::from_version_dir(&path, source.clone()) {
            out.push(install);
        }
    }
}

fn preference(a: &Installation, b: &Installation) -> std::cmp::Ordering {
    a.source
        .rank()
        .cmp(&b.source.rank())
        .then_with(|| compare_versions(&b.version_parts(), &a.version_parts()))
        .then_with(|| b.modified.cmp(&a.modified))
        .then_with(|| a.folder_id.cmp(&b.folder_id))
}

pub fn scan(options: &ScanOptions) -> Detection {
    let mut installations: Vec<Installation> = Vec::new();
    let mut searched = Vec::new();
    let mut notes = Vec::new();
    let mut visited = BTreeSet::new();

    if let Some(custom) = &options.custom_root {
        if !custom.exists() {
            notes.push(format!(
                "the custom install path {} does not exist and was skipped",
                custom.display()
            ));
        }
    }

    let roots = candidates(options);

    for candidate in roots {
        let key = candidate.root.to_string_lossy().to_lowercase();
        if !visited.insert(key) {
            continue;
        }
        searched.push(candidate.root.clone());
        if !candidate.root.is_dir() {
            continue;
        }
        collect_from_root(&candidate.root, &candidate.source, &mut installations);
    }

    let mut unique = BTreeSet::new();
    installations.retain(|install| unique.insert(install.player.to_string_lossy().to_lowercase()));

    installations.sort_by(preference);

    let mut selected = if installations.is_empty() {
        None
    } else {
        Some(0)
    };

    if let Some(folder) = options.pinned.as_deref() {
        match installations
            .iter()
            .position(|install| install.folder_id == folder)
        {
            Some(index) => selected = Some(index),
            None => notes.push(format!(
                "the pinned version {folder} was not found, using the newest install instead"
            )),
        }
    }

    if installations.len() > 1 {
        if let Some(active) = selected.and_then(|index| installations.get(index)) {
            notes.push(format!(
                "{} installations were found, using {} from {}",
                installations.len(),
                active.display_version(),
                active.source.short()
            ));
        }
    }

    Detection {
        installations,
        selected,
        searched,
        notes,
        scanned_at: Local::now(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::roblox::install::PLAYER_EXE;

    #[test]
    fn scanning_a_missing_custom_root_is_reported_not_fatal() {
        let detection = scan(&ScanOptions {
            custom_root: Some(PathBuf::from(r"Z:\definitely\not\here")),
            ..ScanOptions::default()
        });
        assert!(detection
            .notes
            .iter()
            .any(|note| note.contains("does not exist")));
    }

    #[test]
    fn pinning_an_unknown_folder_falls_back_with_a_note() {
        let detection = scan(&ScanOptions {
            pinned: Some("version-does-not-exist".into()),
            ..ScanOptions::default()
        });
        if !detection.installations.is_empty() {
            assert!(detection
                .notes
                .iter()
                .any(|note| note.contains("was not found")));
        }
    }

    #[test]
    fn a_managed_root_is_searched_first() {
        let dir = tempfile::tempdir().unwrap();
        let detection = scan(&ScanOptions {
            managed_root: Some(dir.path().to_path_buf()),
            ..ScanOptions::default()
        });
        assert_eq!(detection.searched.first(), Some(&dir.path().to_path_buf()));
    }

    #[test]
    fn a_managed_root_finds_versions_inside_it() {
        let dir = tempfile::tempdir().unwrap();
        let version = dir.path().join(VERSIONS_DIR).join("version-abc123");
        std::fs::create_dir_all(&version).unwrap();
        std::fs::write(version.join(PLAYER_EXE), b"stub").unwrap();

        let detection = scan(&ScanOptions {
            managed_root: Some(dir.path().to_path_buf()),
            ..ScanOptions::default()
        });

        assert!(
            detection
                .installations
                .iter()
                .any(|install| install.version_dir == version
                    && install.source == InstallSource::Ours)
        );
    }

    fn stub(source: InstallSource, version: &str) -> Installation {
        Installation {
            source,
            version_dir: PathBuf::from(version),
            player: PathBuf::from(version).join(PLAYER_EXE),
            folder_id: version.into(),
            version: Some(version.into()),
            modified: None,
        }
    }

    #[test]
    fn our_copy_wins_over_a_newer_roblox_install() {
        let mut list = [
            stub(InstallSource::MachineWide, "0.999.0.0"),
            stub(InstallSource::UserLocal, "0.900.0.0"),
            stub(InstallSource::Ours, "0.100.0.0"),
            stub(InstallSource::Custom, "0.800.0.0"),
        ];
        list.sort_by(preference);
        let order: Vec<_> = list.iter().map(|install| install.source.short()).collect();
        assert_eq!(order, ["RustBlox", "Custom", "User", "Machine"]);
    }

    #[test]
    fn our_own_copies_are_ordered_newest_first() {
        let mut list = [
            stub(InstallSource::Ours, "0.680.0.0"),
            stub(InstallSource::Ours, "0.681.0.0"),
        ];
        list.sort_by(preference);
        assert_eq!(list[0].display_version(), "0.681.0.0");
    }

    #[test]
    fn no_third_party_launcher_folders_are_searched() {
        let detection = scan(&ScanOptions::default());
        for path in &detection.searched {
            let lowered = path.to_string_lossy().to_lowercase();
            for name in ["bloxstrap", "fishstrap", "voidstrap", "lunarstrap"] {
                assert!(!lowered.contains(name), "{} was searched", path.display());
            }
        }
    }

    #[test]
    fn a_half_extracted_folder_is_not_offered_as_an_install() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["version-abc.incomplete", "version-abc.previous"] {
            let version = dir.path().join(VERSIONS_DIR).join(name);
            std::fs::create_dir_all(&version).unwrap();
            std::fs::write(version.join(PLAYER_EXE), b"stub").unwrap();
        }

        let detection = scan(&ScanOptions {
            managed_root: Some(dir.path().to_path_buf()),
            ..ScanOptions::default()
        });

        assert!(!detection
            .installations
            .iter()
            .any(|install| install.folder_id.contains("version-abc")));
    }
}

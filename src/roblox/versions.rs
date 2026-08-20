use std::path::{Path, PathBuf};

use crate::error::{Context, Error, Result};

use super::install::{format_size, Installation, INCOMPLETE_SUFFIX, PREVIOUS_SUFFIX};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Sweep {
    pub removed: Vec<String>,
    pub reclaimed: u64,
    pub problems: Vec<String>,
}

impl Sweep {
    pub fn absorb(&mut self, other: Sweep) {
        self.removed.extend(other.removed);
        self.reclaimed = self.reclaimed.saturating_add(other.reclaimed);
        self.problems.extend(other.problems);
    }

    pub fn summary(&self) -> String {
        match self.removed.len() {
            0 => "nothing to clean up".into(),
            1 => format!("removed 1 folder, {} freed", format_size(self.reclaimed)),
            count => format!(
                "removed {count} folders, {} freed",
                format_size(self.reclaimed)
            ),
        }
    }
}

fn folder_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

fn is_leftover(name: &str) -> bool {
    let lowered = name.to_lowercase();
    lowered.ends_with(INCOMPLETE_SUFFIX) || lowered.ends_with(PREVIOUS_SUFFIX)
}

fn directory_size(path: &Path) -> u64 {
    super::install::directory_size(path, 0).unwrap_or(0)
}

fn sweep(root: &Path, keep: &[String]) -> Sweep {
    let mut result = Sweep::default();

    let Ok(entries) = std::fs::read_dir(root) else {
        return result;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let name = folder_name(&path);
        if keep.iter().any(|kept| kept.eq_ignore_ascii_case(&name)) && !is_leftover(&name) {
            continue;
        }

        let size = directory_size(&path);
        match std::fs::remove_dir_all(&path) {
            Ok(()) => {
                result.removed.push(name);
                result.reclaimed = result.reclaimed.saturating_add(size);
            }
            Err(err) => result
                .problems
                .push(format!("{name} could not be removed: {err}")),
        }
    }

    result.removed.sort();
    result
}

pub fn prune_versions(versions_root: &Path, keep: &[String]) -> Sweep {
    sweep(versions_root, keep)
}

pub fn tidy_downloads(downloads_root: &Path, keep: &[String]) -> Sweep {
    sweep(downloads_root, keep)
}

pub fn remove_version(versions_root: &Path, dir: &Path) -> Result<u64> {
    let root = versions_root
        .canonicalize()
        .unwrap_or_else(|_| versions_root.to_path_buf());
    let target = dir.canonicalize().ctx_path("could not resolve", dir)?;

    if !target.starts_with(&root) || target == root {
        return Err(Error::invalid(format!(
            "{} is not a version folder RustBlox manages",
            dir.display()
        )));
    }

    let size = directory_size(&target);
    std::fs::remove_dir_all(&target).ctx_path("could not remove", &target)?;
    Ok(size)
}

pub fn update_available(installed: &[String], latest_folder: &str) -> bool {
    !installed
        .iter()
        .any(|folder| folder.eq_ignore_ascii_case(latest_folder))
}

pub fn managed_folders(installations: &[Installation], versions_root: &Path) -> Vec<String> {
    installations
        .iter()
        .filter(|install| install.version_dir.starts_with(versions_root))
        .map(|install| install.folder_id.clone())
        .collect()
}

pub fn version_dir(versions_root: &Path, folder: &str) -> PathBuf {
    versions_root.join(folder)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(root: &Path, name: &str, bytes: usize) {
        let dir = root.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("payload.bin"), vec![7u8; bytes]).unwrap();
    }

    #[test]
    fn pruning_keeps_what_it_is_told_to_keep() {
        let dir = tempfile::tempdir().unwrap();
        version(dir.path(), "version-new", 32);
        version(dir.path(), "version-old", 64);
        version(dir.path(), "version-pinned", 16);

        let sweep = prune_versions(dir.path(), &["version-new".into(), "version-pinned".into()]);

        assert_eq!(sweep.removed, vec!["version-old".to_string()]);
        assert_eq!(sweep.reclaimed, 64);
        assert!(sweep.problems.is_empty());
        assert!(dir.path().join("version-new").is_dir());
        assert!(dir.path().join("version-pinned").is_dir());
        assert!(!dir.path().join("version-old").exists());
    }

    #[test]
    fn pruning_always_removes_staging_leftovers() {
        let dir = tempfile::tempdir().unwrap();
        version(dir.path(), "version-new", 8);
        version(dir.path(), "version-new.incomplete", 8);
        version(dir.path(), "version-new.previous", 8);

        let sweep = prune_versions(dir.path(), &["version-new".into()]);

        assert_eq!(
            sweep.removed,
            vec![
                "version-new.incomplete".to_string(),
                "version-new.previous".to_string()
            ]
        );
        assert!(dir.path().join("version-new").is_dir());
    }

    #[test]
    fn pruning_a_missing_root_is_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let sweep = prune_versions(&dir.path().join("nope"), &[]);
        assert!(sweep.removed.is_empty());
        assert!(sweep.problems.is_empty());
    }

    #[test]
    fn tidying_downloads_clears_everything_when_nothing_is_kept() {
        let dir = tempfile::tempdir().unwrap();
        version(dir.path(), "version-a", 10);
        version(dir.path(), "version-b", 20);

        let sweep = tidy_downloads(dir.path(), &[]);

        assert_eq!(sweep.removed.len(), 2);
        assert_eq!(sweep.reclaimed, 30);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[test]
    fn removing_a_version_refuses_a_folder_outside_the_root() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("Versions");
        std::fs::create_dir_all(&root).unwrap();
        let outside = dir.path().join("elsewhere");
        std::fs::create_dir_all(&outside).unwrap();

        assert!(remove_version(&root, &outside).is_err());
        assert!(outside.is_dir());
    }

    #[test]
    fn removing_a_version_refuses_the_root_itself() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("Versions");
        std::fs::create_dir_all(&root).unwrap();

        assert!(remove_version(&root, &root).is_err());
        assert!(root.is_dir());
    }

    #[test]
    fn removing_a_version_reports_what_it_freed() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("Versions");
        std::fs::create_dir_all(&root).unwrap();
        version(&root, "version-a", 128);

        let freed = remove_version(&root, &root.join("version-a")).unwrap();

        assert_eq!(freed, 128);
        assert!(!root.join("version-a").exists());
    }

    #[test]
    fn an_update_is_available_when_the_latest_folder_is_missing() {
        let installed = vec!["version-old".to_string()];
        assert!(update_available(&installed, "version-new"));
        assert!(!update_available(&installed, "VERSION-OLD"));
        assert!(update_available(&[], "version-new"));
    }

    fn install_at(dir: &Path) -> Installation {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join(super::super::install::PLAYER_EXE), b"stub").unwrap();
        Installation::from_version_dir(dir, super::super::install::InstallSource::Ours).unwrap()
    }

    #[test]
    fn only_folders_inside_our_root_count_as_managed() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("Versions");
        let mine = install_at(&root.join("version-mine"));
        let theirs = install_at(&dir.path().join("Roblox").join("version-theirs"));

        let folders = managed_folders(&[mine, theirs], &root);

        assert_eq!(folders, vec!["version-mine".to_string()]);
    }

    #[test]
    fn a_sweep_absorbs_another() {
        let mut first = Sweep {
            removed: vec!["a".into()],
            reclaimed: 10,
            problems: vec!["x".into()],
        };
        first.absorb(Sweep {
            removed: vec!["b".into()],
            reclaimed: 5,
            problems: vec!["y".into()],
        });

        assert_eq!(first.removed, vec!["a".to_string(), "b".to_string()]);
        assert_eq!(first.reclaimed, 15);
        assert_eq!(first.problems.len(), 2);
        assert!(first.summary().contains("2 folders"));
    }
}

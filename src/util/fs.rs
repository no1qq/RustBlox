use std::io::Write;
use std::path::{Path, PathBuf};

use crate::error::{Context, Result};

pub fn ensure_dir(dir: &Path) -> Result<()> {
    if dir.is_dir() {
        return Ok(());
    }
    std::fs::create_dir_all(dir).ctx_path("could not create", dir)
}

pub fn write_atomic(path: &Path, contents: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        ensure_dir(parent)?;
    }

    let temp = path.with_extension("tmp");
    {
        let mut file = std::fs::File::create(&temp).ctx_path("could not create", &temp)?;
        file.write_all(contents)
            .ctx_path("could not write to", &temp)?;
        file.sync_all().ctx_path("could not flush", &temp)?;
    }

    match std::fs::rename(&temp, path) {
        Ok(()) => Ok(()),
        Err(_) => {
            std::fs::copy(&temp, path).ctx_path("could not replace", path)?;
            let _ = std::fs::remove_file(&temp);
            Ok(())
        }
    }
}

pub fn quarantine(path: &Path) -> Option<std::path::PathBuf> {
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let name = path.file_name()?.to_string_lossy().into_owned();
    let target = path.with_file_name(format!("{name}.{stamp}.bad"));
    std::fs::rename(path, &target).ok()?;
    Some(target)
}

pub fn is_read_only(path: &Path) -> bool {
    std::fs::metadata(path)
        .map(|meta| meta.permissions().readonly())
        .unwrap_or(false)
}

pub fn set_read_only(path: &Path, value: bool) -> Result<()> {
    let mut permissions = std::fs::metadata(path)
        .ctx_path("could not read", path)?
        .permissions();
    if permissions.readonly() == value {
        return Ok(());
    }
    permissions.set_readonly(value);
    std::fs::set_permissions(path, permissions).ctx_path("could not change permissions on", path)
}

pub fn back_up(target: &Path, backup_dir: &Path, keep: usize) -> Result<Option<PathBuf>> {
    if !target.is_file() {
        return Ok(None);
    }
    let Some(stem) = target.file_stem().and_then(|stem| stem.to_str()) else {
        return Ok(None);
    };
    let extension = target
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("bak");

    let contents = std::fs::read(target).ctx_path("could not read", target)?;
    ensure_dir(backup_dir)?;
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let path = backup_dir.join(format!("{stem}-{stamp}.{extension}"));
    write_atomic(&path, &contents)?;
    prune_backups(backup_dir, stem, extension, keep);
    Ok(Some(path))
}

fn prune_backups(backup_dir: &Path, stem: &str, extension: &str, keep: usize) {
    let Ok(entries) = std::fs::read_dir(backup_dir) else {
        return;
    };

    let prefix = format!("{stem}-");
    let suffix = format!(".{extension}");
    let mut found: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with(&prefix) && name.ends_with(&suffix))
        })
        .collect();

    if found.len() <= keep {
        return;
    }

    found.sort();
    for path in found.iter().take(found.len() - keep) {
        let _ = std::fs::remove_file(path);
    }
}

pub fn read_to_string_if_exists(path: &Path) -> Result<Option<String>> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(crate::error::Error::io(
            format!("could not read {}", path.display()),
            err,
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writing_creates_parent_directories() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a").join("b").join("file.json");

        write_atomic(&path, b"hello").unwrap();

        assert_eq!(std::fs::read(&path).unwrap(), b"hello");
    }

    #[test]
    fn writing_replaces_existing_content_without_leaving_a_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("file.json");

        write_atomic(&path, b"first").unwrap();
        write_atomic(&path, b"second").unwrap();

        assert_eq!(std::fs::read(&path).unwrap(), b"second");
        assert!(!path.with_extension("tmp").exists());
    }

    #[test]
    fn reading_a_missing_file_is_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("missing.json");
        assert_eq!(read_to_string_if_exists(&path).unwrap(), None);
    }

    #[test]
    fn quarantine_moves_the_file_aside() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        write_atomic(&path, b"broken").unwrap();

        let saved = quarantine(&path).unwrap();

        assert!(!path.exists());
        assert!(saved.exists());
        assert_eq!(std::fs::read(&saved).unwrap(), b"broken");
    }

    #[test]
    fn a_backup_keeps_the_name_and_the_extension() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("ClientAppSettings.json");
        write_atomic(&target, b"{}").unwrap();

        let backup = back_up(&target, &dir.path().join("backups"), 10)
            .unwrap()
            .unwrap();
        let name = backup.file_name().unwrap().to_string_lossy().into_owned();

        assert!(name.starts_with("ClientAppSettings-"), "{name}");
        assert!(name.ends_with(".json"), "{name}");
        assert_eq!(std::fs::read(&backup).unwrap(), b"{}");
    }

    #[test]
    fn backing_up_a_missing_file_does_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("nope.json");
        assert_eq!(back_up(&missing, dir.path(), 10).unwrap(), None);
    }

    #[test]
    fn only_the_newest_backups_are_kept() {
        let dir = tempfile::tempdir().unwrap();
        let backups = dir.path().join("backups");
        ensure_dir(&backups).unwrap();
        for index in 0..5 {
            let name = format!("Settings-2026010{index}-000000.json");
            write_atomic(&backups.join(name), b"old").unwrap();
        }
        write_atomic(&backups.join("Other-20260101-000000.json"), b"keep").unwrap();

        let target = dir.path().join("Settings.json");
        write_atomic(&target, b"new").unwrap();
        back_up(&target, &backups, 2).unwrap();

        let kept: Vec<String> = std::fs::read_dir(&backups)
            .unwrap()
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.starts_with("Settings-"))
            .collect();

        assert_eq!(kept.len(), 2, "{kept:?}");
        assert!(backups.join("Other-20260101-000000.json").is_file());
    }

    #[test]
    fn the_read_only_attribute_can_be_set_and_cleared() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("locked.xml");
        write_atomic(&path, b"<roblox/>").unwrap();
        assert!(!is_read_only(&path));

        set_read_only(&path, true).unwrap();
        assert!(is_read_only(&path));

        set_read_only(&path, false).unwrap();
        assert!(!is_read_only(&path));
        write_atomic(&path, b"<roblox />").unwrap();
    }

    #[test]
    fn quarantine_of_a_missing_file_returns_nothing() {
        let dir = tempfile::tempdir().unwrap();
        assert!(quarantine(&dir.path().join("nope.json")).is_none());
    }
}

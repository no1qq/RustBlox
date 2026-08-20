use std::io::Write;
use std::path::Path;

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
    fn quarantine_of_a_missing_file_returns_nothing() {
        let dir = tempfile::tempdir().unwrap();
        assert!(quarantine(&dir.path().join("nope.json")).is_none());
    }
}

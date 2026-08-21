use std::path::{Path, PathBuf};

use directories::ProjectDirs;

use crate::error::{Error, Result};

#[derive(Clone, Debug)]
pub struct Paths {
    data: PathBuf,
    config: PathBuf,
}

impl Paths {
    pub fn discover() -> Result<Self> {
        if let Some(root) = std::env::var_os("RUSTBLOX_HOME").map(PathBuf::from) {
            if !root.as_os_str().is_empty() {
                return Ok(Self::rooted(root));
            }
        }

        let dirs = ProjectDirs::from("", "", "RustBlox").ok_or(Error::NoDataDir)?;
        Ok(Self {
            data: dirs.data_local_dir().to_path_buf(),
            config: dirs.config_dir().to_path_buf(),
        })
    }

    pub fn rooted(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        Self {
            config: root.join("config"),
            data: root,
        }
    }

    pub fn config_dir(&self) -> &Path {
        &self.config
    }

    pub fn data_dir(&self) -> &Path {
        &self.data
    }

    pub fn settings_file(&self) -> PathBuf {
        self.config.join("settings.json")
    }

    pub fn state_file(&self) -> PathBuf {
        self.data.join("state.json")
    }

    pub fn flag_profiles_dir(&self) -> PathBuf {
        self.data.join("flag-profiles")
    }

    pub fn log_file(&self) -> PathBuf {
        self.data.join("logs").join("rustblox.log")
    }

    pub fn backup_dir(&self) -> PathBuf {
        self.data.join("backups")
    }

    pub fn mods_dir(&self) -> PathBuf {
        self.data.join("mods")
    }

    pub fn mod_originals_dir(&self) -> PathBuf {
        self.data.join("mod-originals")
    }

    pub fn versions_dir(&self) -> PathBuf {
        self.data.join("Versions")
    }

    pub fn downloads_dir(&self) -> PathBuf {
        self.data.join("Downloads")
    }
}

use std::path::Path;
use std::process::Child;

use crate::error::{Error, Result};

use super::{FileVersion, ProcessInfo};

pub fn find_processes(_names: &[&str]) -> Vec<ProcessInfo> {
    Vec::new()
}

pub fn file_version(_path: &Path) -> FileVersion {
    FileVersion::default()
}

pub fn open_path(_path: &Path) -> Result<()> {
    Err(Error::UnsupportedPlatform)
}

pub fn open_url(_url: &str) -> Result<()> {
    Err(Error::UnsupportedPlatform)
}

pub fn spawn_detached(_program: &Path, _args: &[String], _cwd: Option<&Path>) -> Result<Child> {
    Err(Error::UnsupportedPlatform)
}

pub mod protocol {
    use std::path::Path;

    use crate::error::{Error, Result};
    use crate::platform::SchemeRegistration;

    pub fn inspect(_scheme: &str) -> Result<SchemeRegistration> {
        Err(Error::UnsupportedPlatform)
    }

    pub fn register(_scheme: &str, _exe: &Path) -> Result<()> {
        Err(Error::UnsupportedPlatform)
    }

    pub fn restore(_scheme: &str) -> Result<()> {
        Err(Error::UnsupportedPlatform)
    }
}

pub mod shortcut {
    use std::path::{Path, PathBuf};

    use crate::error::{Error, Result};

    pub struct Shortcut<'a> {
        pub target: &'a Path,
        pub arguments: &'a str,
        pub description: &'a str,
        pub working_dir: Option<&'a Path>,
        pub icon: Option<(&'a Path, i32)>,
    }

    pub fn create(_link: &Path, _shortcut: &Shortcut<'_>) -> Result<()> {
        Err(Error::UnsupportedPlatform)
    }

    pub fn remove(_link: &Path) -> Result<()> {
        Err(Error::UnsupportedPlatform)
    }

    pub fn desktop_dir() -> Option<PathBuf> {
        None
    }

    pub fn start_menu_dir() -> Option<PathBuf> {
        None
    }
}

pub fn attach_parent_console() {}

pub fn free_space(_path: &Path) -> Option<u64> {
    None
}

pub fn system_dark_mode() -> Option<bool> {
    None
}

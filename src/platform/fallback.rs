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

pub fn attach_parent_console() {}

pub fn free_space(_path: &Path) -> Option<u64> {
    None
}

use std::path::PathBuf;

#[cfg(not(windows))]
mod fallback;
#[cfg(windows)]
mod windows;

#[cfg(windows)]
pub use windows::{
    attach_parent_console, file_version, find_processes, free_space, open_path, open_url, protocol,
    spawn_detached,
};

#[cfg(not(windows))]
pub use fallback::{
    attach_parent_console, file_version, find_processes, free_space, open_path, open_url, protocol,
    spawn_detached,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub image: Option<PathBuf>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FileVersion {
    pub product: Option<String>,
    pub file: Option<String>,
}

impl FileVersion {
    pub fn best(&self) -> Option<&str> {
        self.file.as_deref().or(self.product.as_deref())
    }

    pub fn is_empty(&self) -> bool {
        self.product.is_none() && self.file.is_none()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SchemeRegistration {
    pub scheme: String,
    pub command: Option<String>,
    pub owner: SchemeOwner,
    pub saved_backup: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SchemeOwner {
    Unregistered,
    Ours,
    Roblox,
    Other,
}

impl SchemeOwner {
    pub fn label(self) -> &'static str {
        match self {
            SchemeOwner::Unregistered => "Not registered",
            SchemeOwner::Ours => "RustBlox",
            SchemeOwner::Roblox => "Roblox",
            SchemeOwner::Other => "Another application",
        }
    }
}

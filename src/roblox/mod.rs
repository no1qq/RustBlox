use std::path::PathBuf;

pub mod activity;
pub mod deploy;
pub mod detect;
pub mod flags;
pub mod gamesettings;
pub mod install;
pub mod installer;
pub mod launch;
pub mod mods;
pub mod process;
pub mod uri;
pub mod versions;

pub fn local_dir() -> Option<PathBuf> {
    let local = std::env::var_os("LOCALAPPDATA")?;
    if local.is_empty() {
        return None;
    }
    Some(PathBuf::from(local).join("Roblox"))
}

pub fn log_dir() -> Option<PathBuf> {
    Some(local_dir()?.join("logs"))
}

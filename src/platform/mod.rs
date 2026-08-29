use std::path::PathBuf;

#[cfg(not(windows))]
mod fallback;
#[cfg(windows)]
mod windows;

#[cfg(windows)]
#[allow(unused_imports)]
pub use windows::{
    attach_parent_console, clean_roblox_dir_proxies, close_roblox_singleton_mutex, file_version,
    find_processes, free_space, hide_tray_icon, open_path, open_url, protocol,
    run_thewatcher_service, shortcut, spawn_detached, spawn_elevated, system_dark_mode,
};

#[cfg(not(windows))]
#[allow(unused_imports)]
pub use fallback::{
    attach_parent_console, clean_roblox_dir_proxies, close_roblox_singleton_mutex, file_version,
    find_processes, free_space, hide_tray_icon, open_path, open_url, protocol,
    run_thewatcher_service, shortcut, spawn_detached, spawn_elevated, system_dark_mode,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum ThreatKind {
    KnownCheatProcess,
    InjectedModule,
    ScriptExecutorPipe,
    RogueInstallFile,
    ModuleStomping,
    HookTampering,
    UnauthorizedIpcServer,
}

impl ThreatKind {
    #[allow(dead_code)]
    pub fn label(self) -> &'static str {
        match self {
            ThreatKind::KnownCheatProcess => "External cheat or tool",
            ThreatKind::InjectedModule => "Injected internal module",
            ThreatKind::ScriptExecutorPipe => "Script executor pipe",
            ThreatKind::RogueInstallFile => "Rogue proxy file",
            ThreatKind::ModuleStomping => "Module stomping / in-memory patch",
            ThreatKind::HookTampering => "Hook tampering / detours",
            ThreatKind::UnauthorizedIpcServer => {
                "Unauthorized script executor IPC/WebSocket endpoint"
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SecurityThreat {
    pub kind: ThreatKind,
    pub name: String,
    pub detail: String,
    pub pid: Option<u32>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SecurityReport {
    pub threats: Vec<SecurityThreat>,
}

impl SecurityReport {
    #[allow(dead_code)]
    pub fn is_clean(&self) -> bool {
        self.threats.is_empty()
    }
}

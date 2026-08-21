use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::platform::shortcut::{self, Shortcut};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Desktop,
    StartMenu,
    LaunchRoblox,
    Settings,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Where {
    Desktop,
    StartMenu,
}

impl Kind {
    pub const ALL: [Kind; 4] = [
        Kind::Desktop,
        Kind::StartMenu,
        Kind::LaunchRoblox,
        Kind::Settings,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Kind::Desktop => "Desktop icon",
            Kind::StartMenu => "Start menu icon",
            Kind::LaunchRoblox => "Launch Roblox",
            Kind::Settings => "RustBlox settings",
        }
    }

    pub fn detail(self) -> &'static str {
        match self {
            Kind::Desktop => "Opens the small RustBlox menu from your desktop.",
            Kind::StartMenu => "Puts RustBlox in the Start menu, so search finds it.",
            Kind::LaunchRoblox => "Starts Roblox straight away, skipping the menu.",
            Kind::Settings => "Opens the settings window straight away.",
        }
    }

    pub fn file_name(self) -> &'static str {
        match self {
            Kind::Desktop | Kind::StartMenu => "RustBlox.lnk",
            Kind::LaunchRoblox => "Launch Roblox.lnk",
            Kind::Settings => "RustBlox Settings.lnk",
        }
    }

    pub fn arguments(self) -> &'static str {
        match self {
            Kind::Desktop | Kind::StartMenu => "",
            Kind::LaunchRoblox => "--launch",
            Kind::Settings => "--settings",
        }
    }

    pub fn placed(self) -> Where {
        match self {
            Kind::StartMenu => Where::StartMenu,
            _ => Where::Desktop,
        }
    }

    pub fn folder(self) -> Option<PathBuf> {
        match self.placed() {
            Where::Desktop => shortcut::desktop_dir(),
            Where::StartMenu => shortcut::start_menu_dir(),
        }
    }

    pub fn path(self) -> Option<PathBuf> {
        Some(self.folder()?.join(self.file_name()))
    }

    pub fn exists(self) -> bool {
        self.path().is_some_and(|path| path.is_file())
    }
}

pub fn create(kind: Kind, exe: &Path) -> Result<PathBuf> {
    if !exe.is_file() {
        return Err(Error::invalid(
            "RustBlox cannot find its own executable to point at",
        ));
    }
    let Some(path) = kind.path() else {
        return Err(Error::invalid("that folder could not be found on this PC"));
    };

    shortcut::create(
        &path,
        &Shortcut {
            target: exe,
            arguments: kind.arguments(),
            description: kind.detail(),
            working_dir: exe.parent(),
            icon: None,
        },
    )?;

    Ok(path)
}

pub fn remove(kind: Kind) -> Result<()> {
    let Some(path) = kind.path() else {
        return Ok(());
    };
    shortcut::remove(&path)
}

pub fn remove_all() -> Vec<PathBuf> {
    let mut gone = Vec::new();
    for kind in Kind::ALL {
        if !kind.exists() {
            continue;
        }
        if let Some(path) = kind.path() {
            if remove(kind).is_ok() {
                gone.push(path);
            }
        }
    }
    gone
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Present([bool; Kind::ALL.len()]);

impl Present {
    pub fn read() -> Self {
        let mut found = [false; Kind::ALL.len()];
        for (index, kind) in Kind::ALL.iter().enumerate() {
            found[index] = kind.exists();
        }
        Self(found)
    }

    pub fn has(&self, kind: Kind) -> bool {
        Kind::ALL
            .iter()
            .position(|other| *other == kind)
            .is_some_and(|index| self.0[index])
    }

    pub fn count(&self) -> usize {
        self.0.iter().filter(|present| **present).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_has_its_own_file_name_and_arguments() {
        for kind in Kind::ALL {
            assert!(kind.file_name().ends_with(".lnk"), "{:?}", kind);
            assert!(!kind.label().is_empty());
            assert!(!kind.detail().is_empty());
        }

        assert_eq!(Kind::LaunchRoblox.arguments(), "--launch");
        assert_eq!(Kind::Settings.arguments(), "--settings");
        assert!(Kind::Desktop.arguments().is_empty());
    }

    #[test]
    fn the_menu_shortcuts_share_a_name_but_not_a_folder() {
        assert_eq!(Kind::Desktop.file_name(), Kind::StartMenu.file_name());
        assert_ne!(Kind::Desktop.placed(), Kind::StartMenu.placed());
    }

    #[test]
    fn the_desktop_holds_everything_except_the_start_menu_entry() {
        for kind in Kind::ALL {
            let expected = if kind == Kind::StartMenu {
                Where::StartMenu
            } else {
                Where::Desktop
            };
            assert_eq!(kind.placed(), expected, "{kind:?}");
        }
    }

    #[test]
    fn a_missing_executable_is_refused_before_anything_is_written() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("RustBlox.exe");
        assert!(create(Kind::Settings, &missing).is_err());
    }

    #[test]
    fn nothing_is_reported_as_present_by_default() {
        let empty = Present::default();
        assert_eq!(empty.count(), 0);
        for kind in Kind::ALL {
            assert!(!empty.has(kind));
        }
    }
}

use std::path::Path;

use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
use winreg::RegKey;

use crate::error::{Error, Result};
use crate::platform::{SchemeOwner, SchemeRegistration};

const BACKUP_VALUE: &str = "RustBloxPreviousCommand";
const MARKER_VALUE: &str = "RustBloxManaged";

fn classes() -> Result<RegKey> {
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags("Software\\Classes", KEY_READ | KEY_WRITE)
        .map_err(|err| {
            Error::registry(format!("could not open the per-user class registry: {err}"))
        })
}

fn command_path(scheme: &str) -> String {
    format!("{scheme}\\shell\\open\\command")
}

pub fn inspect(scheme: &str) -> Result<SchemeRegistration> {
    let classes = classes()?;

    let command = classes
        .open_subkey(command_path(scheme))
        .ok()
        .and_then(|key| key.get_value::<String, _>("").ok())
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty());

    let scheme_key = classes.open_subkey(scheme).ok();
    let marked = scheme_key
        .as_ref()
        .and_then(|key| key.get_value::<u32, _>(MARKER_VALUE).ok())
        .map(|value| value == 1)
        .unwrap_or(false);
    let saved_backup = scheme_key
        .as_ref()
        .and_then(|key| key.get_value::<String, _>(BACKUP_VALUE).ok())
        .filter(|value| !value.is_empty());

    let owner = match command.as_deref() {
        None => SchemeOwner::Unregistered,
        Some(value) => {
            let lowered = value.to_ascii_lowercase();
            if marked || lowered.contains("rustblox.exe") {
                SchemeOwner::Ours
            } else if lowered.contains("robloxplayerbeta.exe")
                || lowered.contains("robloxplayerlauncher.exe")
                || lowered.contains("robloxplayerinstaller.exe")
            {
                SchemeOwner::Roblox
            } else {
                SchemeOwner::Other
            }
        }
    };

    Ok(SchemeRegistration {
        scheme: scheme.to_owned(),
        command,
        owner,
        saved_backup,
    })
}

pub fn register(scheme: &str, exe: &Path) -> Result<()> {
    if !exe.is_file() {
        return Err(Error::registry(format!(
            "{} is not a file that can be registered",
            exe.display()
        )));
    }

    let existing = inspect(scheme)?;
    let classes = classes()?;

    let (scheme_key, _) = classes
        .create_subkey(scheme)
        .map_err(|err| Error::registry(format!("could not create the {scheme} key: {err}")))?;

    scheme_key
        .set_value("", &format!("URL:{scheme} Protocol"))
        .map_err(|err| Error::registry(format!("could not describe {scheme}: {err}")))?;
    scheme_key
        .set_value("URL Protocol", &"")
        .map_err(|err| Error::registry(format!("could not flag {scheme} as a protocol: {err}")))?;

    if existing.owner != SchemeOwner::Ours {
        if let Some(previous) = existing.command.as_deref() {
            scheme_key
                .set_value(BACKUP_VALUE, &previous)
                .map_err(|err| {
                    Error::registry(format!("could not save the previous handler: {err}"))
                })?;
        }
    }

    scheme_key
        .set_value(MARKER_VALUE, &1u32)
        .map_err(|err| Error::registry(format!("could not mark {scheme} as managed: {err}")))?;

    let (icon_key, _) = classes
        .create_subkey(format!("{scheme}\\DefaultIcon"))
        .map_err(|err| Error::registry(format!("could not create the icon key: {err}")))?;
    icon_key
        .set_value("", &format!("{},0", exe.display()))
        .map_err(|err| Error::registry(format!("could not set the protocol icon: {err}")))?;

    let (command_key, _) = classes
        .create_subkey(command_path(scheme))
        .map_err(|err| Error::registry(format!("could not create the command key: {err}")))?;
    command_key
        .set_value("", &format!("\"{}\" --forward \"%1\"", exe.display()))
        .map_err(|err| Error::registry(format!("could not set the command: {err}")))?;

    Ok(())
}

pub fn restore(scheme: &str) -> Result<()> {
    let existing = inspect(scheme)?;
    if existing.owner != SchemeOwner::Ours {
        return Err(Error::registry(format!(
            "{scheme} is not currently handled by RustBlox"
        )));
    }

    let classes = classes()?;

    match existing.saved_backup {
        Some(previous) => {
            let (command_key, _) = classes.create_subkey(command_path(scheme)).map_err(|err| {
                Error::registry(format!("could not reopen the command key: {err}"))
            })?;
            command_key.set_value("", &previous).map_err(|err| {
                Error::registry(format!("could not restore the previous handler: {err}"))
            })?;

            if let Ok(scheme_key) = classes.open_subkey_with_flags(scheme, KEY_READ | KEY_WRITE) {
                let _ = scheme_key.delete_value(BACKUP_VALUE);
                let _ = scheme_key.delete_value(MARKER_VALUE);
            }
        }
        None => {
            classes.delete_subkey_all(scheme).map_err(|err| {
                Error::registry(format!("could not remove the {scheme} registration: {err}"))
            })?;
        }
    }

    Ok(())
}

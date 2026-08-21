use std::ffi::c_void;
use std::path::{Path, PathBuf};

use windows_sys::core::{GUID, HRESULT, PWSTR};
use windows_sys::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_INPROC_SERVER,
    COINIT_APARTMENTTHREADED,
};
use windows_sys::Win32::UI::Shell::{
    FOLDERID_Desktop, FOLDERID_Programs, SHGetKnownFolderPath, ShellLink,
};

use crate::error::{Error, Result};

use super::{from_wide_nul, wide};

const IID_SHELL_LINK: GUID = GUID::from_u128(0x000214f9_0000_0000_c000_000000000046);
const IID_PERSIST_FILE: GUID = GUID::from_u128(0x0000010b_0000_0000_c000_000000000046);

#[repr(C)]
struct ShellLinkVtbl {
    query_interface:
        unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    get_path: unsafe extern "system" fn(*mut c_void, PWSTR, i32, *mut c_void, u32) -> HRESULT,
    get_id_list: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
    set_id_list: unsafe extern "system" fn(*mut c_void, *const c_void) -> HRESULT,
    get_description: unsafe extern "system" fn(*mut c_void, PWSTR, i32) -> HRESULT,
    set_description: unsafe extern "system" fn(*mut c_void, *const u16) -> HRESULT,
    get_working_directory: unsafe extern "system" fn(*mut c_void, PWSTR, i32) -> HRESULT,
    set_working_directory: unsafe extern "system" fn(*mut c_void, *const u16) -> HRESULT,
    get_arguments: unsafe extern "system" fn(*mut c_void, PWSTR, i32) -> HRESULT,
    set_arguments: unsafe extern "system" fn(*mut c_void, *const u16) -> HRESULT,
    get_hotkey: unsafe extern "system" fn(*mut c_void, *mut u16) -> HRESULT,
    set_hotkey: unsafe extern "system" fn(*mut c_void, u16) -> HRESULT,
    get_show_cmd: unsafe extern "system" fn(*mut c_void, *mut i32) -> HRESULT,
    set_show_cmd: unsafe extern "system" fn(*mut c_void, i32) -> HRESULT,
    get_icon_location: unsafe extern "system" fn(*mut c_void, PWSTR, i32, *mut i32) -> HRESULT,
    set_icon_location: unsafe extern "system" fn(*mut c_void, *const u16, i32) -> HRESULT,
    set_relative_path: unsafe extern "system" fn(*mut c_void, *const u16, u32) -> HRESULT,
    resolve: unsafe extern "system" fn(*mut c_void, *mut c_void, u32) -> HRESULT,
    set_path: unsafe extern "system" fn(*mut c_void, *const u16) -> HRESULT,
}

#[repr(C)]
struct PersistFileVtbl {
    query_interface:
        unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    get_class_id: unsafe extern "system" fn(*mut c_void, *mut GUID) -> HRESULT,
    is_dirty: unsafe extern "system" fn(*mut c_void) -> HRESULT,
    load: unsafe extern "system" fn(*mut c_void, *const u16, u32) -> HRESULT,
    save: unsafe extern "system" fn(*mut c_void, *const u16, i32) -> HRESULT,
    save_completed: unsafe extern "system" fn(*mut c_void, *const u16) -> HRESULT,
    get_cur_file: unsafe extern "system" fn(*mut c_void, *mut PWSTR) -> HRESULT,
}

struct Com(bool);

impl Com {
    fn enter() -> Self {
        let outcome = unsafe { CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as u32) };
        Self(outcome >= 0)
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        if self.0 {
            unsafe { CoUninitialize() };
        }
    }
}

struct Interface(*mut c_void);

impl Interface {
    fn vtable<T>(&self) -> &T {
        unsafe { &**(self.0 as *const *const T) }
    }
}

impl Drop for Interface {
    fn drop(&mut self) {
        if !self.0.is_null() {
            let vtable: &ShellLinkVtbl = self.vtable();
            unsafe { (vtable.release)(self.0) };
        }
    }
}

fn checked(what: &str, outcome: HRESULT) -> Result<()> {
    if outcome >= 0 {
        Ok(())
    } else {
        Err(Error::invalid(format!(
            "{what} failed with code 0x{:08x}",
            outcome as u32
        )))
    }
}

pub struct Shortcut<'a> {
    pub target: &'a Path,
    pub arguments: &'a str,
    pub description: &'a str,
    pub working_dir: Option<&'a Path>,
    pub icon: Option<(&'a Path, i32)>,
}

pub fn create(link: &Path, shortcut: &Shortcut<'_>) -> Result<()> {
    if let Some(parent) = link.parent() {
        crate::util::fs::ensure_dir(parent)?;
    }

    let _com = Com::enter();
    let mut raw: *mut c_void = std::ptr::null_mut();
    checked("creating a shell link", unsafe {
        CoCreateInstance(
            &ShellLink,
            std::ptr::null_mut(),
            CLSCTX_INPROC_SERVER,
            &IID_SHELL_LINK,
            &mut raw,
        )
    })?;
    if raw.is_null() {
        return Err(Error::invalid("the shell returned no link object"));
    }

    let object = Interface(raw);
    let vtable: &ShellLinkVtbl = object.vtable();

    let target = wide(shortcut.target);
    checked("setting the shortcut target", unsafe {
        (vtable.set_path)(object.0, target.as_ptr())
    })?;

    if !shortcut.arguments.is_empty() {
        let arguments = wide(shortcut.arguments);
        checked("setting the shortcut arguments", unsafe {
            (vtable.set_arguments)(object.0, arguments.as_ptr())
        })?;
    }

    if !shortcut.description.is_empty() {
        let description = wide(shortcut.description);
        checked("setting the shortcut description", unsafe {
            (vtable.set_description)(object.0, description.as_ptr())
        })?;
    }

    let working = shortcut
        .working_dir
        .map(Path::to_path_buf)
        .or_else(|| shortcut.target.parent().map(Path::to_path_buf));
    if let Some(working) = working {
        let working = wide(working);
        checked("setting the shortcut folder", unsafe {
            (vtable.set_working_directory)(object.0, working.as_ptr())
        })?;
    }

    if let Some((icon, index)) = shortcut.icon {
        let icon = wide(icon);
        checked("setting the shortcut icon", unsafe {
            (vtable.set_icon_location)(object.0, icon.as_ptr(), index)
        })?;
    }

    let mut file: *mut c_void = std::ptr::null_mut();
    checked("asking the link how to save itself", unsafe {
        (vtable.query_interface)(object.0, &IID_PERSIST_FILE, &mut file)
    })?;
    if file.is_null() {
        return Err(Error::invalid("the link cannot be saved"));
    }

    let persist = Interface(file);
    let saving: &PersistFileVtbl = persist.vtable();
    let path = wide(link);
    checked("saving the shortcut", unsafe {
        (saving.save)(persist.0, path.as_ptr(), 1)
    })
}

pub fn remove(link: &Path) -> Result<()> {
    match std::fs::remove_file(link) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(Error::io(
            format!("could not remove {}", link.display()),
            err,
        )),
    }
}

fn known_folder(id: &GUID) -> Option<PathBuf> {
    let mut raw: PWSTR = std::ptr::null_mut();
    let outcome = unsafe { SHGetKnownFolderPath(id, 0, std::ptr::null_mut(), &mut raw) };
    if outcome < 0 || raw.is_null() {
        return None;
    }

    let mut length = 0;
    while unsafe { *raw.add(length) } != 0 {
        length += 1;
    }
    let text = from_wide_nul(unsafe { std::slice::from_raw_parts(raw, length + 1) });
    unsafe { CoTaskMemFree(raw as *const c_void) };

    if text.is_empty() {
        None
    } else {
        Some(PathBuf::from(text))
    }
}

pub fn desktop_dir() -> Option<PathBuf> {
    known_folder(&FOLDERID_Desktop)
}

pub fn start_menu_dir() -> Option<PathBuf> {
    known_folder(&FOLDERID_Programs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_desktop_and_the_start_menu_can_be_found() {
        let desktop = desktop_dir().expect("a desktop folder");
        let start = start_menu_dir().expect("a start menu folder");

        assert!(desktop.is_absolute(), "{}", desktop.display());
        assert!(start.is_absolute(), "{}", start.display());
    }

    #[test]
    fn a_shortcut_is_written_and_can_be_taken_away_again() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("RustBlox.exe");
        std::fs::write(&target, b"MZ").unwrap();
        let link = dir.path().join("Launch Roblox.lnk");

        create(
            &link,
            &Shortcut {
                target: &target,
                arguments: "--launch",
                description: "Start Roblox through RustBlox",
                working_dir: None,
                icon: None,
            },
        )
        .unwrap();

        assert!(link.is_file());
        let bytes = std::fs::read(&link).unwrap();
        assert!(bytes.len() > 76, "a shell link has a header");
        assert_eq!(&bytes[4..8], &[0x01, 0x14, 0x02, 0x00], "the link CLSID");

        remove(&link).unwrap();
        assert!(!link.exists());
        remove(&link).unwrap();
    }
}

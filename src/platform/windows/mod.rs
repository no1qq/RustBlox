use std::ffi::OsStr;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE, MAX_PATH};
use windows_sys::Win32::Storage::FileSystem::{
    GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW,
};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

use crate::error::{Error, Result};

use super::{FileVersion, ProcessInfo};

const DETACHED_PROCESS: u32 = 0x0000_0008;
const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;

fn wide(value: impl AsRef<OsStr>) -> Vec<u16> {
    value
        .as_ref()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

fn from_wide_nul(buffer: &[u16]) -> String {
    let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    std::ffi::OsString::from_wide(&buffer[..len])
        .to_string_lossy()
        .into_owned()
}

struct OwnedHandle(HANDLE);

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        if !self.0.is_null() && self.0 != INVALID_HANDLE_VALUE {
            unsafe { CloseHandle(self.0) };
        }
    }
}

pub fn find_processes(names: &[&str]) -> Vec<ProcessInfo> {
    let mut found = Vec::new();
    if names.is_empty() {
        return found;
    }

    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot == INVALID_HANDLE_VALUE {
        return found;
    }
    let snapshot = OwnedHandle(snapshot);

    let mut entry = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..unsafe { std::mem::zeroed() }
    };

    let mut ok = unsafe { Process32FirstW(snapshot.0, &mut entry) };
    while ok != 0 {
        let name = from_wide_nul(&entry.szExeFile);
        if names
            .iter()
            .any(|candidate| name.eq_ignore_ascii_case(candidate))
        {
            found.push(ProcessInfo {
                pid: entry.th32ProcessID,
                image: image_path(entry.th32ProcessID),
                name,
            });
        }
        ok = unsafe { Process32NextW(snapshot.0, &mut entry) };
    }

    found
}

fn image_path(pid: u32) -> Option<PathBuf> {
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if handle.is_null() {
        return None;
    }
    let handle = OwnedHandle(handle);

    let mut buffer = vec![0u16; MAX_PATH as usize * 2];
    let mut size = buffer.len() as u32;
    let ok = unsafe {
        QueryFullProcessImageNameW(
            handle.0,
            PROCESS_NAME_FORMAT::default(),
            buffer.as_mut_ptr(),
            &mut size,
        )
    };
    if ok == 0 {
        return None;
    }
    buffer.truncate(size as usize);
    Some(PathBuf::from(std::ffi::OsString::from_wide(&buffer)))
}

pub fn file_version(path: &Path) -> FileVersion {
    let wide_path = wide(path);
    let mut handle = 0u32;
    let size = unsafe { GetFileVersionInfoSizeW(wide_path.as_ptr(), &mut handle) };
    if size == 0 {
        return FileVersion::default();
    }

    let mut data = vec![0u8; size as usize];
    let ok = unsafe {
        GetFileVersionInfoW(
            wide_path.as_ptr(),
            0,
            size,
            data.as_mut_ptr() as *mut std::ffi::c_void,
        )
    };
    if ok == 0 {
        return FileVersion::default();
    }

    let langs = query_translations(&data);
    let mut version = FileVersion::default();
    for (lang, codepage) in langs {
        if version.file.is_none() {
            version.file = query_string(&data, lang, codepage, "FileVersion");
        }
        if version.product.is_none() {
            version.product = query_string(&data, lang, codepage, "ProductVersion");
        }
        if !version.is_empty() {
            break;
        }
    }
    version
}

fn query_translations(data: &[u8]) -> Vec<(u16, u16)> {
    let key = wide("\\VarFileInfo\\Translation");
    let mut ptr: *mut std::ffi::c_void = std::ptr::null_mut();
    let mut len = 0u32;
    let ok = unsafe {
        VerQueryValueW(
            data.as_ptr() as *const std::ffi::c_void,
            key.as_ptr(),
            &mut ptr,
            &mut len,
        )
    };
    if ok == 0 || ptr.is_null() || len < 4 {
        return vec![(0x0409, 0x04B0)];
    }

    let count = (len / 4) as usize;
    let entries = unsafe { std::slice::from_raw_parts(ptr as *const u16, count * 2) };
    let mut out: Vec<(u16, u16)> = entries
        .chunks_exact(2)
        .map(|pair| (pair[0], pair[1]))
        .collect();
    out.push((0x0409, 0x04B0));
    out
}

fn query_string(data: &[u8], lang: u16, codepage: u16, field: &str) -> Option<String> {
    let key = wide(format!(
        "\\StringFileInfo\\{lang:04x}{codepage:04x}\\{field}"
    ));
    let mut ptr: *mut std::ffi::c_void = std::ptr::null_mut();
    let mut len = 0u32;
    let ok = unsafe {
        VerQueryValueW(
            data.as_ptr() as *const std::ffi::c_void,
            key.as_ptr(),
            &mut ptr,
            &mut len,
        )
    };
    if ok == 0 || ptr.is_null() || len == 0 {
        return None;
    }
    let slice = unsafe { std::slice::from_raw_parts(ptr as *const u16, len as usize) };
    let text = from_wide_nul(slice).trim().to_owned();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn shell_execute(verb: &str, target: &str) -> Result<()> {
    let verb = wide(verb);
    let target = wide(target);
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            target.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    if (result as isize) <= 32 {
        return Err(Error::LaunchFailed(format!(
            "the shell refused to open the target (code {})",
            result as isize
        )));
    }
    Ok(())
}

pub fn open_path(path: &Path) -> Result<()> {
    shell_execute("open", &path.display().to_string())
}

pub fn open_url(url: &str) -> Result<()> {
    shell_execute("open", url)
}

pub fn spawn_detached(program: &Path, args: &[String], cwd: Option<&Path>) -> Result<Child> {
    let mut command = Command::new(program);
    command
        .args(args)
        .creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    command.spawn().map_err(|err| {
        Error::LaunchFailed(format!("{} could not be started: {err}", program.display()))
    })
}

pub mod protocol;

pub fn attach_parent_console() {
    use windows_sys::Win32::System::Console::{AllocConsole, AttachConsole, ATTACH_PARENT_PROCESS};

    unsafe {
        if AttachConsole(ATTACH_PARENT_PROCESS) == 0 {
            AllocConsole();
        }
    }
}

pub fn free_space(path: &Path) -> Option<u64> {
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;

    let mut probe = path;
    let existing = loop {
        if probe.is_dir() {
            break probe;
        }
        probe = probe.parent()?;
    };

    let wide_path = wide(existing);
    let mut available = 0u64;
    let ok = unsafe {
        GetDiskFreeSpaceExW(
            wide_path.as_ptr(),
            &mut available,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if ok == 0 {
        None
    } else {
        Some(available)
    }
}

pub fn system_dark_mode() -> Option<bool> {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ};
    use winreg::RegKey;

    let key = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(
            r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
            KEY_READ,
        )
        .ok()?;
    let light: u32 = key.get_value("AppsUseLightTheme").ok()?;
    Some(light == 0)
}

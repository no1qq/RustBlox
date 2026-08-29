use std::collections::HashMap;
use std::ffi::OsString;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};

use windows_sys::Win32::Foundation::{
    CloseHandle, DuplicateHandle, HANDLE, HWND, INVALID_HANDLE_VALUE, LPARAM, LUID,
};
use windows_sys::Win32::Security::{
    AdjustTokenPrivileges, LookupPrivilegeValueW, LUID_AND_ATTRIBUTES, SE_PRIVILEGE_ENABLED,
    TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY,
};
use windows_sys::Win32::Storage::FileSystem::{
    FindClose, FindFirstFileW, FindNextFileW, WIN32_FIND_DATAW,
};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use windows_sys::Win32::System::Threading::{
    GetCurrentProcess, GetProcessId, OpenProcess, OpenProcessToken, QueryFullProcessImageNameW,
    TerminateProcess, PROCESS_DUP_HANDLE, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE,
};
use windows_sys::Win32::UI::Shell::{
    ShellExecuteExW, Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE,
    NOTIFYICONDATAW, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateIconFromResourceEx, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW,
    EnumWindows, GetSystemMetrics, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId,
    IsWindowVisible, LoadIconW, PeekMessageW, RegisterClassW, TranslateMessage, HICON, IDI_SHIELD,
    LR_DEFAULTCOLOR, MSG, PM_REMOVE, SM_CXSMICON, SW_HIDE, WNDCLASSW, WS_OVERLAPPED,
};

use crate::platform::{SecurityReport, SecurityThreat, ThreatKind};

const THEWATCHER_ICO: &[u8] = include_bytes!("../../../assets/thewatcher.ico");

const CHEAT_PROCESS_KEYWORDS: &[&str] = &[
    "matrix",
    "matrixhub",
    "newui",
    "newuiv3",
    "solara",
    "celery",
    "wave",
    "swift",
    "krnl",
    "fluxus",
    "electron",
    "oxygenu",
    "valyse",
    "krampus",
    "xenos",
    "xeno",
    "extremeinjector",
    "cheatengine",
    "x64dbg",
    "x32dbg",
    "ida64",
    "ida",
    "scylla",
    "processhacker",
    "systeminformer",
    "httpdebugger",
    "reclass",
    "sydo",
    "aimbot",
    "wallhack",
    "streamproof",
    "injector",
    "executor",
    "exploit",
];

const CHEAT_WINDOW_KEYWORDS: &[&str] = &[
    "matrix hub",
    "matrix",
    "newuiv3",
    "solara",
    "wave",
    "celery",
    "krampus",
    "xeno",
    "xenos",
    "delta",
    "fluxus",
    "arceus",
    "codex",
    "electron",
    "krnl",
    "swift",
    "nemesis",
    "olympus",
    "valyse",
    "nihon",
    "furk",
    "oxygen u",
    "jjsploit",
    "script-ware",
    "scriptware",
    "cheat engine",
    "x64dbg",
    "x32dbg",
    "ida pro",
    "process hacker",
    "system informer",
    "http debugger",
    "reclass",
    "roblox external",
    "external overlay",
    "script executor",
    "cheat executor",
];

#[allow(dead_code)]
const KNOWN_EXECUTOR_DLLS: &[&str] = &[
    "solara.dll",
    "solarainject.dll",
    "celery.dll",
    "celeryinject.dll",
    "wave.dll",
    "waveinject.dll",
    "swift.dll",
    "swiftinject.dll",
    "krnl.dll",
    "fluxus.dll",
    "oxygenu.dll",
    "valyse.dll",
    "speedhack.dll",
    "minhook.dll",
    "d3d11_hook.dll",
    "dxgi_hook.dll",
    "krampus.dll",
    "macsploit.dll",
];

const PROXY_DLL_NAMES: &[&str] = &[
    "version.dll",
    "dxgi.dll",
    "d3d11.dll",
    "d3d10.dll",
    "d3d9.dll",
    "winmm.dll",
    "hid.dll",
    "userenv.dll",
    "dbghelp.dll",
    "cryptbase.dll",
];

const EXECUTOR_PIPE_PREFIXES: &[&str] = &[
    "matrix",
    "matrix_pipe",
    "celery",
    "solara",
    "wave",
    "swift",
    "krnl",
    "fluxus",
    "electron",
    "oxygen",
    "roblox_pipe",
    "rbx_pipe",
    "injector_pipe",
    "sw_pipe",
    "valyse",
    "wearedevs",
];

struct OwnedHandle(HANDLE);

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        if !self.0.is_null() && self.0 != INVALID_HANDLE_VALUE {
            unsafe { CloseHandle(self.0) };
        }
    }
}

fn from_wide_nul(buffer: &[u16]) -> String {
    let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    OsString::from_wide(&buffer[..len])
        .to_string_lossy()
        .into_owned()
}

fn wide_null(value: &str) -> Vec<u16> {
    std::ffi::OsStr::new(value)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

pub fn scan_security(player_pid: Option<u32>, install_dir: Option<&Path>) -> SecurityReport {
    let mut threats = Vec::new();
    let mut process_map = HashMap::new();

    scan_cheat_processes(&mut threats, &mut process_map);
    scan_cheat_windows(&process_map, &mut threats);
    scan_executor_pipes(&mut threats);

    if let Some(pid) = player_pid {
        scan_roblox_memory_handles(pid, &mut threats);
    }

    if let Some(dir) = install_dir {
        scan_roblox_dir_integrity(dir, &mut threats);
    }

    SecurityReport { threats }
}

const WHITELISTED_PROCESS_NAMES: &[&str] = &[
    "explorer.exe",
    "svchost.exe",
    "csrss.exe",
    "smss.exe",
    "lsass.exe",
    "services.exe",
    "wininit.exe",
    "winlogon.exe",
    "dwm.exe",
    "spoolsv.exe",
    "sihost.exe",
    "taskhostw.exe",
    "ctfmon.exe",
    "cmd.exe",
    "powershell.exe",
    "pwsh.exe",
    "conhost.exe",
    "windowsterminal.exe",
    "openconsole.exe",
    "rustblox.exe",
    "robloxplayerbeta.exe",
    "robloxstudiobeta.exe",
    "robloxcrashhandler.exe",
    "searchhost.exe",
    "startmenuexperiencehost.exe",
    "shellexperiencehost.exe",
    "applicationframehost.exe",
    "systemsettings.exe",
    "textinputhost.exe",
    "runtimebroker.exe",
    "searchapp.exe",
    "securityhealthsystray.exe",
    "securityhealthservice.exe",
    "audiodg.exe",
    "fontdrvhost.exe",
    "taskmgr.exe",
    "tasklist.exe",
    "wmiapsrv.exe",
    "wmiprvse.exe",
    "dllhost.exe",
    "smartscreen.exe",
    "registry",
    "system",
    "discord.exe",
    "discordcanary.exe",
    "discordptb.exe",
    "chrome.exe",
    "msedge.exe",
    "firefox.exe",
    "brave.exe",
    "opera.exe",
    "operagx.exe",
    "vivaldi.exe",
    "code.exe",
    "devenv.exe",
    "git.exe",
    "cargo.exe",
    "rustc.exe",
    "steam.exe",
    "steamwebhelper.exe",
    "epicgameslauncher.exe",
    "spotify.exe",
    "slack.exe",
    "telegram.exe",
    "notepad.exe",
    "notepad++.exe",
    "nvidia share.exe",
    "nvcontainer.exe",
    "amdrsserv.exe",
];

const AUTHORIZED_HANDLE_HOLDERS: &[&str] = &[
    "robloxcrashhandler.exe",
    "rustblox.exe",
    "medalencoder.exe",
    "medal.exe",
    "obs64.exe",
    "obs32.exe",
    "overwolfhelper64.exe",
    "nvidia share.exe",
    "nvcontainer.exe",
    "radeonsofware.exe",
    "amdow.exe",
    "csrss.exe",
    "lsass.exe",
    "services.exe",
    "svchost.exe",
];

pub fn get_process_image_path(pid: u32) -> Option<String> {
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if handle.is_null() {
        return None;
    }
    let handle = OwnedHandle(handle);
    let mut buffer = vec![0u16; 1024];
    let mut size = buffer.len() as u32;
    let ok = unsafe { QueryFullProcessImageNameW(handle.0, 0, buffer.as_mut_ptr(), &mut size) };
    if ok != 0 && size > 0 {
        Some(from_wide_nul(&buffer[..size as usize]))
    } else {
        None
    }
}

const SYSTEM_ONLY_NAMES: &[&str] = &[
    "explorer.exe",
    "svchost.exe",
    "csrss.exe",
    "smss.exe",
    "lsass.exe",
    "services.exe",
    "wininit.exe",
    "winlogon.exe",
    "dwm.exe",
    "spoolsv.exe",
    "sihost.exe",
    "taskhostw.exe",
    "ctfmon.exe",
    "taskmgr.exe",
    "audiodg.exe",
    "fontdrvhost.exe",
    "runtimebroker.exe",
    "searchhost.exe",
    "searchapp.exe",
    "smartscreen.exe",
    "dllhost.exe",
    "conhost.exe",
    "cmd.exe",
    "powershell.exe",
    "pwsh.exe",
];

pub fn is_system_only_name(name: &str) -> bool {
    let name_lower = name.to_ascii_lowercase();
    let file_name = match name_lower.rfind('\\') {
        Some(idx) => &name_lower[idx + 1..],
        None => match name_lower.rfind('/') {
            Some(idx) => &name_lower[idx + 1..],
            None => &name_lower,
        },
    };
    SYSTEM_ONLY_NAMES.contains(&file_name)
}

pub fn is_valid_whitelisted_path(name: &str, path: Option<&str>) -> bool {
    let name_lower = name.to_ascii_lowercase();
    let file_name = match name_lower.rfind('\\') {
        Some(idx) => &name_lower[idx + 1..],
        None => match name_lower.rfind('/') {
            Some(idx) => &name_lower[idx + 1..],
            None => &name_lower,
        },
    };

    if !WHITELISTED_PROCESS_NAMES.contains(&file_name) {
        return false;
    }

    let Some(full_path) = path else {
        return !is_system_only_name(file_name);
    };
    let path_lower = full_path.to_ascii_lowercase();

    if is_system_only_name(file_name) {
        return path_lower.starts_with(r"c:\windows\")
            || path_lower.starts_with(r"c:\program files\windowsapps\");
    }

    let is_non_c_drive = path_lower.len() >= 3
        && path_lower.as_bytes()[1] == b':'
        && path_lower.as_bytes()[2] == b'\\'
        && path_lower.as_bytes()[0] != b'c';

    if (path_lower.contains(r"\appdata\local\temp\")
        || path_lower.contains(r"\temp\")
        || path_lower.contains(r"\downloads\")
        || is_non_c_drive)
        && file_name != "rustblox.exe"
        && file_name != "cargo.exe"
        && file_name != "rustc.exe"
        && file_name != "git.exe"
    {
        return false;
    }

    if file_name.starts_with("discord") {
        return path_lower.contains(r"\discord")
            || path_lower.starts_with(r"c:\program files")
            || path_lower.starts_with(r"c:\program files (x86)");
    }

    if file_name == "chrome.exe"
        || file_name == "msedge.exe"
        || file_name == "brave.exe"
        || file_name == "firefox.exe"
        || file_name == "opera.exe"
        || file_name == "operagx.exe"
        || file_name == "vivaldi.exe"
    {
        return path_lower.starts_with(r"c:\program files")
            || path_lower.starts_with(r"c:\program files (x86)")
            || path_lower.contains(r"\appdata\local\");
    }

    true
}

pub fn is_authorized_roblox_handle_holder(name: &str, path: Option<&str>) -> bool {
    let name_lower = name.to_ascii_lowercase();
    let file_name = match name_lower.rfind('\\') {
        Some(idx) => &name_lower[idx + 1..],
        None => match name_lower.rfind('/') {
            Some(idx) => &name_lower[idx + 1..],
            None => &name_lower,
        },
    };

    if !AUTHORIZED_HANDLE_HOLDERS.contains(&file_name) {
        return false;
    }

    if let Some(p) = path {
        let p_lower = p.to_ascii_lowercase();
        let is_non_c = p_lower.len() >= 3
            && p_lower.as_bytes()[1] == b':'
            && p_lower.as_bytes()[2] == b'\\'
            && p_lower.as_bytes()[0] != b'c';

        if p_lower.contains(r"\temp\") || p_lower.contains(r"\downloads\") || is_non_c {
            return false;
        }
    }

    true
}

#[allow(dead_code)]
pub fn is_whitelisted_process(name: &str) -> bool {
    let name_lower = name.to_ascii_lowercase();
    let file_name = match name_lower.rfind('\\') {
        Some(idx) => &name_lower[idx + 1..],
        None => match name_lower.rfind('/') {
            Some(idx) => &name_lower[idx + 1..],
            None => &name_lower,
        },
    };
    WHITELISTED_PROCESS_NAMES.contains(&file_name)
}

pub fn get_process_name_by_pid(pid: u32) -> Option<String> {
    if pid == 0 || pid == 4 {
        return Some("System".into());
    }
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot == INVALID_HANDLE_VALUE {
        return None;
    }
    let snapshot = OwnedHandle(snapshot);

    let mut entry = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..unsafe { std::mem::zeroed() }
    };

    let mut ok = unsafe { Process32FirstW(snapshot.0, &mut entry) };
    while ok != 0 {
        if entry.th32ProcessID == pid {
            return Some(from_wide_nul(&entry.szExeFile));
        }
        ok = unsafe { Process32NextW(snapshot.0, &mut entry) };
    }
    None
}

pub fn is_whitelisted_pid(pid: u32) -> bool {
    if pid == 0 || pid == 4 {
        return true;
    }
    if let Some(name) = get_process_name_by_pid(pid) {
        let path = get_process_image_path(pid);
        if is_valid_whitelisted_path(&name, path.as_deref()) {
            return true;
        }
    }
    false
}

#[repr(C)]
struct SYSTEM_HANDLE_TABLE_ENTRY_INFO_EX {
    _object: *mut std::ffi::c_void,
    unique_process_id: usize,
    handle_value: usize,
    granted_access: u32,
    _creator_back_trace_index: u16,
    _object_type_index: u16,
    _handle_attributes: u32,
    _reserved: u32,
}

type NtQuerySystemInformationFn = unsafe extern "system" fn(
    system_information_class: i32,
    system_information: *mut std::ffi::c_void,
    system_information_length: u32,
    return_length: *mut u32,
) -> i32;

fn scan_roblox_memory_handles(target_pid: u32, threats: &mut Vec<SecurityThreat>) {
    unsafe {
        let ntdll_name = wide_null("ntdll.dll");
        let h_ntdll = GetModuleHandleW(ntdll_name.as_ptr());
        if h_ntdll.is_null() {
            return;
        }
        let proc = GetProcAddress(h_ntdll, c"NtQuerySystemInformation".as_ptr() as _);
        let Some(proc_addr) = proc else {
            return;
        };
        let nt_query_system_information: NtQuerySystemInformationFn =
            std::mem::transmute(proc_addr);

        let mut size = 1024 * 1024 * 4;
        let mut buffer: Vec<u8> = vec![0; size];
        let mut needed: u32 = 0;

        let mut status =
            nt_query_system_information(64, buffer.as_mut_ptr() as _, size as u32, &mut needed);
        while status == -1073741820 {
            size = (needed as usize) + 1024 * 1024;
            buffer.resize(size, 0);
            status =
                nt_query_system_information(64, buffer.as_mut_ptr() as _, size as u32, &mut needed);
        }

        if status != 0 || buffer.len() < 16 {
            return;
        }

        let count = *(buffer.as_ptr() as *const usize);
        let entry_size = std::mem::size_of::<SYSTEM_HANDLE_TABLE_ENTRY_INFO_EX>();
        let current_process = GetCurrentProcess();
        let my_pid = std::process::id();

        for i in 0..count {
            let offset = 16 + i * entry_size;
            if offset + entry_size > buffer.len() {
                break;
            }
            let entry = &*(buffer.as_ptr().add(offset) as *const SYSTEM_HANDLE_TABLE_ENTRY_INFO_EX);
            let holding_pid = entry.unique_process_id as u32;

            if holding_pid == target_pid
                || holding_pid == my_pid
                || holding_pid == 0
                || holding_pid == 4
            {
                continue;
            }

            if (entry.granted_access
                & (0x0010 | 0x0020 | 0x0008 | 0x0040 | 0x0800 | 0x001F0000 | 0x0400))
                == 0
            {
                continue;
            }

            let holding_handle = OpenProcess(PROCESS_DUP_HANDLE, 0, holding_pid);
            if !holding_handle.is_null() {
                let mut dup_handle: HANDLE = std::ptr::null_mut();
                if DuplicateHandle(
                    holding_handle,
                    entry.handle_value as HANDLE,
                    current_process,
                    &mut dup_handle,
                    PROCESS_QUERY_LIMITED_INFORMATION,
                    0,
                    0,
                ) != 0
                {
                    let target_of_handle = GetProcessId(dup_handle);
                    if target_of_handle == target_pid {
                        let proc_name = get_process_name_by_pid(holding_pid)
                            .unwrap_or_else(|| "Unknown".into());
                        let proc_path = get_process_image_path(holding_pid);

                        if !is_authorized_roblox_handle_holder(&proc_name, proc_path.as_deref()) {
                            threats.push(SecurityThreat {
                                kind: ThreatKind::KnownCheatProcess,
                                name: proc_name.clone(),
                                detail: format!(
                                    "Unauthorized external process holding memory-read handle to Roblox: {proc_name} (PID {holding_pid})"
                                ),
                                pid: Some(holding_pid),
                            });
                        }
                    }
                    CloseHandle(dup_handle);
                }
                CloseHandle(holding_handle);
            }
        }
    }
}

pub fn terminate_threat_pid(pid: u32) -> bool {
    if pid == 0 || pid == 4 || is_whitelisted_pid(pid) {
        return false;
    }
    let handle = unsafe { OpenProcess(PROCESS_TERMINATE, 0, pid) };
    if handle.is_null() {
        return false;
    }
    let handle = OwnedHandle(handle);
    unsafe { TerminateProcess(handle.0, 1) != 0 }
}

fn scan_cheat_processes(threats: &mut Vec<SecurityThreat>, process_map: &mut HashMap<u32, String>) {
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot == INVALID_HANDLE_VALUE {
        return;
    }
    let snapshot = OwnedHandle(snapshot);

    let mut entry = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..unsafe { std::mem::zeroed() }
    };

    let mut ok = unsafe { Process32FirstW(snapshot.0, &mut entry) };
    while ok != 0 {
        let name = from_wide_nul(&entry.szExeFile);
        let name_lower = name.to_ascii_lowercase();
        let pid = entry.th32ProcessID;

        if pid != 0 && pid != 4 {
            let proc_path = get_process_image_path(pid);
            let is_trusted = is_valid_whitelisted_path(&name, proc_path.as_deref());

            if !is_trusted {
                let is_cheat = CHEAT_PROCESS_KEYWORDS
                    .iter()
                    .any(|kw| name_lower.contains(kw));

                let is_system = is_system_only_name(&name);

                if is_cheat || is_system {
                    threats.push(SecurityThreat {
                        kind: ThreatKind::KnownCheatProcess,
                        name: name.clone(),
                        detail: if is_system {
                            format!(
                                "Impostor process disguised with system name: {name} (PID {pid}, Path: {})",
                                proc_path.as_deref().unwrap_or("Unknown")
                            )
                        } else {
                            format!(
                                "Active cheat or script executor process running: {name} (PID {pid})"
                            )
                        },
                        pid: Some(pid),
                    });
                }
            }
        }

        process_map.insert(entry.th32ProcessID, name);
        ok = unsafe { Process32NextW(snapshot.0, &mut entry) };
    }
}

struct WindowScanContext {
    threats: Vec<SecurityThreat>,
}

unsafe extern "system" fn enum_windows_proc(hwnd: HWND, lparam: LPARAM) -> i32 {
    if IsWindowVisible(hwnd) == 0 {
        return 1;
    }
    let length = GetWindowTextLengthW(hwnd);
    if length <= 0 {
        return 1;
    }
    let mut buffer = vec![0u16; (length + 1) as usize];
    let copied = GetWindowTextW(hwnd, buffer.as_mut_ptr(), length + 1);
    if copied <= 0 {
        return 1;
    }
    let title = from_wide_nul(&buffer);
    let title_lower = title.to_ascii_lowercase();

    let matches_cheat = CHEAT_WINDOW_KEYWORDS
        .iter()
        .any(|keyword| title_lower.contains(keyword));

    if matches_cheat {
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid != 0 && pid != 4 && !is_whitelisted_pid(pid) {
            let ctx = &mut *(lparam as *mut WindowScanContext);
            ctx.threats.push(SecurityThreat {
                kind: ThreatKind::KnownCheatProcess,
                name: title.clone(),
                detail: format!("Cheat window '{title}' detected (PID {pid})"),
                pid: Some(pid),
            });
        }
    }

    1
}

fn scan_cheat_windows(_process_map: &HashMap<u32, String>, threats: &mut Vec<SecurityThreat>) {
    let mut context = WindowScanContext {
        threats: Vec::new(),
    };
    unsafe {
        EnumWindows(Some(enum_windows_proc), &mut context as *mut _ as LPARAM);
    }
    threats.extend(context.threats);
}

fn scan_executor_pipes(threats: &mut Vec<SecurityThreat>) {
    let pipe_search = wide_null(r"\\.\pipe\*");
    let mut find_data: WIN32_FIND_DATAW = unsafe { std::mem::zeroed() };

    let handle = unsafe { FindFirstFileW(pipe_search.as_ptr(), &mut find_data) };
    if handle == INVALID_HANDLE_VALUE {
        return;
    }

    struct FindHandle(HANDLE);
    impl Drop for FindHandle {
        fn drop(&mut self) {
            if self.0 != INVALID_HANDLE_VALUE {
                unsafe { FindClose(self.0) };
            }
        }
    }
    let _find_guard = FindHandle(handle);

    loop {
        let pipe_name = from_wide_nul(&find_data.cFileName);
        let pipe_lower = pipe_name.to_ascii_lowercase();

        if EXECUTOR_PIPE_PREFIXES
            .iter()
            .any(|prefix| pipe_lower.contains(prefix))
        {
            threats.push(SecurityThreat {
                kind: ThreatKind::ScriptExecutorPipe,
                name: format!(r"\\.\pipe\{pipe_name}"),
                detail: "Active script executor communication pipe detected".into(),
                pid: None,
            });
        }

        let next = unsafe { FindNextFileW(handle, &mut find_data) };
        if next == 0 {
            break;
        }
    }
}

fn scan_roblox_dir_integrity(dir: &Path, threats: &mut Vec<SecurityThreat>) {
    for proxy in PROXY_DLL_NAMES {
        let path = dir.join(proxy);
        if path.is_file() {
            threats.push(SecurityThreat {
                kind: ThreatKind::RogueInstallFile,
                name: (*proxy).to_string(),
                detail: format!(
                    "Suspected proxy DLL in Roblox installation directory: {}",
                    path.display()
                ),
                pid: None,
            });
        }
    }
}

pub fn clean_roblox_dir_proxies(dir: &Path) -> Vec<PathBuf> {
    let mut removed = Vec::new();
    for proxy in PROXY_DLL_NAMES {
        let path = dir.join(proxy);
        if path.is_file() && std::fs::remove_file(&path).is_ok() {
            removed.push(path);
        }
    }
    removed
}

fn load_thewatcher_icon() -> HICON {
    unsafe {
        let hinstance = GetModuleHandleW(std::ptr::null());
        let icon_res = LoadIconW(hinstance, 2 as _);
        if icon_res != 0 as HICON {
            return icon_res;
        }
    }

    let bytes = THEWATCHER_ICO;
    if bytes.len() >= 22 {
        let count = u16::from_le_bytes([bytes[4], bytes[5]]) as usize;
        let desired_size = unsafe { GetSystemMetrics(SM_CXSMICON) };
        let mut best_offset = 0;
        let mut best_size = 0;
        let mut best_diff = i32::MAX;

        for i in 0..count {
            let entry_offset = 6 + i * 16;
            if entry_offset + 16 > bytes.len() {
                break;
            }
            let mut width = bytes[entry_offset] as i32;
            if width == 0 {
                width = 256;
            }
            let dw_bytes_in_res = u32::from_le_bytes([
                bytes[entry_offset + 8],
                bytes[entry_offset + 9],
                bytes[entry_offset + 10],
                bytes[entry_offset + 11],
            ]) as usize;
            let dw_image_offset = u32::from_le_bytes([
                bytes[entry_offset + 12],
                bytes[entry_offset + 13],
                bytes[entry_offset + 14],
                bytes[entry_offset + 15],
            ]) as usize;

            let diff = (width - desired_size).abs();
            if diff < best_diff {
                best_diff = diff;
                best_offset = dw_image_offset;
                best_size = dw_bytes_in_res;
            }
        }

        if best_size > 0 && best_offset + best_size <= bytes.len() {
            let icon_data = &bytes[best_offset..best_offset + best_size];
            let hicon = unsafe {
                CreateIconFromResourceEx(
                    icon_data.as_ptr(),
                    icon_data.len() as u32,
                    1,
                    0x00030000,
                    desired_size,
                    desired_size,
                    LR_DEFAULTCOLOR,
                )
            };
            if hicon != 0 as HICON {
                return hicon;
            }
        }
    }

    unsafe { LoadIconW(0 as _, IDI_SHIELD) }
}

#[allow(dead_code)]
pub fn show_tray_icon(_tooltip: &str) {}

pub fn hide_tray_icon() {}

pub fn enable_debug_privilege() -> bool {
    unsafe {
        let mut token: HANDLE = std::ptr::null_mut();
        if OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY,
            &mut token,
        ) == 0
        {
            return false;
        }

        let mut luid: LUID = std::mem::zeroed();
        let name = wide_null("SeDebugPrivilege");
        if LookupPrivilegeValueW(std::ptr::null(), name.as_ptr(), &mut luid) == 0 {
            CloseHandle(token);
            return false;
        }

        let mut tp = TOKEN_PRIVILEGES {
            PrivilegeCount: 1,
            Privileges: [LUID_AND_ATTRIBUTES {
                Luid: luid,
                Attributes: SE_PRIVILEGE_ENABLED,
            }],
        };

        let result = AdjustTokenPrivileges(
            token,
            0,
            &mut tp as *mut _ as _,
            std::mem::size_of::<TOKEN_PRIVILEGES>() as u32,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );

        CloseHandle(token);
        result != 0
    }
}

pub fn spawn_elevated(program: &Path, args: &[String]) -> crate::error::Result<()> {
    let file = wide_null(&program.display().to_string());
    let params_str = args
        .iter()
        .map(|a| {
            if a.contains(' ') {
                format!("\"{a}\"")
            } else {
                a.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    let params = wide_null(&params_str);
    let verb = wide_null("runas");

    unsafe {
        let mut info: SHELLEXECUTEINFOW = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
        info.fMask = SEE_MASK_NOASYNC | SEE_MASK_NOCLOSEPROCESS;
        info.lpVerb = verb.as_ptr();
        info.lpFile = file.as_ptr();
        info.lpParameters = params.as_ptr();
        info.nShow = SW_HIDE;

        let ok = ShellExecuteExW(&mut info);
        if ok == 0 {
            let err = windows_sys::Win32::Foundation::GetLastError();
            return Err(crate::error::Error::LaunchFailed(format!(
                "Administrator elevation was cancelled or failed (code {err})"
            )));
        }

        if !info.hProcess.is_null() {
            CloseHandle(info.hProcess);
        }
    }

    Ok(())
}

pub fn run_thewatcher_service(mut pid: u32, install_dir: PathBuf) {
    enable_debug_privilege();

    unsafe {
        let hinstance = GetModuleHandleW(std::ptr::null());
        let class_name = wide_null("TheWatcherTrayClass");

        let wnd_class = WNDCLASSW {
            style: 0,
            lpfnWndProc: Some(DefWindowProcW),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinstance,
            hIcon: 0 as _,
            hCursor: 0 as _,
            hbrBackground: 0 as _,
            lpszMenuName: std::ptr::null(),
            lpszClassName: class_name.as_ptr(),
        };
        RegisterClassW(&wnd_class);

        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            class_name.as_ptr(),
            WS_OVERLAPPED,
            0,
            0,
            0,
            0,
            0 as _,
            0 as _,
            hinstance,
            std::ptr::null(),
        );

        let icon = load_thewatcher_icon();

        let mut tip: [u16; 128] = [0; 128];
        let tooltip = "TheWatcher Anti-Cheat - Active";
        let utf16: Vec<u16> = tooltip.encode_utf16().take(127).collect();
        tip[..utf16.len()].copy_from_slice(&utf16);

        let mut data: NOTIFYICONDATAW = std::mem::zeroed();
        data.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        data.hWnd = hwnd;
        data.uID = 0x524258;
        data.uFlags = NIF_ICON | NIF_TIP | NIF_MESSAGE;
        data.uCallbackMessage = 0x8001;
        data.hIcon = icon;
        data.szTip = tip;

        Shell_NotifyIconW(NIM_ADD, &data);

        let mut msg: MSG = std::mem::zeroed();
        let mut last_scan = std::time::Instant::now();
        let start_time = std::time::Instant::now();
        let mut roblox_handle: HANDLE = if pid != 0 {
            OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid)
        } else {
            std::ptr::null_mut()
        };
        let mut consecutive_dead: u32 = 0;

        loop {
            while PeekMessageW(&mut msg, hwnd, 0, 0, PM_REMOVE) != 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            let status = crate::roblox::process::status();
            if let Some(player) = status.players.first() {
                if player.pid != pid {
                    if !roblox_handle.is_null() {
                        CloseHandle(roblox_handle);
                    }
                    pid = player.pid;
                    roblox_handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
                }
                consecutive_dead = 0;
            } else if pid == 0 {
                if start_time.elapsed() > std::time::Duration::from_secs(120) {
                    break;
                }
            } else {
                consecutive_dead += 1;
                if consecutive_dead >= 10 {
                    break;
                }
            }

            if pid != 0
                && consecutive_dead == 0
                && last_scan.elapsed() >= std::time::Duration::from_secs(3)
            {
                let report = scan_security(Some(pid), Some(&install_dir));
                for threat in report.threats {
                    if let Some(threat_pid) = threat.pid {
                        if threat_pid != pid {
                            terminate_threat_pid(threat_pid);
                        }
                    }
                }
                last_scan = std::time::Instant::now();
            }

            std::thread::sleep(std::time::Duration::from_millis(200));
        }

        if !roblox_handle.is_null() {
            CloseHandle(roblox_handle);
        }

        Shell_NotifyIconW(NIM_DELETE, &data);

        if hwnd != 0 as _ {
            DestroyWindow(hwnd);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_whitelisted_processes() {
        assert!(is_whitelisted_process("explorer.exe"));
        assert!(is_whitelisted_process("EXPLORER.EXE"));
        assert!(is_whitelisted_process(r"C:\Windows\explorer.exe"));
        assert!(is_whitelisted_process(r"C:\Windows\System32\svchost.exe"));
        assert!(is_whitelisted_process("chrome.exe"));
        assert!(is_whitelisted_process("discord.exe"));
        assert!(is_whitelisted_process("code.exe"));
        assert!(is_whitelisted_process("powershell.exe"));
        assert!(is_whitelisted_process("rustblox.exe"));

        assert!(!is_whitelisted_process("newuiv3.exe"));
        assert!(!is_whitelisted_process("matrixhub.exe"));
        assert!(!is_whitelisted_process("solara.exe"));
        assert!(!is_whitelisted_process("wave.exe"));
    }

    #[test]
    fn test_valid_whitelisted_paths() {
        assert!(is_valid_whitelisted_path(
            "explorer.exe",
            Some(r"C:\Windows\explorer.exe")
        ));
        assert!(is_valid_whitelisted_path(
            "svchost.exe",
            Some(r"C:\Windows\System32\svchost.exe")
        ));
        assert!(is_valid_whitelisted_path(
            "discord.exe",
            Some(r"C:\Users\Julian\AppData\Local\Discord\app-1.0.9168\Discord.exe")
        ));
        assert!(is_valid_whitelisted_path(
            "chrome.exe",
            Some(r"C:\Program Files\Google\Chrome\Application\chrome.exe")
        ));

        assert!(!is_valid_whitelisted_path(
            "explorer.exe",
            Some(r"D:\matrix\explorer.exe")
        ));
        assert!(!is_valid_whitelisted_path(
            "svchost.exe",
            Some(r"C:\Users\Julian\AppData\Local\Temp\svchost.exe")
        ));
        assert!(!is_valid_whitelisted_path(
            "discord.exe",
            Some(r"D:\newv3uimatrix\discord.exe")
        ));
        assert!(!is_valid_whitelisted_path(
            "discord.exe",
            Some(r"C:\Users\Julian\AppData\Local\Temp\discord.exe")
        ));
    }

    #[test]
    fn test_authorized_handle_holders() {
        assert!(is_authorized_roblox_handle_holder(
            "RobloxCrashHandler.exe",
            Some(r"C:\Users\Julian\AppData\Local\RustBlox\data\Versions\RobloxCrashHandler.exe")
        ));
        assert!(is_authorized_roblox_handle_holder(
            "RustBlox.exe",
            Some(r"C:\Users\Julian\Desktop\RustBlox.exe")
        ));
        assert!(is_authorized_roblox_handle_holder(
            "MedalEncoder.exe",
            Some(r"C:\Users\Julian\AppData\Local\Medal\recorder\MedalEncoder.exe")
        ));
        assert!(is_authorized_roblox_handle_holder(
            "obs64.exe",
            Some(r"C:\Program Files\obs-studio\bin\64bit\obs64.exe")
        ));

        assert!(!is_authorized_roblox_handle_holder(
            "newuiv3.exe",
            Some(r"D:\matrix\newuiv3.exe")
        ));
        assert!(!is_authorized_roblox_handle_holder(
            "matrix.exe",
            Some(r"D:\matrix\matrix.exe")
        ));
        assert!(!is_authorized_roblox_handle_holder(
            "python.exe",
            Some(r"C:\Python310\python.exe")
        ));
        assert!(!is_authorized_roblox_handle_holder(
            "RobloxCrashHandler.exe",
            Some(r"C:\Users\Julian\AppData\Local\Temp\RobloxCrashHandler.exe")
        ));
    }

    #[test]
    fn test_whitelisted_pid_protection() {
        assert!(is_whitelisted_pid(0));
        assert!(is_whitelisted_pid(4));
        assert!(!terminate_threat_pid(0));
        assert!(!terminate_threat_pid(4));
        let my_pid = std::process::id();
        assert!(
            is_whitelisted_pid(my_pid)
                || is_whitelisted_process("RustBlox.exe")
                || is_whitelisted_process("cargo.exe")
        );
    }
}

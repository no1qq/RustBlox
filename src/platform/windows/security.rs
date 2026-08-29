use std::collections::HashMap;
use std::ffi::OsString;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};

use windows_sys::Win32::Foundation::{
    CloseHandle, DuplicateHandle, HANDLE, HWND, INVALID_HANDLE_VALUE, LPARAM, LUID, RECT,
};
use windows_sys::Win32::Security::{
    AdjustTokenPrivileges, LookupPrivilegeValueW, LUID_AND_ATTRIBUTES, SE_PRIVILEGE_ENABLED,
    TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FindClose, FindFirstFileW, FindNextFileW, FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE,
    FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING, WIN32_FIND_DATAW,
};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Module32FirstW, Module32NextW, Process32FirstW, Process32NextW,
    MODULEENTRY32W, PROCESSENTRY32W, TH32CS_SNAPMODULE, TH32CS_SNAPMODULE32, TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress, LoadLibraryW};
use windows_sys::Win32::System::Memory::{
    VirtualQueryEx, MEMORY_BASIC_INFORMATION, MEM_COMMIT, MEM_PRIVATE, PAGE_EXECUTE,
    PAGE_EXECUTE_READ, PAGE_EXECUTE_READWRITE, PAGE_EXECUTE_WRITECOPY,
};
use windows_sys::Win32::System::Pipes::GetNamedPipeServerProcessId;
use windows_sys::Win32::System::Threading::{
    GetCurrentProcess, GetProcessId, OpenProcess, OpenProcessToken, QueryFullProcessImageNameW,
    TerminateProcess, PROCESS_DUP_HANDLE, PROCESS_QUERY_INFORMATION,
    PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE, PROCESS_VM_READ,
};
use windows_sys::Win32::UI::Shell::{
    ShellExecuteExW, Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE,
    NOTIFYICONDATAW, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateIconFromResourceEx, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW,
    EnumWindows, GetSystemMetrics, GetWindowLongW, GetWindowRect, GetWindowTextLengthW,
    GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible, LoadIconW, PeekMessageW,
    RegisterClassW, TranslateMessage, GWL_EXSTYLE, HICON, IDI_SHIELD, LR_DEFAULTCOLOR, MSG,
    PM_REMOVE, SM_CXSMICON, SW_HIDE, WNDCLASSW, WS_OVERLAPPED,
};

use crate::platform::{SecurityReport, SecurityThreat, ThreatKind};

const THEWATCHER_ICO: &[u8] = include_bytes!("../../../assets/thewatcher.ico");

const CHEAT_PROCESS_KEYWORDS: &[&str] = &[
    "matrix",
    "matrixhub",
    "matcha",
    "matchahub",
    "matchav2",
    "matcha_external",
    "aimmy",
    "neuralaim",
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
    "valex",
    "zenith",
    "horizon",
    "nyx",
    "aether",
    "zeroesp",
    "synapsez",
    "potassium",
    "macsploit",
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
    "interception",
];

const CHEAT_WINDOW_KEYWORDS: &[&str] = &[
    "matrix hub",
    "matrix",
    "matcha external",
    "matcha v2",
    "matcha",
    "matchahub",
    "aimmy",
    "neural aim",
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
    "valex",
    "zenith",
    "horizon",
    "nyx",
    "aether",
    "zeroesp",
    "synapse z",
    "potassium",
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
    "matcha.dll",
    "matchainject.dll",
    "solara.dll",
    "solarainject.dll",
    "celery.dll",
    "celeryinject.dll",
    "wave.dll",
    "waveinject.dll",
    "swift.dll",
    "swiftinject.dll",
    "synapsez.dll",
    "potassium.dll",
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
    "matcha",
    "matcha_pipe",
    "aimmy",
    "synapsez",
    "potassium",
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

const KERNEL_CHEAT_DEVICE_PATHS: &[&str] = &[
    r"\\.\Matcha",
    r"\\.\MatchaDriver",
    r"\\.\MatchaDev",
    r"\\.\GIO",
    r"\\.\gdrv",
    r"\\.\mhyprot2",
    r"\\.\RTCore64",
    r"\\.\DBUtil_2_3",
    r"\\.\EchoDrv",
    r"\\.\DirectIo64",
    r"\\.\Capcom",
    r"\\.\KIO",
    r"\\.\Physmem",
    r"\\.\interception",
];

const CRITICAL_INTEGRITY_DLLS: &[&str] = &[
    "ntdll.dll",
    "kernel32.dll",
    "kernelbase.dll",
    "user32.dll",
    "d3d11.dll",
    "dxgi.dll",
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
    scan_unauthorized_ipc_endpoints(&mut threats);
    scan_kernel_cheat_drivers(&mut threats);

    if let Some(pid) = player_pid {
        scan_roblox_memory_handles(pid, &mut threats);
        scan_layered_esp_overlays(pid, &mut threats);
        scan_roblox_unbacked_memory(pid, &mut threats);
        scan_roblox_module_integrity(pid, &mut threats);
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

fn get_pipe_server_pid(pipe_path: &str) -> Option<u32> {
    let wide_path = wide_null(pipe_path);
    let file_handle = unsafe {
        CreateFileW(
            wide_path.as_ptr(),
            FILE_READ_ATTRIBUTES,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            std::ptr::null(),
            OPEN_EXISTING,
            0,
            std::ptr::null_mut(),
        )
    };

    if file_handle == INVALID_HANDLE_VALUE {
        return None;
    }
    let file_handle = OwnedHandle(file_handle);

    let mut server_pid: u32 = 0;
    let res = unsafe { GetNamedPipeServerProcessId(file_handle.0, &mut server_pid) };
    if res != 0 && server_pid != 0 {
        Some(server_pid)
    } else {
        None
    }
}

fn scan_kernel_cheat_drivers(threats: &mut Vec<SecurityThreat>) {
    for &device in KERNEL_CHEAT_DEVICE_PATHS {
        let wide_path = wide_null(device);
        let handle = unsafe {
            CreateFileW(
                wide_path.as_ptr(),
                FILE_READ_ATTRIBUTES | 0x80000000,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                std::ptr::null(),
                OPEN_EXISTING,
                0,
                std::ptr::null_mut(),
            )
        };

        if handle != INVALID_HANDLE_VALUE {
            unsafe { CloseHandle(handle) };
            threats.push(SecurityThreat {
                kind: ThreatKind::KnownCheatProcess,
                name: device.to_string(),
                detail: format!(
                    "Active kernel cheat or vulnerable BYOVD driver device detected: {device}"
                ),
                pid: None,
            });
        }
    }
}

struct OverlayScanContext {
    roblox_rect: RECT,
    roblox_pid: u32,
    threats: Vec<SecurityThreat>,
}

unsafe extern "system" fn enum_overlays_proc(hwnd: HWND, lparam: LPARAM) -> i32 {
    if IsWindowVisible(hwnd) == 0 {
        return 1;
    }

    let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
    let is_layered = (ex_style & 0x00080000) != 0;
    let is_transparent = (ex_style & 0x00000020) != 0;
    let is_topmost = (ex_style & 0x00000008) != 0;
    let is_noactivate = (ex_style & 0x08000000) != 0;

    if is_layered && (is_transparent || is_topmost || is_noactivate) {
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);
        let ctx = &mut *(lparam as *mut OverlayScanContext);

        if pid != 0 && pid != 4 && pid != ctx.roblox_pid && !is_whitelisted_pid(pid) {
            let mut rect: RECT = std::mem::zeroed();
            if GetWindowRect(hwnd, &mut rect) != 0 {
                let width = (rect.right - rect.left).abs();
                let height = (rect.bottom - rect.top).abs();
                let roblox_w = (ctx.roblox_rect.right - ctx.roblox_rect.left).abs();
                let roblox_h = (ctx.roblox_rect.bottom - ctx.roblox_rect.top).abs();

                if width >= 150 && height >= 150 {
                    let intersects = rect.left < ctx.roblox_rect.right
                        && rect.right > ctx.roblox_rect.left
                        && rect.top < ctx.roblox_rect.bottom
                        && rect.bottom > ctx.roblox_rect.top;

                    let size_match =
                        (width - roblox_w).abs() <= 20 && (height - roblox_h).abs() <= 20;

                    if intersects || size_match {
                        let proc_name =
                            get_process_name_by_pid(pid).unwrap_or_else(|| "Unknown".into());
                        ctx.threats.push(SecurityThreat {
                            kind: ThreatKind::KnownCheatProcess,
                            name: format!("ESP Overlay ({proc_name})"),
                            detail: format!(
                                "Transparent click-through ESP overlay window detected: {proc_name} (PID {pid}, Style: {ex_style:#x})"
                            ),
                            pid: Some(pid),
                        });
                    }
                }
            }
        }
    }

    1
}

struct RobloxWindowContext {
    target_pid: u32,
    found_hwnd: Option<HWND>,
}

unsafe extern "system" fn enum_roblox_window_proc(hwnd: HWND, lparam: LPARAM) -> i32 {
    if IsWindowVisible(hwnd) == 0 {
        return 1;
    }
    let mut pid: u32 = 0;
    GetWindowThreadProcessId(hwnd, &mut pid);
    let ctx = &mut *(lparam as *mut RobloxWindowContext);
    if pid == ctx.target_pid {
        let mut rect: RECT = std::mem::zeroed();
        if GetWindowRect(hwnd, &mut rect) != 0 {
            let width = (rect.right - rect.left).abs();
            let height = (rect.bottom - rect.top).abs();
            if width > 100 && height > 100 {
                ctx.found_hwnd = Some(hwnd);
                return 0;
            }
        }
    }
    1
}

fn find_roblox_main_window(pid: u32) -> Option<HWND> {
    let mut ctx = RobloxWindowContext {
        target_pid: pid,
        found_hwnd: None,
    };
    unsafe {
        EnumWindows(Some(enum_roblox_window_proc), &mut ctx as *mut _ as LPARAM);
    }
    ctx.found_hwnd
}

fn scan_layered_esp_overlays(roblox_pid: u32, threats: &mut Vec<SecurityThreat>) {
    let roblox_hwnd = find_roblox_main_window(roblox_pid);
    let mut roblox_rect: RECT = unsafe { std::mem::zeroed() };

    if let Some(hwnd) = roblox_hwnd {
        unsafe {
            if GetWindowRect(hwnd, &mut roblox_rect) == 0 {
                return;
            }
        }
    } else {
        return;
    }

    let mut context = OverlayScanContext {
        roblox_rect,
        roblox_pid,
        threats: Vec::new(),
    };

    unsafe {
        EnumWindows(Some(enum_overlays_proc), &mut context as *mut _ as LPARAM);
    }

    threats.extend(context.threats);
}

fn scan_roblox_unbacked_memory(roblox_pid: u32, threats: &mut Vec<SecurityThreat>) {
    let roblox_handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, roblox_pid) };
    if roblox_handle.is_null() {
        return;
    }
    let roblox_handle = OwnedHandle(roblox_handle);

    let mut address: usize = 0x10000;
    let mut mbi: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
    let mbi_size = std::mem::size_of::<MEMORY_BASIC_INFORMATION>();
    let mut unbacked_regions = 0;

    while address < 0x7FFFFFFF0000 {
        let queried = unsafe {
            VirtualQueryEx(
                roblox_handle.0,
                address as *const std::ffi::c_void,
                &mut mbi,
                mbi_size,
            )
        };

        if queried == 0 {
            break;
        }

        let is_executable = (mbi.Protect
            & (PAGE_EXECUTE_READWRITE | PAGE_EXECUTE_READ | PAGE_EXECUTE | PAGE_EXECUTE_WRITECOPY))
            != 0;

        if mbi.State == MEM_COMMIT
            && mbi.Type == MEM_PRIVATE
            && is_executable
            && mbi.RegionSize >= 4096
        {
            unbacked_regions += 1;
            if unbacked_regions == 1 {
                threats.push(SecurityThreat {
                    kind: ThreatKind::KnownCheatProcess,
                    name: "Unbacked Executable Memory in Roblox".into(),
                    detail: format!(
                        "Manual-mapped injector code or unbacked executable hook memory detected in Roblox at address {:#x} (Size: {} bytes)",
                        mbi.BaseAddress as usize,
                        mbi.RegionSize
                    ),
                    pid: Some(roblox_pid),
                });
            }
        }

        let next = (mbi.BaseAddress as usize).saturating_add(mbi.RegionSize);
        if next <= address {
            break;
        }
        address = next;
    }
}

type ReadProcessMemoryFn = unsafe extern "system" fn(
    h_process: HANDLE,
    lp_base_address: *const std::ffi::c_void,
    lp_buffer: *mut std::ffi::c_void,
    n_size: usize,
    lp_number_of_bytes_read: *mut usize,
) -> i32;

fn parse_pe_text_section(bytes: &[u8]) -> Option<(u32, u32, u32, u32)> {
    if bytes.len() < 0x40 {
        return None;
    }
    if bytes[0] != b'M' || bytes[1] != b'Z' {
        return None;
    }
    let e_lfanew =
        u32::from_le_bytes([bytes[0x3C], bytes[0x3D], bytes[0x3E], bytes[0x3F]]) as usize;
    if e_lfanew + 0x18 > bytes.len() {
        return None;
    }
    if &bytes[e_lfanew..e_lfanew + 4] != b"PE\0\0" {
        return None;
    }

    let num_sections = u16::from_le_bytes([bytes[e_lfanew + 6], bytes[e_lfanew + 7]]) as usize;
    let size_opt_header = u16::from_le_bytes([bytes[e_lfanew + 20], bytes[e_lfanew + 21]]) as usize;
    let section_table_offset = e_lfanew + 24 + size_opt_header;

    for i in 0..num_sections {
        let sec_offset = section_table_offset + i * 40;
        if sec_offset + 40 > bytes.len() {
            break;
        }
        let sec_name = &bytes[sec_offset..sec_offset + 8];
        let name_trimmed = sec_name.split(|&b| b == 0).next().unwrap_or(sec_name);
        if name_trimmed == b".text" {
            let virtual_size = u32::from_le_bytes([
                bytes[sec_offset + 8],
                bytes[sec_offset + 9],
                bytes[sec_offset + 10],
                bytes[sec_offset + 11],
            ]);
            let virtual_address = u32::from_le_bytes([
                bytes[sec_offset + 12],
                bytes[sec_offset + 13],
                bytes[sec_offset + 14],
                bytes[sec_offset + 15],
            ]);
            let size_of_raw_data = u32::from_le_bytes([
                bytes[sec_offset + 16],
                bytes[sec_offset + 17],
                bytes[sec_offset + 18],
                bytes[sec_offset + 19],
            ]);
            let pointer_to_raw_data = u32::from_le_bytes([
                bytes[sec_offset + 20],
                bytes[sec_offset + 21],
                bytes[sec_offset + 22],
                bytes[sec_offset + 23],
            ]);
            return Some((
                virtual_address,
                virtual_size,
                pointer_to_raw_data,
                size_of_raw_data,
            ));
        }
    }
    None
}

fn scan_roblox_module_integrity(roblox_pid: u32, threats: &mut Vec<SecurityThreat>) {
    let snapshot =
        unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, roblox_pid) };
    if snapshot == INVALID_HANDLE_VALUE {
        return;
    }
    let snapshot = OwnedHandle(snapshot);

    let h_process =
        unsafe { OpenProcess(PROCESS_VM_READ | PROCESS_QUERY_INFORMATION, 0, roblox_pid) };
    if h_process.is_null() {
        return;
    }
    let h_process = OwnedHandle(h_process);

    let kernel32_name = wide_null("kernel32.dll");
    let h_kernel32 = unsafe { GetModuleHandleW(kernel32_name.as_ptr()) };
    if h_kernel32.is_null() {
        return;
    }
    let rpm_proc = unsafe { GetProcAddress(h_kernel32, c"ReadProcessMemory".as_ptr() as _) };
    let Some(rpm_proc_addr) = rpm_proc else {
        return;
    };
    let read_process_memory: ReadProcessMemoryFn = unsafe { std::mem::transmute(rpm_proc_addr) };

    let mut entry = MODULEENTRY32W {
        dwSize: std::mem::size_of::<MODULEENTRY32W>() as u32,
        ..unsafe { std::mem::zeroed() }
    };

    let mut ok = unsafe { Module32FirstW(snapshot.0, &mut entry) };
    while ok != 0 {
        let mod_name = from_wide_nul(&entry.szModule);
        let mod_path = from_wide_nul(&entry.szExePath);
        let mod_name_lower = mod_name.to_ascii_lowercase();
        let mod_path_lower = mod_path.to_ascii_lowercase();

        let is_trusted_dir = mod_path_lower.starts_with(r"c:\windows\system32\")
            || mod_path_lower.starts_with(r"c:\windows\syswow64\")
            || mod_path_lower.starts_with(r"c:\windows\winsxs\")
            || mod_path_lower.contains(r"\rustblox\data\versions\")
            || mod_path_lower.contains(r"\roblox\versions\")
            || mod_path_lower.contains(r"\discord")
            || mod_path_lower.contains(r"\nvidia")
            || mod_path_lower.contains(r"\obs-studio");

        if !is_trusted_dir
            && (mod_path_lower.contains(r"\temp\")
                || mod_path_lower.contains(r"\downloads\")
                || mod_path_lower.contains(r"\appdata\local\temp\"))
        {
            threats.push(SecurityThreat {
                kind: ThreatKind::InjectedModule,
                name: mod_name.clone(),
                detail: format!(
                    "Rogue internal DLL injected into Roblox from suspicious directory: {mod_path}"
                ),
                pid: Some(roblox_pid),
            });
        }

        if CRITICAL_INTEGRITY_DLLS.contains(&mod_name_lower.as_str()) {
            if let Ok(disk_bytes) = std::fs::read(&mod_path) {
                if let Some((v_addr, v_size, raw_ptr, raw_size)) =
                    parse_pe_text_section(&disk_bytes)
                {
                    let compare_len = std::cmp::min(v_size as usize, raw_size as usize);
                    let inspect_len = std::cmp::min(compare_len, 4096);

                    if inspect_len > 0 && (raw_ptr as usize + inspect_len) <= disk_bytes.len() {
                        let base_addr = entry.modBaseAddr as usize;
                        let target_mem_addr = base_addr + v_addr as usize;
                        let mut mem_buf = vec![0u8; inspect_len];
                        let mut bytes_read: usize = 0;

                        let read_ok = unsafe {
                            read_process_memory(
                                h_process.0,
                                target_mem_addr as *const std::ffi::c_void,
                                mem_buf.as_mut_ptr() as *mut std::ffi::c_void,
                                inspect_len,
                                &mut bytes_read,
                            )
                        };

                        if read_ok != 0 && bytes_read == inspect_len {
                            let disk_slice =
                                &disk_bytes[raw_ptr as usize..raw_ptr as usize + inspect_len];
                            let mut mismatches = 0;
                            let mut has_hook_jump = false;

                            for i in 0..inspect_len {
                                if mem_buf[i] != disk_slice[i] {
                                    mismatches += 1;
                                    if mem_buf[i] == 0xE9
                                        || (i + 1 < inspect_len
                                            && mem_buf[i] == 0xFF
                                            && mem_buf[i + 1] == 0x25)
                                    {
                                        has_hook_jump = true;
                                    }
                                }
                            }

                            if has_hook_jump || mismatches > 64 {
                                threats.push(SecurityThreat {
                                    kind: if mismatches > 64 {
                                        ThreatKind::ModuleStomping
                                    } else {
                                        ThreatKind::HookTampering
                                    },
                                    name: mod_name.clone(),
                                    detail: format!(
                                        "Code section integrity violation in {mod_name} (Base: {base_addr:#x}, Mismatched bytes: {mismatches})"
                                    ),
                                    pid: Some(roblox_pid),
                                });
                            }
                        }
                    }
                }
            }
        }

        ok = unsafe { Module32NextW(snapshot.0, &mut entry) };
    }
}

#[repr(C)]
struct MIB_TCPROW_OWNER_PID {
    dw_state: u32,
    dw_local_addr: u32,
    dw_local_port: u32,
    dw_remote_addr: u32,
    dw_remote_port: u32,
    dw_owning_pid: u32,
}

type GetExtendedTcpTableFn = unsafe extern "system" fn(
    p_tcp_table: *mut std::ffi::c_void,
    pdw_size: *mut u32,
    b_order: i32,
    ul_af: u32,
    table_class: i32,
    reserved: u32,
) -> u32;

fn scan_unauthorized_ipc_endpoints(threats: &mut Vec<SecurityThreat>) {
    let iphlpapi_name = wide_null("iphlpapi.dll");
    let h_iphlpapi = unsafe { LoadLibraryW(iphlpapi_name.as_ptr()) };
    if h_iphlpapi.is_null() {
        return;
    }
    let h_guard = OwnedHandle(h_iphlpapi);

    let proc = unsafe { GetProcAddress(h_guard.0, c"GetExtendedTcpTable".as_ptr() as _) };
    let Some(proc_addr) = proc else {
        return;
    };
    let get_extended_tcp_table: GetExtendedTcpTableFn = unsafe { std::mem::transmute(proc_addr) };

    let mut size: u32 = 0;
    let _ = unsafe { get_extended_tcp_table(std::ptr::null_mut(), &mut size, 0, 2, 5, 0) };

    if size == 0 {
        return;
    }

    let mut buffer: Vec<u8> = vec![0; size as usize];
    let res = unsafe {
        get_extended_tcp_table(
            buffer.as_mut_ptr() as *mut std::ffi::c_void,
            &mut size,
            0,
            2,
            5,
            0,
        )
    };

    if res != 0 || buffer.len() < 4 {
        return;
    }

    let num_entries = unsafe { *(buffer.as_ptr() as *const u32) } as usize;
    let row_size = std::mem::size_of::<MIB_TCPROW_OWNER_PID>();
    let table_ptr = unsafe { buffer.as_ptr().add(4) as *const MIB_TCPROW_OWNER_PID };

    for i in 0..num_entries {
        if 4 + (i + 1) * row_size > buffer.len() {
            break;
        }
        let row = unsafe { &*table_ptr.add(i) };
        let port = u16::from_be((row.dw_local_port & 0xFFFF) as u16);
        let pid = row.dw_owning_pid;

        if row.dw_state == 2
            && (row.dw_local_addr == 0x0100007F || row.dw_local_addr == 0)
            && pid != 0
            && pid != 4
            && !is_whitelisted_pid(pid)
        {
            let proc_name = get_process_name_by_pid(pid).unwrap_or_else(|| "Unknown".into());
            let proc_path = get_process_image_path(pid);

            if !is_valid_whitelisted_path(&proc_name, proc_path.as_deref()) {
                threats.push(SecurityThreat {
                    kind: ThreatKind::UnauthorizedIpcServer,
                    name: format!("Local IPC Server ({proc_name})"),
                    detail: format!(
                        "Unauthorized script executor IPC/WebSocket endpoint listening on 127.0.0.1:{port} (PID {pid}, {proc_name})"
                    ),
                    pid: Some(pid),
                });
            }
        }
    }
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
            let full_pipe_name = format!(r"\\.\pipe\{pipe_name}");
            let server_pid = get_pipe_server_pid(&full_pipe_name);

            threats.push(SecurityThreat {
                kind: ThreatKind::ScriptExecutorPipe,
                name: full_pipe_name,
                detail: "Active script executor communication pipe detected".into(),
                pid: server_pid,
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

type SetSecurityInfoFn = unsafe extern "system" fn(
    handle: HANDLE,
    object_type: u32,
    security_info: u32,
    psid_owner: *const std::ffi::c_void,
    psid_group: *const std::ffi::c_void,
    p_dacl: *const std::ffi::c_void,
    p_sacl: *const std::ffi::c_void,
) -> u32;

pub fn harden_watchdog_process() {
    let advapi_name = wide_null("advapi32.dll");
    let h_advapi = unsafe { LoadLibraryW(advapi_name.as_ptr()) };
    if h_advapi.is_null() {
        return;
    }
    let h_guard = OwnedHandle(h_advapi);

    let proc = unsafe { GetProcAddress(h_guard.0, c"SetSecurityInfo".as_ptr() as _) };
    let Some(proc_addr) = proc else {
        return;
    };
    let set_security_info: SetSecurityInfoFn = unsafe { std::mem::transmute(proc_addr) };

    let current_proc = unsafe { GetCurrentProcess() };
    unsafe {
        let _ = set_security_info(
            current_proc,
            6,
            0x00000004 | 0x80000000,
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
        );
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
    harden_watchdog_process();

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
                && last_scan.elapsed() >= std::time::Duration::from_millis(800)
            {
                let report = scan_security(Some(pid), Some(&install_dir));
                for threat in report.threats {
                    if let Some(threat_pid) = threat.pid {
                        if threat_pid != pid {
                            terminate_threat_pid(threat_pid);
                        } else {
                            if !roblox_handle.is_null() {
                                let term_handle = OpenProcess(PROCESS_TERMINATE, 0, pid);
                                if !term_handle.is_null() {
                                    TerminateProcess(term_handle, 1);
                                    CloseHandle(term_handle);
                                }
                            }
                        }
                    }
                }
                last_scan = std::time::Instant::now();
            }

            std::thread::sleep(std::time::Duration::from_millis(150));
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

    #[test]
    fn test_matcha_and_driver_detection() {
        assert!(CHEAT_PROCESS_KEYWORDS.contains(&"matcha"));
        assert!(CHEAT_PROCESS_KEYWORDS.contains(&"matchahub"));
        assert!(CHEAT_PROCESS_KEYWORDS.contains(&"matchav2"));
        assert!(CHEAT_PROCESS_KEYWORDS.contains(&"aimmy"));
        assert!(CHEAT_WINDOW_KEYWORDS.contains(&"matcha external"));
        assert!(CHEAT_WINDOW_KEYWORDS.contains(&"matcha v2"));
        assert!(EXECUTOR_PIPE_PREFIXES.contains(&"matcha"));
        assert!(KERNEL_CHEAT_DEVICE_PATHS.contains(&r"\\.\Matcha"));
        assert!(KERNEL_CHEAT_DEVICE_PATHS.contains(&r"\\.\MatchaDriver"));
        assert!(KERNEL_CHEAT_DEVICE_PATHS.contains(&r"\\.\GIO"));
        assert!(KERNEL_CHEAT_DEVICE_PATHS.contains(&r"\\.\gdrv"));
        assert!(KERNEL_CHEAT_DEVICE_PATHS.contains(&r"\\.\interception"));
    }

    #[test]
    fn test_threat_kind_labels() {
        assert_eq!(
            ThreatKind::ModuleStomping.label(),
            "Module stomping / in-memory patch"
        );
        assert_eq!(
            ThreatKind::HookTampering.label(),
            "Hook tampering / detours"
        );
        assert_eq!(
            ThreatKind::UnauthorizedIpcServer.label(),
            "Unauthorized script executor IPC/WebSocket endpoint"
        );
    }

    #[test]
    fn test_pe_parser_handles_invalid_data() {
        assert!(parse_pe_text_section(&[]).is_none());
        assert!(parse_pe_text_section(&[0u8; 100]).is_none());
    }
}

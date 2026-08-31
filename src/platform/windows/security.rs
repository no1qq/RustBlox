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
    CreateFileW, FindClose, FindFirstFileW, FindNextFileW, FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE,
    FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING, WIN32_FIND_DATAW,
};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress, LoadLibraryW};
use windows_sys::Win32::System::Pipes::GetNamedPipeServerProcessId;
use windows_sys::Win32::System::Threading::{
    GetCurrentProcess, GetExitCodeProcess, GetProcessId, OpenProcess, OpenProcessToken,
    QueryFullProcessImageNameW, TerminateProcess, PROCESS_DUP_HANDLE,
    PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE,
};
use windows_sys::Win32::UI::Shell::{
    ShellExecuteExW, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateIconFromResourceEx, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW,
    EnumWindows, GetSystemMetrics, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId,
    IsWindowVisible, LoadIconW, PeekMessageW, RegisterClassW, ShowWindow, TranslateMessage, HICON,
    IDI_SHIELD, LR_DEFAULTCOLOR, MSG, PM_REMOVE, SM_CXSMICON, SW_HIDE, SW_SHOWNA, WNDCLASSW,
    WS_OVERLAPPED,
};

use crate::platform::{SecurityReport, SecurityThreat, ThreatKind};

#[allow(dead_code)]
const THEWATCHER_ICO: &[u8] = include_bytes!("../../../assets/thewatcher.ico");

const CHEAT_PROCESS_KEYWORDS: &[&str] = &[
    "matrixhub",
    "matchahub",
    "matcha_external",
    "solara",
    "celery",
    "krampus",
    "xenos",
    "synapsez",
    "extremeinjector",
    "cheatengine",
    "x64dbg",
    "x32dbg",
    "ida64",
    "scylla",
    "processhacker",
    "systeminformer",
    "httpdebugger",
    "reclass",
];

const CHEAT_WINDOW_KEYWORDS: &[&str] = &[
    "matrix hub",
    "matrix external",
    "matrix",
    "matcha external",
    "matchahub",
    "matcha",
    "solara executor",
    "solara",
    "celery executor",
    "celery",
    "krampus",
    "xenos injector",
    "xenos",
    "extreme injector",
    "cheat engine",
    "x64dbg",
    "x32dbg",
    "ida pro",
    "process hacker",
    "system informer",
    "http debugger",
    "reclass.net",
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
    "synapsez",
    "synapse_pipe",
    "potassium_pipe",
    "celery_pipe",
    "solara_pipe",
    "krnl_pipe",
    "fluxus_pipe",
    "valyse_pipe",
    "wearedevs_pipe",
    "matcha_pipe",
    "matrix_pipe",
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
    scan_kernel_cheat_drivers(&mut threats);

    if let Some(pid) = player_pid {
        scan_roblox_memory_handles(pid, &mut threats);
        scan_roblox_unbacked_executable_memory(pid, &mut threats);
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
    "dorion.exe",
    "dorion-bin.exe",
    "medal.exe",
    "medalencoder.exe",
    "medalservice.exe",
    "node.exe",
    "electron.exe",
    "antigravity.exe",
    "agy.exe",
    "chrome.exe",
    "msedge.exe",
    "firefox.exe",
    "brave.exe",
    "opera.exe",
    "operagx.exe",
    "vivaldi.exe",
    "code.exe",
    "cursor.exe",
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
    "obs64.exe",
    "obs32.exe",
];

const AUTHORIZED_HANDLE_HOLDERS: &[&str] = &[
    "robloxcrashhandler.exe",
    "rustblox.exe",
    "medalencoder.exe",
    "medal.exe",
    "medalservice.exe",
    "dorion.exe",
    "dorion-bin.exe",
    "discord.exe",
    "discordcanary.exe",
    "discordptb.exe",
    "obs64.exe",
    "obs32.exe",
    "overwolfhelper64.exe",
    "nvidia share.exe",
    "nvcontainer.exe",
    "radeonsofware.exe",
    "amdow.exe",
    "steam.exe",
    "steamwebhelper.exe",
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

    if file_name == "robloxplayerbeta.exe" || file_name == "robloxstudiobeta.exe" {
        return path_lower.contains(r"\rustblox\data\versions\")
            || path_lower.contains(r"\rustblox\data\watcher\")
            || path_lower.contains(r"\roblox\versions\")
            || path_lower.contains(r"\program files (x86)\roblox\versions\")
            || path_lower.contains(r"\program files\roblox\versions\");
    }

    if path_lower.contains(r"\temp\") || path_lower.contains("matrix") {
        return false;
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

    if let Some(p) = path {
        let p_lower = p.to_ascii_lowercase();
        if p_lower.contains(r"\temp\") {
            return false;
        }
        if file_name == "robloxplayerbeta.exe" && p_lower.contains(r"\rustblox\data\watcher\") {
            return true;
        }
    }

    if !AUTHORIZED_HANDLE_HOLDERS.contains(&file_name) {
        return false;
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
    enable_debug_privilege();
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
                            DuplicateHandle(
                                holding_handle,
                                entry.handle_value as HANDLE,
                                0 as HANDLE,
                                std::ptr::null_mut(),
                                0,
                                0,
                                1,
                            );

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

fn scan_roblox_unbacked_executable_memory(target_pid: u32, threats: &mut Vec<SecurityThreat>) {
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, target_pid) };
    if handle.is_null() {
        return;
    }
    let handle_guard = OwnedHandle(handle);

    let kernel32_name = wide_null("kernel32.dll");
    let h_kernel32 = unsafe { LoadLibraryW(kernel32_name.as_ptr()) };
    if h_kernel32.is_null() {
        return;
    }
    let h_kernel_guard = OwnedHandle(h_kernel32);

    #[repr(C)]
    struct MEM_BASIC_INFO {
        base_address: *mut std::ffi::c_void,
        allocation_base: *mut std::ffi::c_void,
        allocation_protect: u32,
        partition_id: u16,
        region_size: usize,
        state: u32,
        protect: u32,
        type_: u32,
    }

    type VirtualQueryExFn = unsafe extern "system" fn(
        h_process: HANDLE,
        lp_address: *const std::ffi::c_void,
        lp_buffer: *mut MEM_BASIC_INFO,
        dw_length: usize,
    ) -> usize;

    let vq_proc = unsafe { GetProcAddress(h_kernel_guard.0, c"VirtualQueryEx".as_ptr() as _) };
    let Some(proc_addr) = vq_proc else {
        return;
    };
    let virtual_query_ex: VirtualQueryExFn = unsafe { std::mem::transmute(proc_addr) };

    let mut address: usize = 0x10000;
    let mut detected_count = 0;
    let max_address: usize = 0x7FFF_FFFF_FFFF;

    while address < max_address && detected_count < 10 {
        let mut mbi: MEM_BASIC_INFO = unsafe { std::mem::zeroed() };
        let bytes = unsafe {
            virtual_query_ex(
                handle_guard.0,
                address as *const std::ffi::c_void,
                &mut mbi,
                std::mem::size_of::<MEM_BASIC_INFO>(),
            )
        };

        if bytes == 0 {
            break;
        }

        let is_committed = mbi.state == 0x1000;
        let is_private = mbi.type_ == 0x20000;
        let is_executable = (mbi.protect & (0x10 | 0x20 | 0x40 | 0x80)) != 0;
        let is_guard_or_noaccess = (mbi.protect & (0x01 | 0x100)) != 0;

        if is_committed && is_private && is_executable && !is_guard_or_noaccess {
            detected_count += 1;
            threats.push(SecurityThreat {
                kind: ThreatKind::ModuleStomping,
                name: format!("Unbacked Memory (0x{:X})", mbi.base_address as usize),
                detail: format!(
                    "Unbacked executable memory page detected in Roblox address space: 0x{:X} (size {} KB, protect 0x{:X})",
                    mbi.base_address as usize,
                    mbi.region_size / 1024,
                    mbi.protect
                ),
                pid: Some(target_pid),
            });
        }

        let next_addr = (mbi.base_address as usize).saturating_add(mbi.region_size);
        if next_addr <= address {
            break;
        }
        address = next_addr;
    }
}

pub fn terminate_threat_pid(pid: u32) -> bool {
    if pid == 0 || pid == 4 || is_whitelisted_pid(pid) {
        return false;
    }
    let Some(name) = get_process_name_by_pid(pid) else {
        return false;
    };
    let proc_path = get_process_image_path(pid);
    if is_authorized_roblox_handle_holder(&name, proc_path.as_deref())
        && is_valid_whitelisted_path(&name, proc_path.as_deref())
    {
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
                let is_roblox_impostor = (name_lower == "robloxplayerbeta.exe"
                    || name_lower == "robloxstudiobeta.exe")
                    && !is_trusted;

                if is_cheat || is_system || is_roblox_impostor {
                    threats.push(SecurityThreat {
                        kind: ThreatKind::KnownCheatProcess,
                        name: name.clone(),
                        detail: if is_roblox_impostor {
                            format!(
                                "Impostor cheat process masquerading as Roblox: {name} (PID {pid}, Path: {})",
                                proc_path.as_deref().unwrap_or("Unknown")
                            )
                        } else if is_system {
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

#[allow(dead_code)]
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

type ConvertStringSecurityDescriptorToSecurityDescriptorWFn = unsafe extern "system" fn(
    string_security_descriptor: *const u16,
    string_sd_revision: u32,
    security_descriptor: *mut *mut std::ffi::c_void,
    security_descriptor_size: *mut u32,
) -> i32;

type SetKernelObjectSecurityFn = unsafe extern "system" fn(
    handle: HANDLE,
    security_information: u32,
    security_descriptor: *const std::ffi::c_void,
) -> i32;

type LocalFreeFn = unsafe extern "system" fn(hmem: *mut std::ffi::c_void) -> *mut std::ffi::c_void;

type CreateJobObjectWFn = unsafe extern "system" fn(
    lp_job_attributes: *const std::ffi::c_void,
    lp_name: *const u16,
) -> HANDLE;

type SetInformationJobObjectFn = unsafe extern "system" fn(
    h_job: HANDLE,
    job_object_info_class: i32,
    lp_job_object_info: *const std::ffi::c_void,
    cb_job_object_info_length: u32,
) -> i32;

type AssignProcessToJobObjectFn =
    unsafe extern "system" fn(h_job: HANDLE, h_process: HANDLE) -> i32;

type HandlerRoutine = unsafe extern "system" fn(ctrl_type: u32) -> i32;

type SetConsoleCtrlHandlerFn =
    unsafe extern "system" fn(handler_routine: Option<HandlerRoutine>, add: i32) -> i32;

#[repr(C)]
struct JOBOBJECT_BASIC_LIMIT_INFORMATION {
    per_process_user_time_limit: i64,
    per_job_user_time_limit: i64,
    limit_flags: u32,
    minimum_working_set_size: usize,
    maximum_working_set_size: usize,
    active_process_limit: u32,
    affinity: usize,
    priority_class: u32,
    scheduling_class: u32,
}

#[repr(C)]
struct IO_COUNTERS {
    read_operation_count: u64,
    write_operation_count: u64,
    other_operation_count: u64,
    read_transfer_count: u64,
    write_transfer_count: u64,
    other_transfer_count: u64,
}

#[repr(C)]
struct JOBOBJECT_EXTENDED_LIMIT_INFORMATION {
    basic_limit_information: JOBOBJECT_BASIC_LIMIT_INFORMATION,
    io_info: IO_COUNTERS,
    process_memory_limit: usize,
    job_memory_limit: usize,
    peak_process_memory_limit: usize,
    peak_job_memory_limit: usize,
}

static WATCHDOG_ROBLOX_PID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

pub fn force_terminate_roblox_pid(pid: u32) -> bool {
    if pid == 0 || pid == 4 {
        return false;
    }
    let handle = unsafe { OpenProcess(PROCESS_TERMINATE, 0, pid) };
    if handle.is_null() {
        return false;
    }
    let handle = OwnedHandle(handle);
    unsafe { TerminateProcess(handle.0, 1) != 0 }
}

unsafe extern "system" fn watcher_ctrl_handler(_ctrl_type: u32) -> i32 {
    let pid = WATCHDOG_ROBLOX_PID.load(std::sync::atomic::Ordering::SeqCst);
    if pid != 0 {
        force_terminate_roblox_pid(pid);
    }
    0
}

fn register_ctrl_handler() {
    let kernel32_name = wide_null("kernel32.dll");
    let h_kernel32 = unsafe { LoadLibraryW(kernel32_name.as_ptr()) };
    if h_kernel32.is_null() {
        return;
    }
    let h_guard = OwnedHandle(h_kernel32);
    if let Some(proc) = unsafe { GetProcAddress(h_guard.0, c"SetConsoleCtrlHandler".as_ptr() as _) }
    {
        let set_ctrl_fn: SetConsoleCtrlHandlerFn = unsafe { std::mem::transmute(proc) };
        unsafe { set_ctrl_fn(Some(watcher_ctrl_handler), 1) };
    }
}

pub fn create_failclosed_job_object() -> Option<HANDLE> {
    let kernel32_name = wide_null("kernel32.dll");
    let h_kernel32 = unsafe { LoadLibraryW(kernel32_name.as_ptr()) };
    if h_kernel32.is_null() {
        return None;
    }
    let h_guard = OwnedHandle(h_kernel32);

    let create_job = unsafe { GetProcAddress(h_guard.0, c"CreateJobObjectW".as_ptr() as _) }?;
    let set_info = unsafe { GetProcAddress(h_guard.0, c"SetInformationJobObject".as_ptr() as _) }?;

    let create_job_fn: CreateJobObjectWFn = unsafe { std::mem::transmute(create_job) };
    let set_info_fn: SetInformationJobObjectFn = unsafe { std::mem::transmute(set_info) };

    let job_handle = unsafe { create_job_fn(std::ptr::null(), std::ptr::null()) };
    if job_handle.is_null() || job_handle == INVALID_HANDLE_VALUE {
        return None;
    }

    let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
    info.basic_limit_information.limit_flags = 0x00002000 | 0x00001000;

    let ok = unsafe {
        set_info_fn(
            job_handle,
            9,
            &info as *const _ as *const std::ffi::c_void,
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        )
    };

    if ok != 0 {
        Some(job_handle)
    } else {
        unsafe { CloseHandle(job_handle) };
        None
    }
}

pub fn assign_roblox_to_job(job_handle: HANDLE, roblox_pid: u32) -> bool {
    if job_handle.is_null() || job_handle == INVALID_HANDLE_VALUE || roblox_pid == 0 {
        return false;
    }
    let kernel32_name = wide_null("kernel32.dll");
    let h_kernel32 = unsafe { LoadLibraryW(kernel32_name.as_ptr()) };
    if h_kernel32.is_null() {
        return false;
    }
    let h_guard = OwnedHandle(h_kernel32);

    let assign_proc =
        match unsafe { GetProcAddress(h_guard.0, c"AssignProcessToJobObject".as_ptr() as _) } {
            Some(addr) => addr,
            None => return false,
        };
    let assign_fn: AssignProcessToJobObjectFn = unsafe { std::mem::transmute(assign_proc) };

    let roblox_handle = unsafe { OpenProcess(0x00000200 | PROCESS_TERMINATE, 0, roblox_pid) };
    if roblox_handle.is_null() {
        return false;
    }
    let roblox_guard = OwnedHandle(roblox_handle);

    unsafe { assign_fn(job_handle, roblox_guard.0) != 0 }
}

static LAUNCHER_JOB: std::sync::Mutex<Option<isize>> = std::sync::Mutex::new(None);

pub fn bind_launcher_job_roblox(roblox_pid: u32) {
    if roblox_pid == 0 {
        return;
    }
    let mut guard = match LAUNCHER_JOB.lock() {
        Ok(g) => g,
        Err(_) => return,
    };
    if guard.is_none() {
        *guard = create_failclosed_job_object().map(|h| h as isize);
    }
    if let Some(job) = *guard {
        assign_roblox_to_job(job as HANDLE, roblox_pid);
    }
    WATCHDOG_ROBLOX_PID.store(roblox_pid, std::sync::atomic::Ordering::SeqCst);
    register_ctrl_handler();
}

pub fn harden_watchdog_process() {
    let advapi_name = wide_null("advapi32.dll");
    let h_advapi = unsafe { LoadLibraryW(advapi_name.as_ptr()) };
    if h_advapi.is_null() {
        return;
    }
    let h_advapi_guard = OwnedHandle(h_advapi);

    let kernel32_name = wide_null("kernel32.dll");
    let h_kernel32 = unsafe { LoadLibraryW(kernel32_name.as_ptr()) };
    let h_kernel32_guard = if !h_kernel32.is_null() {
        Some(OwnedHandle(h_kernel32))
    } else {
        None
    };

    let proc_convert = unsafe {
        GetProcAddress(
            h_advapi_guard.0,
            c"ConvertStringSecurityDescriptorToSecurityDescriptorW".as_ptr() as _,
        )
    };
    let proc_set_sec =
        unsafe { GetProcAddress(h_advapi_guard.0, c"SetKernelObjectSecurity".as_ptr() as _) };
    let proc_local_free = if let Some(ref k) = h_kernel32_guard {
        unsafe { GetProcAddress(k.0, c"LocalFree".as_ptr() as _) }
    } else {
        None
    };

    if let (Some(conv), Some(set_sec)) = (proc_convert, proc_set_sec) {
        let convert_fn: ConvertStringSecurityDescriptorToSecurityDescriptorWFn =
            unsafe { std::mem::transmute(conv) };
        let set_sec_fn: SetKernelObjectSecurityFn = unsafe { std::mem::transmute(set_sec) };

        let sddl = wide_null(
            "D:P(D;;0x000c0829;;;WD)(D;;0x000c0829;;;BU)(D;;0x000c0829;;;BA)(D;;0x000c0829;;;IU)(A;;0x00101000;;;WD)(A;;GA;;;SY)",
        );
        let mut p_sd: *mut std::ffi::c_void = std::ptr::null_mut();
        if unsafe { convert_fn(sddl.as_ptr(), 1, &mut p_sd, std::ptr::null_mut()) } != 0
            && !p_sd.is_null()
        {
            let current_proc = unsafe { GetCurrentProcess() };
            let _ = unsafe { set_sec_fn(current_proc, 4, p_sd) };
            if let Some(lf) = proc_local_free {
                let local_free_fn: LocalFreeFn = unsafe { std::mem::transmute(lf) };
                unsafe { local_free_fn(p_sd) };
            }
        }
    }
}

#[allow(dead_code)]
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

type LoadLibraryExWFn = unsafe extern "system" fn(
    lp_lib_file_name: *const u16,
    h_file: HANDLE,
    dw_flags: u32,
) -> HANDLE;
type FreeLibraryFn = unsafe extern "system" fn(h_module: HANDLE) -> i32;
type FindResourceWFn =
    unsafe extern "system" fn(h_module: HANDLE, lp_name: *const u16, lp_type: *const u16) -> HANDLE;
type SizeofResourceFn = unsafe extern "system" fn(h_module: HANDLE, h_res_info: HANDLE) -> u32;
type LoadResourceFn = unsafe extern "system" fn(h_module: HANDLE, h_res_info: HANDLE) -> HANDLE;
type LockResourceFn = unsafe extern "system" fn(h_res_data: HANDLE) -> *const std::ffi::c_void;
type BeginUpdateResourceWFn =
    unsafe extern "system" fn(p_file_name: *const u16, b_delete_existing: i32) -> HANDLE;
type UpdateResourceWFn = unsafe extern "system" fn(
    h_update: HANDLE,
    lp_type: *const u16,
    lp_name: *const u16,
    w_language: u16,
    lp_data: *const std::ffi::c_void,
    cb: u32,
) -> i32;
type EndUpdateResourceWFn = unsafe extern "system" fn(h_update: HANDLE, f_discard: i32) -> i32;

struct ResourceReader {
    find: FindResourceWFn,
    size_of: SizeofResourceFn,
    load: LoadResourceFn,
    lock: LockResourceFn,
}

impl ResourceReader {
    fn read(&self, h_module: HANDLE, res_type: u16, res_id: u16) -> Option<Vec<u8>> {
        unsafe {
            let h_res = (self.find)(h_module, res_id as *const u16, res_type as *const u16);
            if h_res.is_null() {
                return None;
            }
            let size = (self.size_of)(h_module, h_res);
            if size == 0 {
                return None;
            }
            let h_loaded = (self.load)(h_module, h_res);
            if h_loaded.is_null() {
                return None;
            }
            let ptr = (self.lock)(h_loaded);
            if ptr.is_null() {
                return None;
            }
            Some(std::slice::from_raw_parts(ptr as *const u8, size as usize).to_vec())
        }
    }
}

#[allow(clippy::manual_dangling_ptr)]
fn patch_pe_resources(source: &Path, target: &Path) -> Option<()> {
    let source_str = source.to_str()?;
    if source_str.is_empty() {
        return None;
    }
    let target_str = target.to_str()?;
    if target_str.is_empty() {
        return None;
    }

    unsafe {
        let kernel32_name = wide_null("kernel32.dll");
        let h_kernel32 = LoadLibraryW(kernel32_name.as_ptr());
        if h_kernel32.is_null() {
            return None;
        }
        let _k_guard = OwnedHandle(h_kernel32);

        let get_proc = |name: &'static str| -> Option<unsafe extern "system" fn() -> isize> {
            GetProcAddress(h_kernel32, wide_null(name).as_ptr() as _)
        };

        let load_library_ex: LoadLibraryExWFn = std::mem::transmute::<
            unsafe extern "system" fn() -> isize,
            LoadLibraryExWFn,
        >(get_proc("LoadLibraryExW")?);
        let free_library: FreeLibraryFn = std::mem::transmute::<
            unsafe extern "system" fn() -> isize,
            FreeLibraryFn,
        >(get_proc("FreeLibrary")?);
        let begin_update: BeginUpdateResourceWFn = std::mem::transmute::<
            unsafe extern "system" fn() -> isize,
            BeginUpdateResourceWFn,
        >(get_proc("BeginUpdateResourceW")?);
        let update_resource: UpdateResourceWFn = std::mem::transmute::<
            unsafe extern "system" fn() -> isize,
            UpdateResourceWFn,
        >(get_proc("UpdateResourceW")?);
        let end_update: EndUpdateResourceWFn = std::mem::transmute::<
            unsafe extern "system" fn() -> isize,
            EndUpdateResourceWFn,
        >(get_proc("EndUpdateResourceW")?);

        let reader = ResourceReader {
            find: std::mem::transmute::<unsafe extern "system" fn() -> isize, FindResourceWFn>(
                get_proc("FindResourceW")?,
            ),
            size_of: std::mem::transmute::<unsafe extern "system" fn() -> isize, SizeofResourceFn>(
                get_proc("SizeofResource")?,
            ),
            load: std::mem::transmute::<unsafe extern "system" fn() -> isize, LoadResourceFn>(
                get_proc("LoadResource")?,
            ),
            lock: std::mem::transmute::<unsafe extern "system" fn() -> isize, LockResourceFn>(
                get_proc("LockResource")?,
            ),
        };

        let source_wide = wide_null(source_str);
        let h_source = load_library_ex(source_wide.as_ptr(), std::ptr::null_mut(), 0x00000002);
        if h_source.is_null() {
            return None;
        }

        let version_data = reader.read(h_source, 16, 1);
        let group_icon_data = reader
            .read(h_source, 14, 101)
            .or_else(|| reader.read(h_source, 14, 100))
            .or_else(|| reader.read(h_source, 14, 1))
            .or_else(|| reader.read(h_source, 14, 32512));

        let mut icon_entries: Vec<(u16, Vec<u8>)> = Vec::new();
        if let Some(ref gid) = group_icon_data {
            if gid.len() >= 6 {
                let count = u16::from_le_bytes([gid[4], gid[5]]) as usize;
                for i in 0..count {
                    let offset = 6 + i * 14;
                    if offset + 14 <= gid.len() {
                        let icon_id = u16::from_le_bytes([gid[offset + 12], gid[offset + 13]]);
                        if let Some(data) = reader.read(h_source, 3, icon_id) {
                            icon_entries.push((icon_id, data));
                        }
                    }
                }
            }
        }

        free_library(h_source);

        let target_wide = wide_null(target_str);
        let h_update = begin_update(target_wide.as_ptr(), 0);
        if h_update.is_null() {
            return None;
        }

        let lang: u16 = 0x0409;

        if let Some(ref data) = version_data {
            update_resource(
                h_update,
                16_usize as *const u16,
                1_usize as *const u16,
                lang,
                data.as_ptr() as _,
                data.len() as u32,
            );
        }

        if let Some(ref data) = group_icon_data {
            update_resource(
                h_update,
                14_usize as *const u16,
                1_usize as *const u16,
                lang,
                data.as_ptr() as _,
                data.len() as u32,
            );
        }

        for (id, data) in &icon_entries {
            update_resource(
                h_update,
                3_usize as *const u16,
                (*id as usize) as *const u16,
                lang,
                data.as_ptr() as _,
                data.len() as u32,
            );
        }

        end_update(h_update, 0);
        Some(())
    }
}

fn patch_exe_version_strings(target: &Path) {
    let Ok(mut bytes) = std::fs::read(target) else {
        return;
    };

    let needle_desc: Vec<u8> = "RustBlox desktop client for Roblox"
        .encode_utf16()
        .flat_map(|u| u.to_le_bytes())
        .collect();

    let repl_desc_str = "Roblox Game Client\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0";
    let repl_desc_utf16: Vec<u16> = repl_desc_str.encode_utf16().collect();
    let repl_desc: Vec<u8> = repl_desc_utf16
        .iter()
        .take(needle_desc.len() / 2)
        .flat_map(|u| u.to_le_bytes())
        .collect();

    if needle_desc.len() == repl_desc.len() {
        let mut pos = 0;
        while let Some(idx) = bytes[pos..]
            .windows(needle_desc.len())
            .position(|w| w == needle_desc)
        {
            let match_idx = pos + idx;
            bytes[match_idx..match_idx + needle_desc.len()].copy_from_slice(&repl_desc);
            pos = match_idx + needle_desc.len();
        }
    }

    let needle_prod: Vec<u8> = "RustBlox"
        .encode_utf16()
        .flat_map(|u| u.to_le_bytes())
        .collect();
    let repl_prod_str = "Roblox\0\0";
    let repl_prod_utf16: Vec<u16> = repl_prod_str.encode_utf16().collect();
    let repl_prod: Vec<u8> = repl_prod_utf16
        .iter()
        .take(needle_prod.len() / 2)
        .flat_map(|u| u.to_le_bytes())
        .collect();

    if needle_prod.len() == repl_prod.len() {
        let mut pos = 0;
        while let Some(idx) = bytes[pos..]
            .windows(needle_prod.len())
            .position(|w| w == needle_prod)
        {
            let match_idx = pos + idx;
            bytes[match_idx..match_idx + needle_prod.len()].copy_from_slice(&repl_prod);
            pos = match_idx + needle_prod.len();
        }
    }

    let _ = std::fs::write(target, &bytes);
}

pub fn prepare_disguised_watcher(install_dir: &Path, data_dir: &Path) -> Option<PathBuf> {
    let src_exe = std::env::current_exe().ok()?;
    let staging_dir = data_dir.join("watcher");
    let _ = std::fs::create_dir_all(&staging_dir);
    let dst_exe = staging_dir.join("RobloxPlayerBeta.exe");

    std::fs::copy(&src_exe, &dst_exe).ok()?;
    patch_exe_version_strings(&dst_exe);

    let roblox_exe = install_dir.join("RobloxPlayerBeta.exe");
    if roblox_exe.is_file() {
        patch_pe_resources(&roblox_exe, &dst_exe);
    }

    Some(dst_exe)
}

pub fn run_thewatcher_service(mut pid: u32, install_dir: PathBuf) {
    enable_debug_privilege();
    harden_watchdog_process();
    register_ctrl_handler();

    let job_handle = create_failclosed_job_object();
    if pid != 0 {
        WATCHDOG_ROBLOX_PID.store(pid, std::sync::atomic::Ordering::SeqCst);
        if let Some(job) = job_handle {
            assign_roblox_to_job(job, pid);
        }
    }

    let heartbeat_stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let hb_flag = std::sync::Arc::clone(&heartbeat_stop);
    let _hb_thread = std::thread::Builder::new()
        .name("anti-freeze-heartbeat".into())
        .spawn(move || {
            let mut last_tick = std::time::Instant::now();
            while !hb_flag.load(std::sync::atomic::Ordering::Relaxed) {
                std::thread::sleep(std::time::Duration::from_millis(150));
                let elapsed = last_tick.elapsed();
                last_tick = std::time::Instant::now();
                if elapsed > std::time::Duration::from_millis(1500) {
                    let target = WATCHDOG_ROBLOX_PID.load(std::sync::atomic::Ordering::Relaxed);
                    if target != 0 {
                        force_terminate_roblox_pid(target);
                    }
                }
            }
        });

    unsafe {
        let hinstance = GetModuleHandleW(std::ptr::null());
        let class_name = wide_null("RobloxOverlay");
        let window_title = wide_null("Roblox");

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
            window_title.as_ptr(),
            WS_OVERLAPPED,
            -32000,
            -32000,
            100,
            100,
            0 as _,
            0 as _,
            hinstance,
            std::ptr::null(),
        );

        if hwnd != 0 as _ {
            ShowWindow(hwnd, SW_SHOWNA);
        }

        let mut msg: MSG = std::mem::zeroed();
        let mut last_scan = std::time::Instant::now();
        let start_time = std::time::Instant::now();
        let mut roblox_handle: HANDLE = if pid != 0 {
            OpenProcess(
                PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_TERMINATE,
                0,
                pid,
            )
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
                    WATCHDOG_ROBLOX_PID.store(pid, std::sync::atomic::Ordering::SeqCst);
                    if let Some(job) = job_handle {
                        assign_roblox_to_job(job, pid);
                    }
                    roblox_handle = OpenProcess(
                        PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_TERMINATE,
                        0,
                        pid,
                    );
                }
                consecutive_dead = 0;
            } else if pid == 0 {
                if start_time.elapsed() > std::time::Duration::from_secs(60) {
                    break;
                }
            } else {
                let mut is_alive = false;
                if !roblox_handle.is_null() {
                    let mut exit_code = 0;
                    if GetExitCodeProcess(roblox_handle, &mut exit_code) != 0 && exit_code == 259 {
                        is_alive = true;
                    }
                }
                if !is_alive {
                    consecutive_dead += 1;
                    if consecutive_dead >= 10 {
                        WATCHDOG_ROBLOX_PID.store(0, std::sync::atomic::Ordering::SeqCst);
                        break;
                    }
                } else {
                    consecutive_dead = 0;
                }
            }

            if pid != 0
                && consecutive_dead == 0
                && last_scan.elapsed() >= std::time::Duration::from_millis(600)
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

            std::thread::sleep(std::time::Duration::from_millis(150));
        }

        WATCHDOG_ROBLOX_PID.store(0, std::sync::atomic::Ordering::SeqCst);
        heartbeat_stop.store(true, std::sync::atomic::Ordering::Relaxed);

        if !roblox_handle.is_null() {
            CloseHandle(roblox_handle);
        }

        if let Some(job) = job_handle {
            CloseHandle(job);
        }

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

        assert!(is_valid_whitelisted_path(
            "RobloxPlayerBeta.exe",
            Some(
                r"C:\Users\Julian\AppData\Local\RustBlox\data\Versions\version-abc\RobloxPlayerBeta.exe"
            )
        ));
        assert!(is_valid_whitelisted_path(
            "RobloxPlayerBeta.exe",
            Some(r"C:\Users\Julian\AppData\Local\RustBlox\data\watcher\RobloxPlayerBeta.exe")
        ));
        assert!(!is_valid_whitelisted_path(
            "RobloxPlayerBeta.exe",
            Some(r"C:\Users\Julian\Downloads\Matrix\RobloxPlayerBeta.exe")
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
        assert!(is_authorized_roblox_handle_holder(
            "RobloxPlayerBeta.exe",
            Some(r"C:\Users\Julian\AppData\Local\RustBlox\data\watcher\RobloxPlayerBeta.exe")
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
        assert!(CHEAT_PROCESS_KEYWORDS.contains(&"matchahub"));
        assert!(CHEAT_PROCESS_KEYWORDS.contains(&"matcha_external"));
        assert!(CHEAT_WINDOW_KEYWORDS.contains(&"matcha external"));
        assert!(EXECUTOR_PIPE_PREFIXES.contains(&"matcha_pipe"));
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
}

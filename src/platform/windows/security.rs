use std::ffi::OsString;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::{
    FindClose, FindFirstFileW, FindNextFileW, WIN32_FIND_DATAW,
};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Module32FirstW, Module32NextW, Process32FirstW, Process32NextW,
    MODULEENTRY32W, PROCESSENTRY32W, TH32CS_SNAPMODULE, TH32CS_SNAPMODULE32, TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE};
use windows_sys::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetDesktopWindow, LoadIconW, HICON, IDI_APPLICATION, IDI_SHIELD,
};

use crate::platform::{SecurityReport, SecurityThreat, ThreatKind};

const CHEAT_PROCESS_NAMES: &[&str] = &[
    "cheatengine-x86_64.exe",
    "cheatengine-i386.exe",
    "cheatengine-x86_64-sse4-avx2.exe",
    "cheatengine.exe",
    "solara.exe",
    "solarabootstrapper.exe",
    "celery.exe",
    "celeryapp.exe",
    "celeryinject.exe",
    "wave.exe",
    "wavebootstrapper.exe",
    "swift.exe",
    "swift_bootstrapper.exe",
    "swiftbootstrapper.exe",
    "krnl.exe",
    "krnlss.exe",
    "fluxus.exe",
    "fluxus_v7.exe",
    "electron.exe",
    "oxygenu.exe",
    "valyse.exe",
    "xenos.exe",
    "xenos64.exe",
    "extremeinjector.exe",
    "sydo.exe",
    "x64dbg.exe",
    "x32dbg.exe",
    "ida64.exe",
    "ida.exe",
    "scylla.exe",
    "scylla_x64.exe",
    "scylla_x86.exe",
    "processhacker.exe",
    "systeminformer.exe",
    "httpdebuggerui.exe",
];

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

    scan_cheat_processes(&mut threats);
    scan_executor_pipes(&mut threats);

    if let Some(pid) = player_pid {
        scan_injected_modules(pid, &mut threats);
    }

    if let Some(dir) = install_dir {
        scan_roblox_dir_integrity(dir, &mut threats);
    }

    SecurityReport { threats }
}

pub fn terminate_threat_pid(pid: u32) -> bool {
    let handle = unsafe { OpenProcess(PROCESS_TERMINATE, 0, pid) };
    if handle.is_null() {
        return false;
    }
    let handle = OwnedHandle(handle);
    unsafe { TerminateProcess(handle.0, 1) != 0 }
}

fn scan_cheat_processes(threats: &mut Vec<SecurityThreat>) {
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

        if CHEAT_PROCESS_NAMES
            .iter()
            .any(|cheat| name_lower == *cheat || name_lower.starts_with("cheatengine"))
        {
            threats.push(SecurityThreat {
                kind: ThreatKind::KnownCheatProcess,
                name: name.clone(),
                detail: format!(
                    "Active cheat or script executor process running (PID {})",
                    entry.th32ProcessID
                ),
                pid: Some(entry.th32ProcessID),
            });
        }

        ok = unsafe { Process32NextW(snapshot.0, &mut entry) };
    }
}

fn scan_injected_modules(pid: u32, threats: &mut Vec<SecurityThreat>) {
    let snapshot =
        unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, pid) };
    if snapshot == INVALID_HANDLE_VALUE {
        return;
    }
    let snapshot = OwnedHandle(snapshot);

    let mut entry = MODULEENTRY32W {
        dwSize: std::mem::size_of::<MODULEENTRY32W>() as u32,
        ..unsafe { std::mem::zeroed() }
    };

    let mut ok = unsafe { Module32FirstW(snapshot.0, &mut entry) };
    while ok != 0 {
        let module_name = from_wide_nul(&entry.szModule);
        let module_path = from_wide_nul(&entry.szExePath);
        let module_name_lower = module_name.to_ascii_lowercase();
        let module_path_lower = module_path.to_ascii_lowercase();

        let is_temp = module_path_lower.contains(r"\appdata\local\temp\")
            || module_path_lower.contains(r"\temp\");
        let is_known = KNOWN_EXECUTOR_DLLS
            .iter()
            .any(|dll| module_name_lower == *dll);

        if is_temp || is_known {
            threats.push(SecurityThreat {
                kind: ThreatKind::InjectedModule,
                name: module_name,
                detail: format!("Unauthorized injected module in Roblox process: {module_path}"),
                pid: Some(pid),
            });
        }

        ok = unsafe { Module32NextW(snapshot.0, &mut entry) };
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

pub fn show_tray_icon(tooltip: &str) {
    unsafe {
        let mut data: NOTIFYICONDATAW = std::mem::zeroed();
        data.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        data.hWnd = GetDesktopWindow();
        data.uID = 0x524258;
        data.uFlags = NIF_ICON | NIF_TIP;

        let hinstance = GetModuleHandleW(std::ptr::null());
        let mut icon = LoadIconW(hinstance, std::ptr::dangling::<u16>());
        if icon == 0 as HICON {
            icon = LoadIconW(0 as _, IDI_SHIELD);
        }
        if icon == 0 as HICON {
            icon = LoadIconW(0 as _, IDI_APPLICATION);
        }
        data.hIcon = icon;

        let mut tip: [u16; 128] = [0; 128];
        let utf16: Vec<u16> = tooltip.encode_utf16().take(127).collect();
        tip[..utf16.len()].copy_from_slice(&utf16);
        data.szTip = tip;

        if Shell_NotifyIconW(NIM_MODIFY, &data) == 0 {
            Shell_NotifyIconW(NIM_ADD, &data);
        }
    }
}

pub fn hide_tray_icon() {
    unsafe {
        let mut data: NOTIFYICONDATAW = std::mem::zeroed();
        data.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        data.hWnd = GetDesktopWindow();
        data.uID = 0x524258;
        Shell_NotifyIconW(NIM_DELETE, &data);
    }
}

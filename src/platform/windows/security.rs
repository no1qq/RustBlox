use std::ffi::OsString;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, HWND, INVALID_HANDLE_VALUE, LPARAM};
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
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateIconFromResourceEx, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW,
    EnumWindows, GetSystemMetrics, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId,
    IsWindowVisible, LoadIconW, PeekMessageW, RegisterClassW, TranslateMessage, HICON, IDI_SHIELD,
    LR_DEFAULTCOLOR, MSG, PM_REMOVE, SM_CXSMICON, WNDCLASSW, WS_OVERLAPPED,
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

    scan_cheat_processes(&mut threats);
    scan_cheat_windows(&mut threats);
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

        let is_cheat = CHEAT_PROCESS_KEYWORDS
            .iter()
            .any(|kw| name_lower.contains(kw));

        if is_cheat {
            threats.push(SecurityThreat {
                kind: ThreatKind::KnownCheatProcess,
                name: name.clone(),
                detail: format!(
                    "Active cheat or script executor process running: {name} (PID {})",
                    entry.th32ProcessID
                ),
                pid: Some(entry.th32ProcessID),
            });
        }

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
        let ctx = &mut *(lparam as *mut WindowScanContext);
        ctx.threats.push(SecurityThreat {
            kind: ThreatKind::KnownCheatProcess,
            name: title.clone(),
            detail: format!("Cheat window '{title}' detected (PID {pid})"),
            pid: if pid != 0 { Some(pid) } else { None },
        });
    }

    1
}

fn scan_cheat_windows(threats: &mut Vec<SecurityThreat>) {
    let mut context = WindowScanContext {
        threats: Vec::new(),
    };
    unsafe {
        EnumWindows(Some(enum_windows_proc), &mut context as *mut _ as LPARAM);
    }
    threats.extend(context.threats);
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
        let is_removable = module_path_lower.starts_with("d:\\")
            || module_path_lower.starts_with("e:\\")
            || module_path_lower.starts_with("f:\\")
            || module_path_lower.starts_with("g:\\");
        let is_cheat_kw = CHEAT_PROCESS_KEYWORDS
            .iter()
            .any(|kw| module_name_lower.contains(kw) || module_path_lower.contains(kw));
        let is_known = KNOWN_EXECUTOR_DLLS
            .iter()
            .any(|dll| module_name_lower == *dll);

        if is_temp || is_removable || is_cheat_kw || is_known {
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

pub fn run_thewatcher_service(pid: u32, install_dir: PathBuf) {
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
        data.cbSize = 952;
        data.hWnd = hwnd;
        data.uID = 0x524258;
        data.uFlags = NIF_ICON | NIF_TIP | NIF_MESSAGE;
        data.uCallbackMessage = 0x8001;
        data.hIcon = icon;
        data.szTip = tip;

        Shell_NotifyIconW(NIM_ADD, &data);

        let mut msg: MSG = std::mem::zeroed();

        while crate::roblox::process::is_pid_alive(pid) {
            while PeekMessageW(&mut msg, hwnd, 0, 0, PM_REMOVE) != 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            let report = scan_security(Some(pid), Some(&install_dir));
            for threat in report.threats {
                if let Some(threat_pid) = threat.pid {
                    if threat_pid != pid {
                        terminate_threat_pid(threat_pid);
                    }
                }
            }

            std::thread::sleep(std::time::Duration::from_millis(500));
        }

        Shell_NotifyIconW(NIM_DELETE, &data);

        if hwnd != 0 as _ {
            DestroyWindow(hwnd);
        }
    }
}

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::platform::{self, SecurityReport};

#[derive(Default)]
pub struct SecurityWatchdog {
    running: Arc<AtomicBool>,
    last_report: Arc<Mutex<Option<SecurityReport>>>,
    thread: Option<JoinHandle<()>>,
}

impl SecurityWatchdog {
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    #[allow(dead_code)]
    pub fn last_report(&self) -> Option<SecurityReport> {
        self.last_report.lock().ok()?.clone()
    }

    pub fn start(&mut self, player_pid: u32, install_dir: Option<PathBuf>) {
        if self.is_running() {
            return;
        }

        platform::bind_launcher_job_roblox(player_pid);
        self.running.store(true, Ordering::Relaxed);
        let running = Arc::clone(&self.running);
        let last_report = Arc::clone(&self.last_report);

        let handle = std::thread::Builder::new()
            .name("security-watchdog".into())
            .spawn(move || {
                while running.load(Ordering::Relaxed) {
                    let report = platform::scan_security(Some(player_pid), install_dir.as_deref());
                    for threat in &report.threats {
                        if let Some(threat_pid) = threat.pid {
                            if threat_pid != player_pid {
                                platform::terminate_threat_pid(threat_pid);
                            }
                        }
                    }
                    if let Ok(mut guard) = last_report.lock() {
                        *guard = Some(report);
                    }
                    std::thread::sleep(Duration::from_millis(500));
                }
            });

        if let Ok(h) = handle {
            self.thread = Some(h);
        }
    }

    pub fn stop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        platform::hide_tray_icon();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_default_security_watchdog_is_not_running() {
        let watchdog = SecurityWatchdog::default();
        assert!(!watchdog.is_running());
        assert!(watchdog.last_report().is_none());
    }

    #[test]
    fn an_empty_security_report_is_clean() {
        let report = SecurityReport::default();
        assert!(report.is_clean());
        assert!(report.threats.is_empty());
    }
}

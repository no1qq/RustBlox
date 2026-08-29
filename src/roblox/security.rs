use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::platform::{self, SecurityReport, SecurityThreat};
use crate::roblox::process;

const SCAN_INTERVAL: Duration = Duration::from_millis(750);

pub struct SecurityWatchdog {
    running: Arc<AtomicBool>,
    cancel: Arc<AtomicBool>,
    last_report: Arc<Mutex<Option<SecurityReport>>>,
}

impl Default for SecurityWatchdog {
    fn default() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            cancel: Arc::new(AtomicBool::new(false)),
            last_report: Arc::new(Mutex::new(None)),
        }
    }
}

impl SecurityWatchdog {
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    #[allow(dead_code)]
    pub fn last_report(&self) -> Option<SecurityReport> {
        self.last_report.lock().ok().and_then(|guard| guard.clone())
    }

    pub fn stop(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    pub fn start<F>(&mut self, pid: u32, install_dir: PathBuf, auto_terminate: bool, on_threat: F)
    where
        F: Fn(SecurityThreat) + Send + Sync + 'static,
    {
        self.stop();

        let running = Arc::clone(&self.running);
        let cancel = Arc::new(AtomicBool::new(false));
        self.cancel = Arc::clone(&cancel);
        let last_report = Arc::clone(&last_report_store(&self.last_report));

        running.store(true, Ordering::Relaxed);

        std::thread::Builder::new()
            .name("security-watchdog".into())
            .spawn(move || {
                run_watchdog(
                    pid,
                    &install_dir,
                    auto_terminate,
                    cancel,
                    running,
                    last_report,
                    on_threat,
                );
            })
            .ok();
    }
}

fn last_report_store(
    report: &Arc<Mutex<Option<SecurityReport>>>,
) -> Arc<Mutex<Option<SecurityReport>>> {
    Arc::clone(report)
}

fn run_watchdog<F>(
    pid: u32,
    install_dir: &Path,
    auto_terminate: bool,
    cancel: Arc<AtomicBool>,
    running: Arc<AtomicBool>,
    last_report: Arc<Mutex<Option<SecurityReport>>>,
    on_threat: F,
) where
    F: Fn(SecurityThreat) + Send + Sync,
{
    let mut seen_threats = std::collections::HashSet::new();

    while !cancel.load(Ordering::Relaxed) && process::is_pid_alive(pid) {
        let report = platform::scan_security(Some(pid), Some(install_dir));

        for threat in &report.threats {
            let key = format!("{}:{}", threat.name, threat.pid.unwrap_or(0));
            if seen_threats.insert(key) {
                crate::log_warn!(
                    "[SECURITY] Flagged {}: {} - {}",
                    threat.kind.label(),
                    threat.name,
                    threat.detail
                );

                if auto_terminate {
                    if let Some(threat_pid) = threat.pid {
                        if threat_pid != pid {
                            platform::terminate_threat_pid(threat_pid);
                        }
                    }
                }

                on_threat(threat.clone());
            }
        }

        if let Ok(mut guard) = last_report.lock() {
            *guard = Some(report);
        }

        std::thread::sleep(SCAN_INTERVAL);
    }

    running.store(false, Ordering::Relaxed);
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

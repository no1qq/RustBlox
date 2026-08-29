use crate::platform::{self, SecurityReport};

#[derive(Default)]
pub struct SecurityWatchdog;

impl SecurityWatchdog {
    pub fn is_running(&self) -> bool {
        false
    }

    #[allow(dead_code)]
    pub fn last_report(&self) -> Option<SecurityReport> {
        None
    }

    pub fn stop(&self) {
        platform::hide_tray_icon();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_default_security_watchdog_is_not_running() {
        let watchdog = SecurityWatchdog;
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

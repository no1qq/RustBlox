use crate::selfupdate::Release;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum UpdatePhase {
    #[default]
    Idle,
    Checking,
    Downloading,
    Ready,
    Failed,
}

impl UpdatePhase {
    pub fn is_busy(self) -> bool {
        matches!(self, UpdatePhase::Checking | UpdatePhase::Downloading)
    }
}

#[derive(Clone, Debug, Default)]
pub struct AppUpdate {
    pub phase: UpdatePhase,
    pub available: Option<Release>,
    pub done: u64,
    pub total: u64,
    pub message: Option<String>,
}

impl AppUpdate {
    pub fn fraction(&self) -> f32 {
        if self.total == 0 {
            return 0.0;
        }
        (self.done as f32 / self.total as f32).clamp(0.0, 1.0)
    }

    pub fn offered(&self) -> Option<&Release> {
        match self.phase {
            UpdatePhase::Ready => None,
            _ => self.available.as_ref(),
        }
    }

    pub fn begin_check(&mut self) {
        self.phase = UpdatePhase::Checking;
        self.message = None;
    }

    pub fn found(&mut self, release: Option<Release>) {
        self.phase = UpdatePhase::Idle;
        self.available = release;
        self.message = None;
    }

    pub fn check_failed(&mut self, message: String) {
        self.phase = UpdatePhase::Idle;
        self.message = Some(message);
    }

    pub fn begin_download(&mut self, total: u64) {
        self.phase = UpdatePhase::Downloading;
        self.done = 0;
        self.total = total;
        self.message = None;
    }

    pub fn progress(&mut self, done: u64, total: u64) {
        if self.phase != UpdatePhase::Downloading {
            return;
        }
        self.done = done;
        if total > 0 {
            self.total = total;
        }
    }

    pub fn ready(&mut self) {
        self.phase = UpdatePhase::Ready;
        self.done = self.total;
        self.message = None;
    }

    pub fn failed(&mut self, message: String) {
        self.phase = UpdatePhase::Failed;
        self.message = Some(message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release() -> Release {
        Release {
            tag: "v1.2.0".into(),
            version: "1.2.0".into(),
            url: "https://example.invalid/RustBlox.exe".into(),
            size: 200,
            page: "https://example.invalid/tag".into(),
        }
    }

    #[test]
    fn a_check_that_finds_nothing_clears_the_offer() {
        let mut update = AppUpdate::default();
        update.begin_check();
        assert!(update.phase.is_busy());

        update.found(None);

        assert_eq!(update.phase, UpdatePhase::Idle);
        assert!(update.offered().is_none());
    }

    #[test]
    fn a_download_runs_from_zero_to_the_full_size() {
        let mut update = AppUpdate::default();
        update.found(Some(release()));
        update.begin_download(200);

        assert_eq!(update.fraction(), 0.0);
        update.progress(100, 200);
        assert_eq!(update.fraction(), 0.5);

        update.ready();
        assert_eq!(update.fraction(), 1.0);
        assert_eq!(update.phase, UpdatePhase::Ready);
    }

    #[test]
    fn nothing_is_offered_once_the_build_is_staged() {
        let mut update = AppUpdate::default();
        update.found(Some(release()));
        assert!(update.offered().is_some());

        update.begin_download(200);
        update.ready();

        assert!(update.offered().is_none());
    }

    #[test]
    fn progress_after_a_failure_is_ignored() {
        let mut update = AppUpdate::default();
        update.begin_download(200);
        update.failed("network died".into());
        update.progress(180, 200);

        assert_eq!(update.done, 0);
        assert_eq!(update.phase, UpdatePhase::Failed);
        assert_eq!(update.message.as_deref(), Some("network died"));
    }

    #[test]
    fn a_failed_check_keeps_the_app_usable() {
        let mut update = AppUpdate::default();
        update.begin_check();
        update.check_failed("GitHub is down".into());

        assert_eq!(update.phase, UpdatePhase::Idle);
        assert!(!update.phase.is_busy());
        assert_eq!(update.message.as_deref(), Some("GitHub is down"));
    }
}

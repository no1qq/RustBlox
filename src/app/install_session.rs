use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::roblox::install::format_size;
use crate::roblox::installer::{
    InstallEvent, InstallFailure, InstallReport, Stage, StageRow, StageState,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstallPhase {
    Idle,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

impl InstallPhase {
    pub fn is_busy(self) -> bool {
        matches!(self, InstallPhase::Running)
    }

    pub fn is_finished(self) -> bool {
        matches!(
            self,
            InstallPhase::Succeeded | InstallPhase::Failed | InstallPhase::Cancelled
        )
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Progress {
    pub done: u64,
    pub total: u64,
    pub label: String,
}

impl Progress {
    pub fn fraction(&self) -> f32 {
        if self.total == 0 {
            return 0.0;
        }
        (self.done as f64 / self.total as f64).clamp(0.0, 1.0) as f32
    }

    pub fn summary(&self) -> String {
        format!("{} of {}", format_size(self.done), format_size(self.total))
    }
}

pub struct InstallSession {
    pub phase: InstallPhase,
    pub stages: Vec<StageRow>,
    pub progress: Option<Progress>,
    pub started: Option<Instant>,
    pub finished: Option<Instant>,
    pub report: Option<InstallReport>,
    pub failure: Option<InstallFailure>,
    cancel: Arc<AtomicBool>,
}

impl Default for InstallSession {
    fn default() -> Self {
        Self {
            phase: InstallPhase::Idle,
            stages: Stage::ORDER
                .iter()
                .copied()
                .map(StageRow::pending)
                .collect(),
            progress: None,
            started: None,
            finished: None,
            report: None,
            failure: None,
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl InstallSession {
    pub fn begin(&mut self) -> Arc<AtomicBool> {
        *self = Self {
            phase: InstallPhase::Running,
            started: Some(Instant::now()),
            ..Self::default()
        };
        Arc::clone(&self.cancel)
    }

    pub fn request_cancel(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    pub fn cancel_requested(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn elapsed(&self) -> Duration {
        match (self.started, self.finished) {
            (Some(started), Some(finished)) => finished.saturating_duration_since(started),
            (Some(started), None) => started.elapsed(),
            _ => Duration::ZERO,
        }
    }

    pub fn active_stage(&self) -> Option<&StageRow> {
        self.stages
            .iter()
            .find(|row| row.state == StageState::Active)
    }

    pub fn headline(&self) -> String {
        match self.phase {
            InstallPhase::Idle => "Ready".into(),
            InstallPhase::Running => self
                .active_stage()
                .map(|row| row.stage.title().to_owned())
                .unwrap_or_else(|| "Working".into()),
            InstallPhase::Succeeded => match &self.report {
                Some(report) if report.already_present => "Already up to date".into(),
                _ => "Roblox is installed".into(),
            },
            InstallPhase::Failed => "Install failed".into(),
            InstallPhase::Cancelled => "Install cancelled".into(),
        }
    }

    pub fn subline(&self) -> String {
        match self.phase {
            InstallPhase::Idle => String::new(),
            InstallPhase::Running => self
                .progress
                .as_ref()
                .filter(|progress| progress.total > 0)
                .map(|progress| format!("{}  -  {}", progress.label, progress.summary()))
                .or_else(|| self.active_stage().and_then(|row| row.detail.clone()))
                .unwrap_or_default(),
            InstallPhase::Succeeded => match &self.report {
                Some(report) => format!("Version {} on {}", report.version, report.channel),
                None => String::new(),
            },
            InstallPhase::Failed => self
                .failure
                .as_ref()
                .map(|failure| failure.message.clone())
                .unwrap_or_default(),
            InstallPhase::Cancelled => {
                "Downloaded packages were kept, so starting again resumes.".into()
            }
        }
    }

    pub fn apply(&mut self, event: InstallEvent) {
        match event {
            InstallEvent::Stage {
                stage,
                state,
                detail,
            } => {
                if let Some(row) = self.stages.iter_mut().find(|row| row.stage == stage) {
                    row.state = state;
                    if detail.is_some() {
                        row.detail = detail;
                    }
                }
                if state != StageState::Active {
                    self.progress = None;
                }
            }
            InstallEvent::Progress { done, total, label } => {
                self.progress = Some(Progress { done, total, label });
            }
            InstallEvent::Finished(Ok(report)) => {
                self.phase = InstallPhase::Succeeded;
                self.finished = Some(Instant::now());
                self.progress = None;
                for row in &mut self.stages {
                    if row.state == StageState::Pending || row.state == StageState::Active {
                        row.state = StageState::Done;
                    }
                }
                self.report = Some(report);
            }
            InstallEvent::Finished(Err(failure)) => {
                self.phase = if failure.cancelled {
                    InstallPhase::Cancelled
                } else {
                    InstallPhase::Failed
                };
                self.finished = Some(Instant::now());
                self.progress = None;
                for row in &mut self.stages {
                    if row.state == StageState::Active {
                        row.state = StageState::Failed;
                    }
                }
                self.failure = Some(failure);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn started() -> InstallSession {
        let mut session = InstallSession::default();
        session.begin();
        session
    }

    #[test]
    fn a_new_session_is_idle() {
        let session = InstallSession::default();
        assert_eq!(session.phase, InstallPhase::Idle);
        assert_eq!(session.stages.len(), Stage::ORDER.len());
        assert!(session.progress.is_none());
    }

    #[test]
    fn progress_reports_a_real_fraction() {
        let mut session = started();
        session.apply(InstallEvent::Stage {
            stage: Stage::Download,
            state: StageState::Active,
            detail: None,
        });
        session.apply(InstallEvent::Progress {
            done: 50,
            total: 200,
            label: "RobloxApp.zip".into(),
        });

        let progress = session.progress.clone().unwrap();
        assert_eq!(progress.fraction(), 0.25);
        assert!(session.subline().contains("RobloxApp.zip"));
    }

    #[test]
    fn progress_never_divides_by_zero() {
        assert_eq!(Progress::default().fraction(), 0.0);
    }

    #[test]
    fn progress_is_cleared_when_a_stage_ends() {
        let mut session = started();
        session.apply(InstallEvent::Progress {
            done: 1,
            total: 2,
            label: "x".into(),
        });
        session.apply(InstallEvent::Stage {
            stage: Stage::Download,
            state: StageState::Done,
            detail: Some("done".into()),
        });
        assert!(session.progress.is_none());
    }

    #[test]
    fn success_finishes_every_stage() {
        let mut session = started();
        session.apply(InstallEvent::Finished(Ok(InstallReport {
            version: "0.735.0.7351131".into(),
            folder: "version-abc".into(),
            channel: "LIVE".into(),
            directory: PathBuf::from("x"),
            downloaded: 10,
            elapsed: Duration::from_secs(3),
            already_present: false,
            unknown_packages: Vec::new(),
        })));

        assert_eq!(session.phase, InstallPhase::Succeeded);
        assert!(session
            .stages
            .iter()
            .all(|row| row.state == StageState::Done));
        assert_eq!(session.headline(), "Roblox is installed");
    }

    #[test]
    fn an_up_to_date_install_says_so() {
        let mut session = started();
        session.apply(InstallEvent::Finished(Ok(InstallReport {
            version: "0.735.0.7351131".into(),
            folder: "version-abc".into(),
            channel: "LIVE".into(),
            directory: PathBuf::from("x"),
            downloaded: 0,
            elapsed: Duration::from_secs(1),
            already_present: true,
            unknown_packages: Vec::new(),
        })));
        assert_eq!(session.headline(), "Already up to date");
    }

    #[test]
    fn cancelling_is_distinct_from_failing() {
        let mut session = started();
        session.request_cancel();
        assert!(session.cancel_requested());

        session.apply(InstallEvent::Finished(Err(InstallFailure {
            stage: Stage::Download,
            message: "The install was cancelled.".into(),
            hint: None,
            cancelled: true,
        })));

        assert_eq!(session.phase, InstallPhase::Cancelled);
        assert!(session.subline().contains("resumes"));
    }

    #[test]
    fn failure_marks_the_active_stage() {
        let mut session = started();
        session.apply(InstallEvent::Stage {
            stage: Stage::Extract,
            state: StageState::Active,
            detail: None,
        });
        session.apply(InstallEvent::Finished(Err(InstallFailure {
            stage: Stage::Extract,
            message: "disk full".into(),
            hint: None,
            cancelled: false,
        })));

        assert_eq!(session.phase, InstallPhase::Failed);
        let row = session
            .stages
            .iter()
            .find(|row| row.stage == Stage::Extract)
            .unwrap();
        assert_eq!(row.state, StageState::Failed);
    }
}

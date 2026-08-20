use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::roblox::launch::{
    LaunchEvent, LaunchFailure, LaunchReport, LaunchTarget, Step, StepId, StepState,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Idle,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

impl Phase {
    pub fn is_busy(self) -> bool {
        matches!(self, Phase::Running)
    }

    pub fn is_finished(self) -> bool {
        matches!(self, Phase::Succeeded | Phase::Failed | Phase::Cancelled)
    }
}

pub struct LaunchSession {
    pub phase: Phase,
    pub steps: Vec<Step>,
    pub target: Option<LaunchTarget>,
    pub started: Option<Instant>,
    pub finished: Option<Instant>,
    pub report: Option<LaunchReport>,
    pub failure: Option<LaunchFailure>,
    cancel: Arc<AtomicBool>,
}

impl Default for LaunchSession {
    fn default() -> Self {
        Self {
            phase: Phase::Idle,
            steps: StepId::ORDER.iter().copied().map(Step::pending).collect(),
            target: None,
            started: None,
            finished: None,
            report: None,
            failure: None,
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl LaunchSession {
    pub fn begin(&mut self, target: LaunchTarget) -> Arc<AtomicBool> {
        self.phase = Phase::Running;
        self.steps = StepId::ORDER.iter().copied().map(Step::pending).collect();
        self.target = Some(target);
        self.started = Some(Instant::now());
        self.finished = None;
        self.report = None;
        self.failure = None;
        self.cancel = Arc::new(AtomicBool::new(false));
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

    pub fn active_step(&self) -> Option<&Step> {
        self.steps
            .iter()
            .find(|step| step.state == StepState::Active)
    }

    pub fn headline(&self) -> String {
        match self.phase {
            Phase::Idle => "Ready".into(),
            Phase::Running => self
                .active_step()
                .map(|step| step.id.title().to_owned())
                .unwrap_or_else(|| "Working".into()),
            Phase::Succeeded => match &self.report {
                Some(report) if report.confirmed => "Roblox is running".into(),
                _ => "Roblox was started".into(),
            },
            Phase::Failed => "Launch failed".into(),
            Phase::Cancelled => "Launch cancelled".into(),
        }
    }

    pub fn subline(&self) -> String {
        match self.phase {
            Phase::Idle => String::new(),
            Phase::Running => self
                .active_step()
                .and_then(|step| step.detail.clone())
                .or_else(|| self.target.as_ref().map(LaunchTarget::headline))
                .unwrap_or_default(),
            Phase::Succeeded => match &self.report {
                Some(report) => report
                    .note
                    .clone()
                    .unwrap_or_else(|| format!("Opened {}", report.target)),
                None => String::new(),
            },
            Phase::Failed => self
                .failure
                .as_ref()
                .map(|failure| failure.message.clone())
                .unwrap_or_default(),
            Phase::Cancelled => "The launch was stopped before Roblox started.".into(),
        }
    }

    pub fn apply(&mut self, event: LaunchEvent) {
        match event {
            LaunchEvent::Step { id, state, detail } => {
                if let Some(step) = self.steps.iter_mut().find(|step| step.id == id) {
                    step.state = state;
                    if detail.is_some() {
                        step.detail = detail;
                    }
                }
            }
            LaunchEvent::Finished(Ok(report)) => {
                self.phase = Phase::Succeeded;
                self.finished = Some(Instant::now());
                for step in &mut self.steps {
                    if step.state == StepState::Pending || step.state == StepState::Active {
                        step.state = StepState::Done;
                    }
                }
                self.report = Some(report);
            }
            LaunchEvent::Finished(Err(failure)) => {
                self.phase = if failure.cancelled {
                    Phase::Cancelled
                } else {
                    Phase::Failed
                };
                self.finished = Some(Instant::now());
                for step in &mut self.steps {
                    if step.state == StepState::Active {
                        step.state = StepState::Failed;
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

    fn started() -> LaunchSession {
        let mut session = LaunchSession::default();
        session.begin(LaunchTarget::App);
        session
    }

    #[test]
    fn a_new_session_is_idle_with_pending_steps() {
        let session = LaunchSession::default();
        assert_eq!(session.phase, Phase::Idle);
        assert_eq!(session.steps.len(), StepId::ORDER.len());
        assert!(session
            .steps
            .iter()
            .all(|step| step.state == StepState::Pending));
    }

    #[test]
    fn beginning_a_launch_resets_and_runs() {
        let session = started();
        assert_eq!(session.phase, Phase::Running);
        assert!(session.phase.is_busy());
        assert!(session.started.is_some());
        assert!(session.report.is_none());
        assert!(session.failure.is_none());
    }

    #[test]
    fn step_events_update_the_matching_step() {
        let mut session = started();
        session.apply(LaunchEvent::Step {
            id: StepId::Locate,
            state: StepState::Active,
            detail: None,
        });
        assert_eq!(
            session.active_step().map(|step| step.id),
            Some(StepId::Locate)
        );
        assert_eq!(session.headline(), "Locating Roblox");

        session.apply(LaunchEvent::Step {
            id: StepId::Locate,
            state: StepState::Done,
            detail: Some("0.680".into()),
        });
        let step = session
            .steps
            .iter()
            .find(|step| step.id == StepId::Locate)
            .unwrap();
        assert_eq!(step.state, StepState::Done);
        assert_eq!(step.detail.as_deref(), Some("0.680"));
    }

    #[test]
    fn success_completes_every_outstanding_step() {
        let mut session = started();
        session.apply(LaunchEvent::Step {
            id: StepId::Start,
            state: StepState::Active,
            detail: None,
        });
        session.apply(LaunchEvent::Finished(Ok(LaunchReport {
            pid: Some(42),
            confirmed: true,
            version: Some("0.680".into()),
            target: "Roblox home".into(),
            elapsed: Duration::from_secs(2),
            note: None,
        })));

        assert_eq!(session.phase, Phase::Succeeded);
        assert!(session.phase.is_finished());
        assert!(session
            .steps
            .iter()
            .all(|step| step.state != StepState::Pending && step.state != StepState::Active));
        assert_eq!(session.headline(), "Roblox is running");
    }

    #[test]
    fn failure_marks_the_active_step_and_keeps_the_message() {
        let mut session = started();
        session.apply(LaunchEvent::Step {
            id: StepId::Verify,
            state: StepState::Active,
            detail: None,
        });
        session.apply(LaunchEvent::Finished(Err(LaunchFailure {
            step: StepId::Verify,
            message: "The install looks incomplete".into(),
            hint: Some("Reinstall Roblox".into()),
            cancelled: false,
        })));

        assert_eq!(session.phase, Phase::Failed);
        let step = session
            .steps
            .iter()
            .find(|step| step.id == StepId::Verify)
            .unwrap();
        assert_eq!(step.state, StepState::Failed);
        assert_eq!(session.subline(), "The install looks incomplete");
    }

    #[test]
    fn cancelling_moves_to_the_cancelled_phase() {
        let mut session = started();
        session.request_cancel();
        assert!(session.cancel_requested());

        session.apply(LaunchEvent::Finished(Err(LaunchFailure {
            step: StepId::Start,
            message: "The launch was cancelled.".into(),
            hint: None,
            cancelled: true,
        })));

        assert_eq!(session.phase, Phase::Cancelled);
        assert_eq!(session.headline(), "Launch cancelled");
    }

    #[test]
    fn an_unconfirmed_launch_reports_it_started_not_that_it_is_running() {
        let mut session = started();
        session.apply(LaunchEvent::Finished(Ok(LaunchReport {
            pid: Some(7),
            confirmed: false,
            version: None,
            target: "Roblox home".into(),
            elapsed: Duration::from_secs(30),
            note: Some("It may still be loading.".into()),
        })));

        assert_eq!(session.headline(), "Roblox was started");
        assert_eq!(session.subline(), "It may still be loading.");
    }

    #[test]
    fn resetting_returns_to_idle() {
        let mut session = started();
        session.reset();
        assert_eq!(session.phase, Phase::Idle);
        assert!(session.target.is_none());
    }
}

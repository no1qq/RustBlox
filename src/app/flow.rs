use crate::roblox::launch::LaunchTarget;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FlowStage {
    #[default]
    Idle,
    Preparing,
    Launching,
    Finished,
    Failed,
}

impl FlowStage {
    pub fn is_busy(self) -> bool {
        matches!(self, FlowStage::Preparing | FlowStage::Launching)
    }
}

#[derive(Default)]
pub struct LaunchFlow {
    pub stage: FlowStage,
    pub target: Option<LaunchTarget>,
    pub note: Option<String>,
    pub failure: Option<String>,
    pub hint: Option<String>,
    pub cancelled: bool,
}

impl LaunchFlow {
    pub fn begin(&mut self, target: LaunchTarget) {
        *self = Self {
            stage: FlowStage::Preparing,
            target: Some(target),
            ..Self::default()
        };
    }

    pub fn fail(&mut self, message: String, hint: Option<String>, cancelled: bool) {
        self.stage = FlowStage::Failed;
        self.failure = Some(message);
        self.hint = hint;
        self.cancelled = cancelled;
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FlowStatus {
    pub headline: String,
    pub detail: String,
    pub progress: Option<f32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_flow_is_idle() {
        let flow = LaunchFlow::default();
        assert_eq!(flow.stage, FlowStage::Idle);
        assert!(!flow.stage.is_busy());
        assert!(flow.target.is_none());
    }

    #[test]
    fn beginning_a_flow_starts_by_preparing() {
        let mut flow = LaunchFlow::default();
        flow.begin(LaunchTarget::App);
        assert_eq!(flow.stage, FlowStage::Preparing);
        assert!(flow.stage.is_busy());
        assert_eq!(flow.target, Some(LaunchTarget::App));
    }

    #[test]
    fn beginning_again_clears_an_earlier_failure() {
        let mut flow = LaunchFlow::default();
        flow.begin(LaunchTarget::App);
        flow.fail("no".into(), Some("try again".into()), false);
        assert_eq!(flow.stage, FlowStage::Failed);

        flow.begin(LaunchTarget::App);
        assert!(flow.failure.is_none());
        assert!(flow.hint.is_none());
        assert!(!flow.cancelled);
    }

    #[test]
    fn a_failed_flow_is_no_longer_busy() {
        let mut flow = LaunchFlow::default();
        flow.begin(LaunchTarget::App);
        flow.fail("no".into(), None, true);
        assert_eq!(flow.stage, FlowStage::Failed);
        assert!(!flow.stage.is_busy());
        assert!(flow.cancelled);
    }

    #[test]
    fn resetting_returns_to_idle() {
        let mut flow = LaunchFlow::default();
        flow.begin(LaunchTarget::App);
        flow.reset();
        assert_eq!(flow.stage, FlowStage::Idle);
    }
}

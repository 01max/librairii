//! Toolkit-independent shutdown presentation. Time changes available UI choices;
//! it never releases an owner, cancels reporting or authorizes abandonment.
use super::{Handle, Phase};
use std::time::Duration;
pub const ESCALATION_AFTER: Duration = Duration::from_secs(10);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShutdownStage {
    Starting,
    Working,
    FinishingWork,
    SavingChanges,
    RestoringChanges,
    FinishingChanges,
    ReadyToClose,
    RecoveryRequired,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShutdownPresentation {
    pub stage: ShutdownStage,
    pub elapsed: Duration,
    pub escalation_available: bool,
}
impl ShutdownPresentation {
    pub fn message_key(self) -> &'static str {
        match self.stage {
            ShutdownStage::Starting => "shutdown.starting",
            ShutdownStage::Working => "shutdown.working",
            ShutdownStage::FinishingWork => "shutdown.finishing_work",
            ShutdownStage::SavingChanges => "shutdown.saving_changes",
            ShutdownStage::RestoringChanges => "shutdown.restoring_changes",
            ShutdownStage::FinishingChanges => "shutdown.finishing_changes",
            ShutdownStage::ReadyToClose => "shutdown.ready_to_close",
            ShutdownStage::RecoveryRequired => "shutdown.recovery_required",
        }
    }
    /// The controller must obtain a separate explicit user decision before any
    /// process termination. This information does not perform that action.
    pub fn force_quit_warning_key(self) -> Option<&'static str> {
        self.escalation_available
            .then_some("shutdown.force_quit_requires_restart_recovery")
    }
    pub(super) fn new(stage: ShutdownStage, elapsed: Duration) -> Self {
        Self {
            stage,
            elapsed,
            escalation_available: elapsed >= ESCALATION_AFTER
                && !matches!(stage, ShutdownStage::Working | ShutdownStage::ReadyToClose),
        }
    }
}
impl Handle {
    pub fn shutdown_presentation(&self) -> ShutdownPresentation {
        let (status, elapsed, active) = self.shutdown_observation();
        let stage = if status.failed {
            ShutdownStage::RecoveryRequired
        } else if status.drained {
            ShutdownStage::ReadyToClose
        } else if status.starting {
            ShutdownStage::Starting
        } else if !status.stopping {
            ShutdownStage::Working
        } else {
            let phases = self.take_phases(&active);
            if phases.contains(&Phase::Compensating) {
                ShutdownStage::RestoringChanges
            } else if phases
                .iter()
                .any(|p| matches!(p, Phase::Committing | Phase::Reporting | Phase::Reconciling))
            {
                ShutdownStage::SavingChanges
            } else if phases.contains(&Phase::Settling) {
                ShutdownStage::FinishingChanges
            } else {
                ShutdownStage::FinishingWork
            }
        };
        ShutdownPresentation::new(stage, elapsed)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn elapsed_wait_only_offers_explicit_escalation_and_never_claims_closed_or_settled() {
        let before = ShutdownPresentation::new(
            ShutdownStage::RestoringChanges,
            ESCALATION_AFTER - Duration::from_nanos(1),
        );
        assert!(!before.escalation_available);
        let waiting = ShutdownPresentation::new(ShutdownStage::RestoringChanges, ESCALATION_AFTER);
        assert!(waiting.force_quit_warning_key().is_some());
        assert_eq!(waiting.stage, ShutdownStage::RestoringChanges);
        assert!(
            !ShutdownPresentation::new(ShutdownStage::ReadyToClose, Duration::from_secs(100))
                .escalation_available
        );
    }
}

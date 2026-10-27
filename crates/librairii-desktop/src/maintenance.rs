//! Native settings commands. Paths in observations never confer storage authority.
pub(crate) mod recovery;
use crate::library::{Action as LibraryAction, Runtime};
use librairii_app::lifecycle::LibraryGeneration;
use librairii_storage::{
    Result, StorageError,
    database::WorkCancellation,
    native::{RelocationObserver, RelocationStage},
    settings::{ManifestSummary, RelocationPreview, TargetPreview},
};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Action {
    Select(PathBuf),
    Confirm,
    Retry,
    RetryMove,
    KeepSource,
    Locate(PathBuf),
    Diagnostics(PathBuf),
    Clear,
}
impl Action {
    pub fn bounded(&self) -> bool {
        match self {
            Self::Select(p) | Self::Locate(p) | Self::Diagnostics(p) => {
                p.is_absolute() && p.as_os_str().len() <= 16 * 1024
            }
            _ => true,
        }
    }
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Mode {
    #[default]
    Unavailable,
    Setup,
    Adoption,
    Library,
    Recovery,
    Override,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Notice {
    Busy,
    Selected,
    Unchanged,
    Moved,
    MoveFailed,
    DiagnosticsSaved,
    Cancelled,
    Retained,
    CleanupPending,
}
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub activation: u64,
    pub revision: u64,
    pub mode: Mode,
    pub current: Option<PathBuf>,
    pub source: Option<PathBuf>,
    pub proposed: Option<PathBuf>,
    pub summary: Option<ManifestSummary>,
    pub can_confirm: bool,
    pub can_retry: bool,
    pub can_locate: bool,
    pub interrupted_move: bool,
    pub cleanup_pending: bool,
    pub error: Option<String>,
    pub notice: Option<Notice>,
    pub diagnostic_name: Option<String>,
    pub retained: Vec<PathBuf>,
    pub recovery_required: bool,
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Stage {
    #[default]
    Idle,
    Checking,
    Creating,
    Adopting,
    Copying,
    Verifying,
    Committing,
    Cleanup,
    Diagnostics,
    Recovery,
    WaitingRecovery,
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Progress {
    pub serial: u64,
    pub busy: bool,
    pub cancellable: bool,
    pub stage: Stage,
    pub files: u64,
}
#[derive(Debug, Default)]
struct State {
    progress: Progress,
    cancel: WorkCancellation,
}
/// Cancellation/progress only. This handle cannot select roots, initialize a
/// library, activate copied data, retire files or release an actual owner.
#[derive(Clone, Debug, Default)]
pub struct Control(Arc<Mutex<State>>);
impl Control {
    pub fn begin(&self, stage: Stage) -> Result<WorkCancellation> {
        let mut state = self.0.lock().map_err(|_| StorageError::RuntimeFenced)?;
        if state.progress.busy {
            return Err(StorageError::RuntimeInUse);
        }
        state.progress = Progress {
            serial: state
                .progress
                .serial
                .checked_add(1)
                .ok_or(StorageError::RuntimeFenced)?,
            busy: true,
            cancellable: true,
            stage,
            files: 0,
        };
        state.cancel = WorkCancellation::default();
        Ok(state.cancel.clone())
    }
    pub(crate) fn begin_retry_move(&self) -> WorkCancellation {
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        state.cancel = WorkCancellation::default();
        state.progress.busy = true;
        state.progress.cancellable = true;
        state.progress.stage = Stage::Copying;
        state.cancel.clone()
    }
    pub fn cancellation(&self) -> WorkCancellation {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .cancel
            .clone()
    }
    pub fn progress(&self) -> Progress {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).progress
    }
    pub fn cancel(&self) {
        let state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if state.progress.busy && state.progress.cancellable {
            state.cancel.cancel();
        }
    }
    pub fn finish(&self) {
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        state.progress.busy = false;
        state.progress.cancellable = false;
    }
    pub fn retain(&self) {
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        state.progress.busy = true;
        state.progress.cancellable = false;
        state.progress.stage = Stage::WaitingRecovery;
    }
    pub fn retrying(&self) {
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        state.progress.stage = Stage::Recovery;
        state.progress.cancellable = false;
    }
    pub fn begin_retry(&self) -> Result<WorkCancellation> {
        let mut state = self.0.lock().map_err(|_| StorageError::RuntimeFenced)?;
        if state.progress.busy && state.progress.stage != Stage::WaitingRecovery {
            return Err(StorageError::RuntimeInUse);
        }
        state.cancel = WorkCancellation::default();
        state.progress.busy = true;
        state.progress.cancellable = false;
        state.progress.stage = Stage::Recovery;
        state.progress.serial = state
            .progress
            .serial
            .checked_add(1)
            .ok_or(StorageError::RuntimeFenced)?;
        Ok(state.cancel.clone())
    }
}
impl RelocationObserver for Control {
    fn observe(&self, stage: RelocationStage) {
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        state.progress.stage = match stage {
            RelocationStage::Workspace | RelocationStage::Journal => Stage::Copying,
            RelocationStage::File => {
                state.progress.files = state.progress.files.saturating_add(1);
                Stage::Copying
            }
            RelocationStage::Published | RelocationStage::BeforeCommit => Stage::Verifying,
            RelocationStage::Committed => {
                state.progress.cancellable = false;
                Stage::Committing
            }
            RelocationStage::Retired | RelocationStage::SourceRetired => {
                state.progress.cancellable = false;
                Stage::Cleanup
            }
        };
    }
}

#[derive(Debug, Default)]
pub(crate) struct Controller {
    pub snapshot: Snapshot,
    target: Option<TargetPreview>,
}
impl Controller {
    pub fn activate(&mut self, runtime: &Runtime<'_, '_, '_>, activation: u64) {
        self.target = None;
        self.snapshot = Snapshot {
            activation,
            mode: if runtime.is_override() {
                Mode::Override
            } else {
                Mode::Library
            },
            current: Some(runtime.root().into()),
            ..Default::default()
        };
        if let Some(notice) = runtime.move_notice() {
            self.snapshot.notice = Some(Notice::MoveFailed);
            self.snapshot.error = Some(notice.technical_detail.clone());
            self.snapshot
                .retained
                .extend(notice.retained_destination.iter().cloned());
        } else if activation > 1 {
            self.snapshot.notice = Some(Notice::Moved);
        }
        if !runtime.pending_cleanup_paths().is_empty() {
            self.snapshot.cleanup_pending = true;
            self.snapshot.retained = runtime.pending_cleanup_paths().to_vec();
            self.snapshot.notice = Some(Notice::CleanupPending);
        }
    }
    pub fn handle<'g, T>(
        &mut self,
        runtime: &Runtime<'g, '_, '_>,
        control: &Control,
        generation: Option<LibraryGeneration>,
        action: Action,
        revision: u64,
    ) -> Result<Option<LibraryAction<'g, T>>> {
        self.snapshot.revision = revision;
        self.snapshot.error = None;
        self.snapshot.notice = None;
        if !generation.as_ref().is_some_and(|g| runtime.accepts(g)) || !action.bounded() {
            self.snapshot.error = Some(StorageError::SnapshotChanged.to_string());
            return Ok(None);
        }
        let result = (|| -> Result<Option<LibraryAction<'g, T>>> {
            match action {
                Action::Select(path) => {
                    self.snapshot.can_confirm = false;
                    self.snapshot.proposed = None;
                    self.snapshot.summary = None;
                    self.target = None;
                    let cancel = control.begin(Stage::Checking)?;
                    let preview = runtime.preview_relocation(&path, &cancel);
                    let prepared = (|| -> Result<()> {
                        match preview? {
                            RelocationPreview::Unchanged => {
                                self.snapshot.notice = Some(Notice::Unchanged)
                            }
                            RelocationPreview::Destination(mut target) => {
                                let observed = runtime.relocation_summary(&cancel)?;
                                target.check_capacity(observed.bytes)?;
                                // Retain the actual target even when a writable probe fails.
                                let writable = target.check_relocation_writable(&cancel);
                                self.snapshot.proposed = Some(target.resolved().into());
                                self.snapshot.summary = Some(observed);
                                self.snapshot.recovery_required = target.has_retained_probes();
                                self.target = Some(*target);
                                writable?;
                                self.snapshot.can_confirm = true;
                                self.snapshot.notice = Some(Notice::Selected);
                            }
                        }
                        Ok(())
                    })();
                    if self.snapshot.recovery_required {
                        control.retain();
                    } else {
                        control.finish();
                    }
                    prepared?;
                }
                Action::Confirm => {
                    if !self.snapshot.can_confirm {
                        return Err(StorageError::SnapshotChanged);
                    }
                    let target = self.target.take().ok_or(StorageError::SnapshotChanged)?;
                    self.snapshot.can_confirm = false;
                    let id = librairii_adapters::identities::job_id()
                        .map_err(|source| StorageError::Io {
                            operation: "generate move identity",
                            source,
                        })?
                        .to_string();
                    let _cancel = control.begin(Stage::Copying)?;
                    match runtime.move_library_controlled(target, id, control.clone()) {
                        Ok(action) => return Ok(Some(action)),
                        Err(error) => {
                            control.finish();
                            return Err(match error {
                                librairii_app::lifecycle::LifecycleError::Busy => {
                                    StorageError::RuntimeInUse
                                }
                                _ => StorageError::RuntimeFenced,
                            });
                        }
                    }
                }
                Action::Diagnostics(path) => {
                    let cancel = control.begin(Stage::Diagnostics)?;
                    let report = runtime.diagnostics(&path, &cancel);
                    self.snapshot.recovery_required = runtime.diagnostics_recovery_required();
                    if self.snapshot.recovery_required {
                        control.retain();
                    } else {
                        control.finish();
                    }
                    let report = report?;
                    self.snapshot.diagnostic_name = Some(report.file_name);
                    self.snapshot.notice = Some(Notice::DiagnosticsSaved);
                }
                Action::Clear => {
                    self.target = None;
                    self.snapshot.proposed = None;
                    self.snapshot.summary = None;
                    self.snapshot.can_confirm = false;
                }
                _ => return Err(StorageError::InvalidNavigation),
            }
            Ok(None)
        })();
        match result {
            Ok(action) => Ok(action),
            Err(error) => {
                if matches!(error, StorageError::RuntimeInUse | StorageError::QueueFull) {
                    self.snapshot.notice = Some(Notice::Busy);
                }
                self.snapshot.recovery_required |= matches!(
                    error,
                    StorageError::RuntimeFenced
                        | StorageError::CustodyRecoveryRequired
                        | StorageError::ProfileRestartRequired
                );
                self.snapshot.error = Some(error.to_string());
                Ok(None)
            }
        }
    }
}

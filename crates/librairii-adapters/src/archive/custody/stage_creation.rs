//! Local routing of an actual storage-issued stage creation result.
//! No filesystem allocation, cleanup, error conversion or authority construction.

use super::{CopyOutcome, QualificationEnvelope, copy_to_stage};
use librairii_storage::{
    database::WorkCancellation,
    platform::unix::{Custody, OwnedReadSource, OwnedStage, StageCreateFailure},
};

/// A failed creation still owns its actual source and exclusion. In particular,
/// RetainedUnresolved does not mean that nothing was created. NoAllocation is
/// THIS call only; it settles no earlier attempt and clears no fence.
///
/// Fields close source/failure resources before the guard in normal field-drop
/// order. The caller must physically retain unresolved ownership until an actual
/// qualified resolution/handoff; Drop is neither cleanup nor settlement.
#[must_use = "retain the original failure/source/custody until actual resolution"]
#[derive(Debug)]
pub struct CreationFailure<'files, 'library> {
    pub source: OwnedReadSource<'files, 'library>,
    pub failure: Box<StageCreateFailure<'files, 'library>>,
    pub custody: Custody<'library>,
    /// Orthogonal observation; does not erase or replace failure.cause().
    pub cancellation_requested: bool,
}

/// No imported/committed/accepted-inspection fact is constructed. A CopyAttempt
/// may itself contain a work error and actual unused-temp retirement result.
///
/// Positive type control: the real creation result can be bound/matched and
/// retained WITHOUT an Error/From conversion. This function is compiled only;
/// it constructs no fixture, executes no factory and drops no unresolved owner.
/// ```no_run
/// use librairii_adapters::archive::custody::{
///     copy_created_stage, CreationOutcome, QualificationEnvelope};
/// use librairii_storage::{database::WorkCancellation,
///     platform::unix::{LibraryFiles, OwnedReadSource, Custody}};
/// fn retain_result<'files, 'library>(
///     files: &'files LibraryFiles<'library>,
///     source: OwnedReadSource<'files, 'library>,
///     guard: Custody<'library>, cancellation: &WorkCancellation,
/// ) -> CreationOutcome<'files, 'library> {
///     let creation = files.create_stage(&guard);
///     copy_created_stage(source, creation, guard,
///         QualificationEnvelope::default(), cancellation)
/// }
/// ```
/// Borrowed error observation also requires no erased/cloned bare error. The
/// actual failure owner stays borrowed inside the outcome throughout reporting.
/// ```no_run
/// use librairii_adapters::archive::custody::CreationOutcome;
/// use librairii_storage::{StorageError, platform::unix::StageCreateDisposition};
/// fn observe<'borrow>(outcome: &'borrow CreationOutcome<'_, '_>)
///     -> Option<(&'borrow StorageError, StageCreateDisposition)> {
///     match outcome {
///         CreationOutcome::CreationFailed(owned) =>
///             Some((owned.failure.cause(), owned.failure.disposition())),
///         CreationOutcome::CopyAttempt(_) => None,
///     }
/// }
/// ```
#[must_use = "the result physically owns the source, creation/copy result and custody"]
#[derive(Debug)]
pub enum CreationOutcome<'files, 'library> {
    CopyAttempt(Box<CopyOutcome<'files, 'library>>),
    CreationFailed(CreationFailure<'files, 'library>),
}

/// Route the caller's real creation result within its SAME Files/DB/Native/
/// profile lexical frame. Success delegates the existing algorithm unchanged.
/// Failure moves the WHOLE box; no partial digest, cleanup, retry, fence clear,
/// path scan or conversion to StorageError/string takes place.
pub fn copy_created_stage<'files, 'library>(
    source: OwnedReadSource<'files, 'library>,
    creation: Result<OwnedStage<'files, 'library>, Box<StageCreateFailure<'files, 'library>>>,
    custody: Custody<'library>,
    envelope: QualificationEnvelope,
    cancellation: &WorkCancellation,
) -> CreationOutcome<'files, 'library> {
    match creation {
        Ok(stage) => CreationOutcome::CopyAttempt(Box::new(copy_to_stage(
            source,
            stage,
            custody,
            envelope,
            cancellation,
        ))),
        Err(failure) => CreationOutcome::CreationFailed(CreationFailure {
            source,
            failure,
            custody,
            cancellation_requested: cancellation.is_cancelled(),
        }),
    }
}

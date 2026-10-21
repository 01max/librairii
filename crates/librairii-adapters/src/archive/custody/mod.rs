//! Synchronous, successful-stage-input qualification mechanism only.
//!
//! The caller retains the actual Native/profile/Database/Files lexical frame
//! and supplies its managed source, newly created empty stage and SAME custody
//! guard. This module neither constructs those capabilities nor admits an
//! import. Readonly metadata/hash agreement is cooperative observation, never
//! an adversarial snapshot. Issuer/stage-allocation failures are outside scope.
#![cfg(any(target_os = "macos", target_os = "linux"))]

use librairii_storage::{
    StorageError,
    database::WorkCancellation,
    platform::{
        HASH_BUFFER_BYTES, StreamDigest, hash_stream,
        unix::{Custody, OwnedReadSource, OwnedStage, RetiredTemp, TempCleanupFailure},
    },
};
use std::{
    fmt,
    io::{self, Read, Seek, SeekFrom, Write},
};

/// An optional limit for explicitly bounded qualification inputs. This is NOT
/// a product source-size policy or the expanded archive budget.
#[derive(Clone, Copy, Debug, Default)]
pub struct QualificationEnvelope {
    pub source_bytes: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CopyPhase {
    Input,
    HashBefore,
    Copy,
    HashAfter,
    MeasureStage,
    Complete,
    RetireUnused,
}

/// Local mechanism errors, not legacy inspection/outcome codes.
#[derive(Debug)]
pub enum CopyError {
    Storage {
        phase: CopyPhase,
        cause: StorageError,
    },
    Io {
        phase: CopyPhase,
        operation: &'static str,
        cause: io::Error,
    },
    Cancelled(CopyPhase),
    QualificationLimit {
        observed: u64,
        limit: u64,
    },
    LengthChanged {
        expected: u64,
        observed: u64,
    },
    StageNotEmpty,
    DigestChanged,
    CounterOverflow,
}
impl fmt::Display for CopyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "prepublication copy: {self:?}")
    }
}
impl std::error::Error for CopyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Storage { cause, .. } => Some(cause),
            Self::Io { cause, .. } => Some(cause),
            _ => None,
        }
    }
}

/// Acknowledged copy progress only. An I/O error may have written unacknowledged
/// bytes; the retained actual stage, not this counter, owns that physical prefix.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CopyProgress {
    /// Copy-pass bytes returned by Read, including any one growth sentinel.
    pub read_bytes: u64,
    pub written_bytes: u64,
    pub source_eof: bool,
    pub write_error_may_have_written: bool,
}

#[derive(Debug)]
pub enum StageOutcome<'files, 'library> {
    /// Full EOF measurements agreed. Still NOT inspection/publication/commit.
    Measured {
        stage: OwnedStage<'files, 'library>,
        measurement: StreamDigest,
    },
    /// Recoverable owned trash, NOT deletion or clean native finalization.
    Retired(RetiredTemp<'files, 'library>),
    /// Actual FD/directory/source+trash witnesses and independent cleanup cause.
    /// The caller physically retains these resources AND the guard below.
    RetirementFailed(Box<TempCleanupFailure<'files, 'library>>),
}

/// Every outcome physically retains source/name/FD and custody. The host must
/// hold unresolved resources/exclusion until actual resolution or an actual
/// acknowledged guarded handoff; returning/enqueueing a DTO is not settlement.
///
/// The actual Database cannot close before the returned owners drain:
/// ```compile_fail
/// use librairii_adapters::archive::custody::{copy_to_stage, QualificationEnvelope};
/// use librairii_storage::{Result, database::{Database, WorkCancellation},
///     platform::unix::{LibraryFiles, EntryName, FileArea}};
/// fn close_before_outcome(database: &mut Database<'_>) -> Result<()> {
///     let files = LibraryFiles::open_native(database)?;
///     let guard = files.try_custody()?;
///     let source = files.open_read_source(FileArea::Archives, EntryName::parse("owned")?)?;
///     let stage = files.create_stage(&guard)?;
///     let outcome = copy_to_stage(source, stage, guard, QualificationEnvelope::default(),
///         &WorkCancellation::default());
///     database.close()?;
///     drop(outcome);
///     Ok(())
/// }
/// ```
/// A path/ID observation cannot replace the actual borrowed Files owner:
/// ```compile_fail
/// use librairii_adapters::archive::custody::{copy_to_stage, QualificationEnvelope};
/// use librairii_storage::{Result, database::{Database, WorkCancellation},
///     platform::unix::{LibraryFiles, EntryName, FileArea}};
/// fn discard_files(database: &mut Database<'_>) -> Result<()> {
///     let files = LibraryFiles::open_native(database)?;
///     let guard = files.try_custody()?;
///     let source = files.open_read_source(FileArea::Archives, EntryName::parse("owned")?)?;
///     let stage = files.create_stage(&guard)?;
///     let outcome = copy_to_stage(source, stage, guard, QualificationEnvelope::default(),
///         &WorkCancellation::default());
///     drop(files);
///     drop(outcome);
///     Ok(())
/// }
/// ```
#[must_use = "retain the actual source/stage/custody owners through resolution"]
#[derive(Debug)]
pub struct CopyOutcome<'files, 'library> {
    pub source: OwnedReadSource<'files, 'library>,
    pub stage: StageOutcome<'files, 'library>,
    // Keep exclusion through the source/stage descriptor lifetimes.
    pub custody: Custody<'library>,
    pub progress: CopyProgress,
    pub error: Option<CopyError>,
    /// Independent observation; never replaces an actual I/O/cleanup cause.
    pub cancellation_requested: bool,
}

/// Copy only an already-issued managed source into an already-successfully-
/// created empty stage under the caller's SAME guard and lexical Files frame.
/// Arbitrary blocking Read cannot be preempted by cooperative checkpoints.
pub fn copy_to_stage<'files, 'library>(
    source: OwnedReadSource<'files, 'library>,
    stage: OwnedStage<'files, 'library>,
    custody: Custody<'library>,
    envelope: QualificationEnvelope,
    cancellation: &WorkCancellation,
) -> CopyOutcome<'files, 'library> {
    copy_with_io(
        source,
        stage,
        custody,
        envelope,
        cancellation,
        &mut DirectIo,
    )
}

// Fault seams BORROW actual owners. There is no generic reader/file constructor
// that replaces their origin, parent/name validation, leases or custody.
trait CopyIo {
    fn checkpoint(&mut self, _phase: CopyPhase) {}
    fn read(
        &mut self,
        source: &mut OwnedReadSource<'_, '_>,
        _phase: CopyPhase,
        bytes: &mut [u8],
    ) -> io::Result<usize> {
        source.read(bytes)
    }
    fn write(&mut self, stage: &mut OwnedStage<'_, '_>, bytes: &[u8]) -> io::Result<usize> {
        stage.write(bytes)
    }
}
struct DirectIo;
impl CopyIo for DirectIo {}

struct SourceView<'borrow, 'files, 'library, I> {
    source: &'borrow mut OwnedReadSource<'files, 'library>,
    io: &'borrow mut I,
    phase: CopyPhase,
}
impl<I: CopyIo> Read for SourceView<'_, '_, '_, I> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.io.read(self.source, self.phase, bytes)
    }
}

fn copy_with_io<'files, 'library>(
    mut source: OwnedReadSource<'files, 'library>,
    mut stage: OwnedStage<'files, 'library>,
    custody: Custody<'library>,
    envelope: QualificationEnvelope,
    cancellation: &WorkCancellation,
    io: &mut impl CopyIo,
) -> CopyOutcome<'files, 'library> {
    let mut progress = CopyProgress::default();
    let work = copy_bytes(
        &mut source,
        &mut stage,
        envelope,
        cancellation,
        io,
        &mut progress,
    );
    let (stage, error) = match work {
        Ok(measurement) => (StageOutcome::Measured { stage, measurement }, None),
        Err(error) => {
            io.checkpoint(CopyPhase::RetireUnused);
            // Mandatory resolution has NO ambient cancellation argument. Only
            // the real unused-stage capability is accepted; never publication,
            // reuse, committed/uncertain bytes or a pathname/hash tuple.
            let retained = match stage.retire_owned_temp(&custody) {
                Ok(retired) => StageOutcome::Retired(retired),
                Err(failure) => StageOutcome::RetirementFailed(failure),
            };
            (retained, Some(error))
        }
    };
    CopyOutcome {
        source,
        stage,
        custody,
        progress,
        error,
        cancellation_requested: cancellation.is_cancelled(),
    }
}

fn check_cancel(cancellation: &WorkCancellation, phase: CopyPhase) -> Result<(), CopyError> {
    if cancellation.is_cancelled() {
        Err(CopyError::Cancelled(phase))
    } else {
        Ok(())
    }
}
fn storage(phase: CopyPhase, cause: StorageError) -> CopyError {
    CopyError::Storage { phase, cause }
}
fn io_error(phase: CopyPhase, operation: &'static str, cause: io::Error) -> CopyError {
    CopyError::Io {
        phase,
        operation,
        cause,
    }
}
fn add(counter: &mut u64, count: usize) -> Result<(), CopyError> {
    *counter = counter
        .checked_add(u64::try_from(count).map_err(|_| CopyError::CounterOverflow)?)
        .ok_or(CopyError::CounterOverflow)?;
    Ok(())
}

fn source_hash(
    source: &mut OwnedReadSource<'_, '_>,
    limit: u64,
    cancellation: &WorkCancellation,
    io: &mut impl CopyIo,
    phase: CopyPhase,
) -> Result<StreamDigest, CopyError> {
    io.checkpoint(phase);
    check_cancel(cancellation, phase)?;
    source.revalidate().map_err(|cause| storage(phase, cause))?;
    source
        .rewind()
        .map_err(|cause| io_error(phase, "rewind source", cause))?;
    hash_stream(&mut SourceView { source, io, phase }, limit, cancellation)
        .map_err(|cause| storage(phase, cause))
}

fn copy_bytes(
    source: &mut OwnedReadSource<'_, '_>,
    stage: &mut OwnedStage<'_, '_>,
    envelope: QualificationEnvelope,
    cancellation: &WorkCancellation,
    io: &mut impl CopyIo,
    progress: &mut CopyProgress,
) -> Result<StreamDigest, CopyError> {
    io.checkpoint(CopyPhase::Input);
    check_cancel(cancellation, CopyPhase::Input)?;
    source
        .revalidate()
        .map_err(|cause| storage(CopyPhase::Input, cause))?;
    let expected = source.initial().bytes();
    if let Some(limit) = envelope.source_bytes
        && expected > limit
    {
        return Err(CopyError::QualificationLimit {
            observed: expected,
            limit,
        });
    }
    // A successful stage constructor is a precondition, not permission to
    // append to pre-existing stage bytes or to hide a previous failed copy.
    let length = stage
        .inspector()
        .seek(SeekFrom::End(0))
        .map_err(|cause| io_error(CopyPhase::Input, "inspect stage length", cause))?;
    if length != 0 {
        return Err(CopyError::StageNotEmpty);
    }
    stage
        .inspector()
        .rewind()
        .map_err(|cause| io_error(CopyPhase::Input, "rewind stage", cause))?;
    let before = source_hash(source, expected, cancellation, io, CopyPhase::HashBefore)?;
    if before.bytes() != expected {
        return Err(CopyError::LengthChanged {
            expected,
            observed: before.bytes(),
        });
    }
    source
        .revalidate()
        .map_err(|cause| storage(CopyPhase::HashBefore, cause))?;
    source
        .rewind()
        .map_err(|cause| io_error(CopyPhase::Copy, "rewind source", cause))?;
    copy_stream(source, stage, expected, cancellation, io, progress)?;
    let after = source_hash(source, expected, cancellation, io, CopyPhase::HashAfter)?;
    if after != before {
        return Err(CopyError::DigestChanged);
    }
    io.checkpoint(CopyPhase::MeasureStage);
    check_cancel(cancellation, CopyPhase::MeasureStage)?;
    let staged = stage
        .measure(expected, cancellation)
        .map_err(|cause| storage(CopyPhase::MeasureStage, cause))?;
    if staged != before {
        return Err(CopyError::DigestChanged);
    }
    io.checkpoint(CopyPhase::Complete);
    source
        .revalidate()
        .map_err(|cause| storage(CopyPhase::Complete, cause))?;
    check_cancel(cancellation, CopyPhase::Complete)?;
    Ok(staged)
}

fn copy_stream(
    source: &mut OwnedReadSource<'_, '_>,
    stage: &mut OwnedStage<'_, '_>,
    expected: u64,
    cancellation: &WorkCancellation,
    io: &mut impl CopyIo,
    progress: &mut CopyProgress,
) -> Result<(), CopyError> {
    io.checkpoint(CopyPhase::Copy);
    let mut buffer = [0u8; HASH_BUFFER_BYTES];
    loop {
        check_cancel(cancellation, CopyPhase::Copy)?;
        let remaining = expected
            .checked_sub(progress.written_bytes)
            .ok_or(CopyError::CounterOverflow)?;
        let request = remaining.saturating_add(1).min(HASH_BUFFER_BYTES as u64) as usize;
        let count = match io.read(source, CopyPhase::Copy, &mut buffer[..request]) {
            Ok(count) if count <= request => count,
            Ok(_) => {
                return Err(io_error(
                    CopyPhase::Copy,
                    "read source",
                    io::Error::new(io::ErrorKind::InvalidData, "read exceeded borrowed buffer"),
                ));
            }
            Err(cause) if cause.kind() == io::ErrorKind::Interrupted => continue,
            Err(cause) => return Err(io_error(CopyPhase::Copy, "read source", cause)),
        };
        add(&mut progress.read_bytes, count)?;
        if count == 0 {
            progress.source_eof = true;
            check_cancel(cancellation, CopyPhase::Copy)?;
            if progress.written_bytes != expected {
                return Err(CopyError::LengthChanged {
                    expected,
                    observed: progress.written_bytes,
                });
            }
            break;
        }
        // Consume at most one sentinel beyond the observed/envelope bound;
        // that sentinel is NEVER written to the stage.
        let allowed = count.min(usize::try_from(remaining).unwrap_or(usize::MAX));
        let mut offset = 0;
        while offset < allowed {
            check_cancel(cancellation, CopyPhase::Copy)?;
            match io.write(stage, &buffer[offset..allowed]) {
                Ok(0) => {
                    return Err(io_error(
                        CopyPhase::Copy,
                        "write stage",
                        io::ErrorKind::WriteZero.into(),
                    ));
                }
                Ok(written) if written <= allowed - offset => {
                    offset += written;
                    add(&mut progress.written_bytes, written)?;
                }
                Ok(_) => {
                    progress.write_error_may_have_written = true;
                    return Err(io_error(
                        CopyPhase::Copy,
                        "write stage",
                        io::Error::new(
                            io::ErrorKind::InvalidData,
                            "write exceeded borrowed buffer",
                        ),
                    ));
                }
                Err(cause) if cause.kind() == io::ErrorKind::Interrupted => continue,
                Err(cause) => {
                    progress.write_error_may_have_written = true;
                    return Err(io_error(CopyPhase::Copy, "write stage", cause));
                }
            }
        }
        if count > allowed {
            return Err(CopyError::LengthChanged {
                expected,
                observed: progress.read_bytes,
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;

//! Import effects over the actual DB/library and retained file owners. Call on
//! bounded blocking workers; no filesystem work executes in SQL closures.
use crate::{
    Result, StorageError,
    database::{Database, TransactionOutcome, WorkCancellation},
    platform::{
        StreamDigest,
        unix::{
            Custody, EntryName, FileArea, LibraryFiles, MetadataStageCleanupFailure, OwnedStage,
            PickedSource, Publication, PublishFailure, StageCreateFailure, TempCleanupFailure,
        },
    },
};
use librairii_app::{
    importing::{Inspection, InspectionError, Inspector},
    native_jobs::{Boundary, Cancellation, DurableReport, Execution, ProgressSink, Report},
    ports::jobs::WorkItem,
};
use librairii_core::operations::{ItemState, RawOutcome};
use std::{io::Write, os::unix::ffi::OsStrExt, path::Path, sync::Arc, thread, time::Duration};
mod catalog;
use catalog::Candidate;
pub use catalog::{Decision, ImportedStory};

#[derive(Debug)]
enum Held<'db> {
    Creation(Box<StageCreateFailure<'db, 'db>>),
    Publication(Box<PublishFailure<'db, 'db>>),
    Cleanup(Box<TempCleanupFailure<'db, 'db>>),
    Settlement(Box<MetadataStageCleanupFailure<'db, 'db>>),
}
/// Move-only effects stay alive through compensation and durable terminal ack.
#[derive(Debug)]
pub struct ImportResources<'db> {
    settled: bool,
    operation: String,
    item: i64,
    expected: Option<(ItemState, i64, i64, RawOutcome)>,
    files: Arc<LibraryFiles<'db>>,
    extra_files: Vec<Arc<LibraryFiles<'db>>>,
    guard: Option<Custody<'db>>,
    stage: Option<OwnedStage<'db, 'db>>,
    artwork_stage: Option<OwnedStage<'db, 'db>>,
    publications: Vec<Publication<'db, 'db>>,
    held: Vec<Held<'db>>,
    candidate: Option<Candidate>,
    rollback: bool,
}
impl Drop for ImportResources<'_> {
    fn drop(&mut self) {
        if !self.settled {
            self.files.fence_metadata();
        }
    }
}
impl ImportResources<'_> {
    /// Bounded diagnostic categories only; retained owners and path details
    /// stay private and cannot become cleanup instructions.
    pub fn retained_failures(&self) -> Vec<&'static str> {
        self.held
            .iter()
            .map(|held| match held {
                Held::Creation(f) => {
                    let _ = f.retained_allocation();
                    "creation"
                }
                Held::Publication(f) => {
                    let _ = f.publish_uncertain;
                    "publication"
                }
                Held::Cleanup(f) => {
                    let _ = f.cleanup_uncertain;
                    "cleanup"
                }
                Held::Settlement(f) => {
                    let _ = &f.cause;
                    "settlement"
                }
            })
            .collect()
    }
}
#[derive(Debug)]
pub struct ImportStorage<'db, 'library> {
    database: &'db Database<'library>,
    observer: Option<&'db dyn ImportObserver>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImportPoint {
    SourceOpened,
    BeforeRead,
    AfterCopyChunk,
    BeforeHash,
    Inspected,
    BeforeArchivePublication,
    ArchivePublished,
    BeforeArtworkWrite,
    ArtworkPublished,
    BeforeCatalog,
    BeforeSettlement,
}
/// Optional synchronous effect-boundary observation. A callback can refuse
/// before an effect or coordinate a failure/cancel/crash test. It has no file
/// authority, cannot fabricate successful effects and never runs in SQL.
pub trait ImportObserver: Sync + std::fmt::Debug {
    fn before(&self, _point: ImportPoint, _cancel: &Cancellation) -> Result<()> {
        Ok(())
    }
    fn committed(&self, _cancel: &Cancellation) {}
}
#[derive(Clone, Copy, Debug)]
struct Problem(&'static str);
impl<'db, 'library> ImportStorage<'db, 'library> {
    pub const fn new(database: &'db Database<'library>) -> Self {
        Self {
            database,
            observer: None,
        }
    }
    pub const fn observed(
        database: &'db Database<'library>,
        observer: &'db dyn ImportObserver,
    ) -> Self {
        Self {
            database,
            observer: Some(observer),
        }
    }
    fn observe(
        &self,
        point: ImportPoint,
        cancel: &Cancellation,
        code: &'static str,
    ) -> std::result::Result<(), Problem> {
        self.observer
            .map_or(Ok(()), |o| o.before(point, cancel))
            .map_err(|_| Problem(code))
    }
    /// The path is explicit read-only picker input; it cannot authorize managed
    /// mutation. The passed WorkItem supplies correlation, never cleanup rights.
    pub fn execute(
        &self,
        work: &WorkItem,
        path: &Path,
        inspector: &impl Inspector,
        cancel: &Cancellation,
        progress: &ProgressSink,
    ) -> Execution<Box<ImportResources<'db>>> {
        let source = PickedSource::open(path);
        let files = self.database.metadata_files();
        let (source, files) = match (source, files) {
            (Ok(source), Ok(files)) => (source, Arc::new(files)),
            _ => return failed(work, cancel, "stage_failed", None),
        };
        let mut resources = Box::new(ImportResources {
            settled: false,
            operation: work.operation.identity.id.clone(),
            item: work.item.identity.id,
            expected: None,
            files,
            extra_files: Vec::new(),
            guard: None,
            stage: None,
            artwork_stage: None,
            publications: Vec::new(),
            held: Vec::new(),
            candidate: None,
            rollback: false,
        });
        let result = self.run(work, source, inspector, cancel, progress, &mut resources);
        match result {
            Ok(decision) => {
                if matches!(decision, Decision::Imported(_))
                    && let Some(observer) = self.observer
                {
                    observer.committed(cancel);
                }
                let (state, message) = match &decision {
                    Decision::Imported(_) => (ItemState::Succeeded, "Story imported."),
                    Decision::Duplicate(_) => (
                        ItemState::Skipped,
                        "This archive is already in the collection.",
                    ),
                    Decision::Conflict(_) => (
                        ItemState::Conflicted,
                        "Different archive bytes already use this story UUID.",
                    ),
                };
                let report = Report {
                    state,
                    story_id: decision.story().story_id,
                    completed_bytes: work.item.progress.total,
                    outcome: RawOutcome {
                        code: Some(decision.code().into()),
                        message: Some(message.into()),
                    },
                };
                resources.expected = Some((
                    report.state,
                    report.story_id,
                    report.completed_bytes,
                    report.outcome.clone(),
                ));
                Execution {
                    boundary: Boundary::Committed,
                    report,
                    resources: Some(resources),
                }
            }
            Err(Problem(code)) => {
                resources.rollback = !resources.publications.is_empty();
                failed(work, cancel, code, Some(resources))
            }
        }
    }
    fn guard(&self, files: &LibraryFiles<'db>, cancel: &WorkCancellation) -> Result<Custody<'db>> {
        loop {
            if cancel.is_cancelled() {
                return Err(StorageError::Cancelled);
            }
            match files.try_custody() {
                Err(StorageError::CustodyBusy) => thread::sleep(Duration::from_millis(2)),
                other => return other,
            }
        }
    }
    fn stage(
        &self,
        files: Arc<LibraryFiles<'db>>,
        guard: &Custody<'db>,
        work: &WorkItem,
        r: &mut ImportResources<'db>,
        artwork: bool,
    ) -> std::result::Result<(), Problem> {
        let creation = LibraryFiles::create_owned_stage(files, guard);
        match creation {
            Ok(mut stage) => {
                let registration = if self.database.native_import_registration() {
                    stage.register_operation(
                        guard,
                        work.operation.identity.id.clone(),
                        work.item.identity.id,
                    )
                } else {
                    Ok(())
                };
                if artwork {
                    r.artwork_stage = Some(stage);
                } else {
                    r.stage = Some(stage);
                }
                registration.map_err(|_| Problem("stage_failed"))
            }
            Err(failure) => {
                if failure.retained_allocation().is_some() {
                    r.held.push(Held::Creation(failure));
                }
                Err(Problem("stage_failed"))
            }
        }
    }
    fn run(
        &self,
        work: &WorkItem,
        mut source: PickedSource,
        inspector: &impl Inspector,
        cancel: &Cancellation,
        progress: &ProgressSink,
        r: &mut ImportResources<'db>,
    ) -> std::result::Result<Decision, Problem> {
        if work.operation.kind != librairii_core::OperationKind::Import
            || source.bytes() != work.item.progress.total as u64
        {
            return Err(Problem("stage_failed"));
        }
        let cancellation = WorkCancellation::for_job(cancel);
        self.observe(ImportPoint::SourceOpened, cancel, "stage_failed")?;
        let guard = self
            .guard(&r.files, &cancellation)
            .map_err(|_| Problem("stage_failed"))?;
        self.stage(r.files.clone(), &guard, work, r, false)?;
        // Private files have their own move-only owner. Release global custody
        // while copying/inspecting so other workers may make independent progress.
        drop(guard);
        let measured = source
            .copy_observed(
                r.stage.as_mut().ok_or(Problem("stage_failed"))?,
                &cancellation,
                |point| {
                    use crate::platform::unix::CopyPoint;
                    let point = match point {
                        CopyPoint::Read => ImportPoint::BeforeRead,
                        CopyPoint::Written(bytes) => {
                            let _ = progress.advance(bytes as i64);
                            ImportPoint::AfterCopyChunk
                        }
                        CopyPoint::Hash => ImportPoint::BeforeHash,
                    };
                    self.observer.map_or(Ok(()), |o| o.before(point, cancel))
                },
            )
            .map_err(|_| Problem("stage_failed"))?;
        let _ = progress.advance(measured.bytes() as i64);
        let guard = self
            .guard(&r.files, &cancellation)
            .map_err(|_| Problem("lookup_failed"))?;
        let checksum = measured.checksum();
        let existing = self
            .database
            .barrier_read(move |tx| catalog::by_hash(tx, checksum))
            .and_then(|p| p.wait())
            .map_err(|_| Problem("lookup_failed"))?;
        if let Some(existing) = existing {
            r.guard = Some(guard);
            return Ok(Decision::Duplicate(existing));
        }
        drop(guard);
        let original = source.name().as_os_str().as_bytes().to_vec();
        let metadata = inspector
            .inspect(
                &mut r.stage.as_mut().ok_or(Problem("stage_failed"))?.inspector(),
                &original,
                cancel,
            )
            .map_err(|e| match e {
                InspectionError::Cancelled => Problem("cancelled"),
                InspectionError::Validation(e) => Problem(e.code.as_str()),
            })?;
        source.revalidate().map_err(|_| Problem("stage_failed"))?;
        self.observe(ImportPoint::Inspected, cancel, "inspection_failed")?;
        let guard = self
            .guard(&r.files, &cancellation)
            .map_err(|_| Problem("publish_failed"))?;
        let mut managed = format!("archives/{checksum}/").into_bytes();
        managed.extend_from_slice(&original);
        let candidate = Candidate {
            metadata,
            filename: original,
            managed,
            checksum,
            bytes: measured.bytes() as i64,
            artwork: String::new(),
        };
        let check = candidate.clone();
        let existing = self
            .database
            .barrier_read(move |tx| catalog::before_publish(tx, &check))
            .and_then(|p| p.wait())
            .map_err(|_| Problem("lookup_failed"))?;
        if let Some(existing) = existing {
            r.guard = Some(guard);
            return Ok(existing);
        }
        r.guard = Some(guard);
        r.candidate = Some(candidate);
        self.observe(
            ImportPoint::BeforeArchivePublication,
            cancel,
            "publish_failed",
        )?;
        self.publish_archive(r, source.name().clone(), measured, &cancellation)?;
        self.observe(ImportPoint::ArchivePublished, cancel, "publish_failed")?;
        self.publish_artwork(work, r, &cancellation, cancel)?;
        self.observe(ImportPoint::BeforeCatalog, cancel, "catalog_failed")?;
        let candidate = r
            .candidate
            .as_ref()
            .ok_or(Problem("catalog_failed"))?
            .clone();
        let result = self
            .database
            .transaction(cancellation, move |tx| catalog::create(tx, &candidate))
            .and_then(|p| p.wait());
        match result {
            Ok(TransactionOutcome::Committed(decision)) => Ok(decision),
            Ok(TransactionOutcome::ProvenNotCommitted(_)) => Err(Problem("catalog_failed")),
            // An uncertain write is never repeated. A same-writer barrier reads
            // back exact committed catalog facts before rollback can be considered.
            Ok(TransactionOutcome::CommitUncertain(_)) | Err(StorageError::WorkerStopped) => {
                let c = r
                    .candidate
                    .as_ref()
                    .ok_or(Problem("catalog_failed"))?
                    .clone();
                loop {
                    match self
                        .database
                        .barrier_read({
                            let c = c.clone();
                            move |tx| catalog::readback(tx, &c)
                        })
                        .and_then(|p| p.wait())
                    {
                        Ok(Some(row)) => {
                            return Ok(Decision::Imported(row));
                        }
                        Ok(None) => return Err(Problem("catalog_failed")),
                        _ => thread::sleep(Duration::from_millis(10)),
                    }
                }
            }
            Err(_) => Err(Problem("catalog_failed")),
        }
    }
    fn publish_archive(
        &self,
        r: &mut ImportResources<'db>,
        name: EntryName,
        measured: StreamDigest,
        cancel: &WorkCancellation,
    ) -> std::result::Result<(), Problem> {
        let stage = r.stage.take().ok_or(Problem("publish_failed"))?;
        match stage.publish_archive(
            r.guard.as_ref().ok_or(Problem("publish_failed"))?,
            name,
            measured,
            cancel,
        ) {
            Ok(receipt) => {
                r.publications.push(receipt);
                Ok(())
            }
            Err(f) => {
                if f.publish_uncertain {
                    r.held.push(Held::Publication(f));
                } else {
                    r.stage = Some(f.staged);
                }
                Err(Problem("publish_failed"))
            }
        }
    }
    fn publish_artwork(
        &self,
        work: &WorkItem,
        r: &mut ImportResources<'db>,
        cancel: &WorkCancellation,
        job: &Cancellation,
    ) -> std::result::Result<(), Problem> {
        let c = r.candidate.as_mut().ok_or(Problem("artwork_failed"))?;
        let Some(bytes) = c.metadata.artwork.take() else {
            return Ok(());
        };
        self.observe(ImportPoint::BeforeArtworkWrite, job, "artwork_failed")?;
        let hash = librairii_app::metadata::digest(&bytes);
        c.artwork = format!(
            "catalog/embedded/{}/{hash}.png",
            c.metadata.identity.canonical()
        );
        let uuid = c.metadata.identity.typed();
        let guard = r.guard.take().ok_or(Problem("artwork_failed"))?;
        let mut files = self
            .database
            .metadata_files()
            .map_err(|_| Problem("artwork_failed"))?;
        let route = files.embedded_artwork(&guard, uuid);
        let files = Arc::new(files);
        r.extra_files.push(files.clone());
        let create = route
            .map_err(|_| Problem("artwork_failed"))
            .and_then(|_| self.stage(files, &guard, work, r, true));
        r.guard = Some(guard);
        create?;
        let mut stage = r.artwork_stage.take().ok_or(Problem("artwork_failed"))?;
        let write = (|| {
            for bytes in bytes.chunks(64 * 1024) {
                if cancel.is_cancelled() {
                    return Err(StorageError::Cancelled);
                }
                stage
                    .write_all(bytes)
                    .map_err(|e| StorageError::io("write embedded artwork", e))?;
            }
            stage.measure(bytes.len() as u64, cancel)
        })();
        let measured = match write {
            Ok(m) if m.checksum() == hash => m,
            _ => {
                r.artwork_stage = Some(stage);
                return Err(Problem("artwork_failed"));
            }
        };
        match stage.publish_no_clobber(
            r.guard.as_ref().ok_or(Problem("artwork_failed"))?,
            FileArea::Catalog,
            EntryName::parse(format!("{hash}.png")).map_err(|_| Problem("artwork_failed"))?,
            measured,
            cancel,
        ) {
            Ok(receipt) => {
                r.publications.push(receipt);
                self.observe(ImportPoint::ArtworkPublished, job, "artwork_failed")?;
                Ok(())
            }
            Err(f) => {
                if f.publish_uncertain {
                    r.held.push(Held::Publication(f));
                } else {
                    r.artwork_stage = Some(f.staged);
                }
                Err(Problem("artwork_failed"))
            }
        }
    }
    /// Required compensation is uncancelled and requires actual same-writer
    /// absence plus the original file receipts and same retained custody guard.
    pub fn compensate(&self, r: &mut ImportResources<'db>) -> bool {
        if !r.rollback {
            return true;
        }
        let Some(c) = r.candidate.as_ref().cloned() else {
            return false;
        };
        let absent = self
            .database
            .barrier_read(move |tx| catalog::absent(tx, &c))
            .and_then(|p| p.wait());
        if !matches!(absent, Ok(true)) {
            return false;
        }
        let Some(guard) = r.guard.as_ref() else {
            return false;
        };
        let candidate = match &r.candidate {
            Some(c) => c,
            None => return false,
        };
        let mut paths = vec![candidate.managed.clone()];
        if !candidate.artwork.is_empty() {
            paths.push(candidate.artwork.as_bytes().to_vec());
        }
        let proof = UnreferencedImport {
            _database: self.database,
            guard,
            paths,
        };
        while let Some(receipt) = r.publications.pop() {
            match receipt.rollback_import(guard, &proof) {
                Ok(stage) => {
                    if let Err(f) = stage.retire_owned_temp(guard) {
                        r.held.push(Held::Cleanup(f));
                        return false;
                    }
                }
                Err(f) => {
                    r.held.push(Held::Publication(f));
                    return false;
                }
            }
        }
        r.rollback = false;
        r.held.is_empty()
    }
    /// Called only after the runtime's actual durable report acknowledgement.
    pub fn settle(
        &self,
        mut r: Box<ImportResources<'db>>,
        durable: &DurableReport<'_>,
    ) -> std::result::Result<(), Box<ImportResources<'db>>> {
        if self.observer.is_some_and(|o| {
            o.before(ImportPoint::BeforeSettlement, &Cancellation::default())
                .is_err()
        }) {
            return Err(r);
        }
        let command = durable.command();
        let expected = (
            command.state,
            command.story_id,
            command.completed_bytes,
            command.outcome.clone(),
        );
        if command.operation.identity.id != r.operation
            || command.item.id != r.item
            || r.expected.as_ref() != Some(&expected)
            || durable
                .interrupted()
                .is_some_and(|h| h.identity.id != r.operation)
            || r.rollback
            || !r.held.is_empty()
        {
            return Err(r);
        }
        if r.guard.is_none() {
            match self.guard(&r.files, &WorkCancellation::default()) {
                Ok(guard) => r.guard = Some(guard),
                Err(_) => return Err(r),
            }
        }
        let Some(guard) = r.guard.as_ref() else {
            return Err(r);
        };
        for stage in [r.artwork_stage.take(), r.stage.take()]
            .into_iter()
            .flatten()
        {
            let cleanup = if command.outcome.code.as_deref() == Some("duplicate_checksum") {
                stage.discard_duplicate_stage(guard)
            } else {
                stage.retire_owned_temp(guard).map(|_| ())
            };
            if let Err(f) = cleanup {
                r.held.push(Held::Cleanup(f));
                return Err(r);
            }
        }
        while let Some(receipt) = r.publications.pop() {
            if let Err(f) = receipt.finish_metadata_stage(guard) {
                r.held.push(Held::Settlement(f));
                return Err(r);
            }
        }
        r.settled = true;
        Ok(())
    }
}
/// No public constructor, no path/hash fields and no serialized authority. Only
/// this module's same-writer absence observation under the live custody guard
/// constructs the transient permission used WITH an original publication owner.
pub(crate) struct UnreferencedImport<'guard, 'db> {
    _database: &'guard Database<'db>,
    guard: &'guard Custody<'db>,
    paths: Vec<Vec<u8>>,
}
impl UnreferencedImport<'_, '_> {
    pub(crate) fn matches(&self, guard: &Custody<'_>, path: &[u8]) -> bool {
        std::ptr::eq(self.guard, guard) && self.paths.iter().any(|p| p == path)
    }
}
fn failed<'db>(
    work: &WorkItem,
    cancel: &Cancellation,
    code: &str,
    mut resources: Option<Box<ImportResources<'db>>>,
) -> Execution<Box<ImportResources<'db>>> {
    let cancelled = cancel.is_cancelled();
    let boundary = if resources.as_ref().is_some_and(|r| r.rollback) {
        Boundary::Compensating
    } else {
        Boundary::BeforeCommit
    };
    let report = Report {
        state: if cancelled {
            ItemState::Cancelled
        } else {
            ItemState::Failed
        },
        story_id: 0,
        completed_bytes: 0,
        outcome: RawOutcome {
            code: Some(if cancelled { "cancelled" } else { code }.into()),
            message: Some(
                if cancelled {
                    "Import cancelled."
                } else {
                    failure_message(code)
                }
                .into(),
            ),
        },
    };
    if let Some(r) = resources.as_mut() {
        r.expected = Some((
            report.state,
            report.story_id,
            report.completed_bytes,
            report.outcome.clone(),
        ));
    }
    debug_assert!(
        resources
            .as_ref()
            .is_none_or(|r| r.operation == work.operation.identity.id)
    );
    Execution {
        boundary,
        report,
        resources,
    }
}
fn failure_message(code: &str) -> &'static str {
    use librairii_core::import::ValidationCode;
    match ValidationCode::from_code(code) {
        Some(
            ValidationCode::UnsafePath
            | ValidationCode::LinkEntry
            | ValidationCode::NestedArchive
            | ValidationCode::EntryLimit
            | ValidationCode::ExpandedSizeLimit
            | ValidationCode::CompressionRatioLimit
            | ValidationCode::MetadataLimit
            | ValidationCode::ArtworkLimit,
        ) => "The archive exceeds safe import limits.",
        Some(ValidationCode::InvalidUuid) => "The archive does not contain a valid story UUID.",
        Some(_) => "The file is not a supported story archive.",
        None => "The file could not be imported.",
    }
}

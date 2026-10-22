//! Prepared native export composition over the admitted Task 6 job session.
use super::{preparation::Running, *};
use crate::{
    StorageError,
    database::WorkCancellation,
    operations_repository::TerminalRepository,
    platform::unix::{
        Custody, ExportTemporaryFailure, LibraryFiles, OwnedExportTemporary,
        transfer::ArchiveSource,
    },
};
use librairii_app::{
    native_jobs::*,
    ports::{
        creation::{NewItem, NewOperation, PlannedItem},
        jobs::WorkItem,
    },
};
use librairii_core::{
    OperationKind,
    operations::{ExportSource, ExportSourceType, ItemState, RawOutcome},
};
use std::{thread, time::Duration};
#[derive(Debug, thiserror::Error)]
pub enum DurableExportError {
    #[error(transparent)]
    Storage(#[from] StorageError),
    #[error(transparent)]
    Session(#[from] SessionError),
}
impl Exports<'_, '_> {
    /// The event-loop callback receives the one move-only validated Start. It
    /// submits with try_start, observes admission and requests cancellation using
    /// Task 6's bounded handle. All actual file/SQL owners outlive its worker joins.
    pub fn run_prepared<T>(
        &mut self,
        id: &librairii_core::export::PreparationId,
        operation: String,
        configuration: Configuration,
        event_loop: impl FnOnce(Handle, Start) -> T,
    ) -> std::result::Result<T, DurableExportError> {
        self.database.native_file_owner()?;
        let running = self.start(id, &WorkCancellation::default())?;
        let input = running.operation_input(operation)?;
        let operation = input.id.clone();
        let start =
            Start::new(input).map_err(|_| StorageError::Export(ExportError::ResourceLimit))?;
        let database = running.owner.database;
        let files = LibraryFiles::open_native(database)?;
        let store = TerminalRepository::new(database, 1);
        let executor = CopyExecutor {
            running: &running,
            files: &files,
            operation,
        };
        Ok(with_runtime(&store, &executor, configuration, |handle| {
            event_loop(handle, start)
        })?)
    }
}
impl Running<'_, '_, '_> {
    fn operation_input(&self, id: String) -> Result<NewOperation> {
        let source = &self.plan.scope.source;
        let source_type = match source.kind {
            ExportScopeKind::CurrentQuery => ExportSourceType::CurrentQuery,
            ExportScopeKind::Selection => ExportSourceType::Selection,
            ExportScopeKind::CollectionUnion => {
                if source.ids.len() == 1 {
                    ExportSourceType::Collection
                } else {
                    ExportSourceType::Collections
                }
            }
            ExportScopeKind::ShelfUnion | ExportScopeKind::RefinedShelf => {
                if source.ids.len() == 1 {
                    ExportSourceType::CuratedShelf
                } else {
                    ExportSourceType::CuratedShelves
                }
            }
        };
        // Go's JSON encoder replaces invalid UTF-8 in display names. Raw
        // names remain in the retained plan; numeric IDs remain identity.
        let names = source
            .names
            .iter()
            .map(|n| String::from_utf8_lossy(n).into_owned())
            .collect::<Vec<_>>();
        let collections = source_type.group_kind()
            == Some(librairii_core::operations::ExportGroupKind::Collection);
        let grouped = source_type.group_kind().is_some();
        let input = NewOperation {
            id,
            kind: OperationKind::Export,
            source: Some(ExportSource {
                source_type,
                collection_ids: if collections {
                    source.ids.clone()
                } else {
                    Vec::new()
                },
                collection_names: if collections {
                    names.clone()
                } else {
                    Vec::new()
                },
                shelf_ids: if grouped && !collections {
                    source.ids.clone()
                } else {
                    Vec::new()
                },
                shelf_names: if grouped && !collections {
                    names
                } else {
                    Vec::new()
                },
            }),
            destination: Some(self.plan.destination.path().to_owned().into()),
            items: self
                .plan
                .scope
                .stories
                .iter()
                .zip(self.plan.preview.items.iter())
                .map(|(s, p)| {
                    let skip = match p.disposition {
                        librairii_core::export::Disposition::Skipped(
                            librairii_core::export::Issue::StoryMarkedBroken,
                        ) => "story_marked_broken",
                        librairii_core::export::Disposition::Skipped(
                            librairii_core::export::Issue::StoryUnavailable,
                        ) => "story_unavailable",
                        _ => "",
                    };
                    NewItem {
                        story_id: s.id,
                        story_uuid: s.uuid.clone(),
                        story_title: s.title.clone(),
                        source_name: if s.original_filename.is_empty() {
                            s.title.clone()
                        } else {
                            s.original_filename.clone()
                        },
                        output_name: s.original_filename.clone(),
                        archive_relative_path: s.managed_relative_path.clone(),
                        archive_sha256: s.sha256.clone(),
                        export_skip_code: skip.into(),
                        total_bytes: if p.disposition == librairii_core::export::Disposition::Ready
                        {
                            s.byte_size
                        } else {
                            0
                        },
                        planned: match p.disposition {
                            librairii_core::export::Disposition::Ready => None,
                            librairii_core::export::Disposition::Skipped(issue) => {
                                Some(PlannedItem {
                                    state: ItemState::Skipped,
                                    outcome: RawOutcome {
                                        code: Some(issue.code().into()),
                                        message: Some(issue_message(issue).into()),
                                    },
                                })
                            }
                            librairii_core::export::Disposition::Conflicted => Some(PlannedItem {
                                state: ItemState::Conflicted,
                                outcome: RawOutcome {
                                    code: Some("filename_conflict".into()),
                                    message: Some(
                                        "A file with this name already exists in the destination."
                                            .into(),
                                    ),
                                },
                            }),
                        },
                    }
                })
                .collect(),
        };
        input.validate()?;
        Ok(input)
    }
}
struct CopyExecutor<'a, 'run, 'db, 'library> {
    running: &'a Running<'run, 'db, 'library>,
    files: &'a LibraryFiles<'db>,
    operation: String,
}
enum Temporary<'a, 'library> {
    Ready(Box<OwnedExportTemporary<'a, 'library>>),
    Acquisition(Box<ExportTemporaryFailure<'a, 'library>>),
}
struct Resources<'a, 'library> {
    temporary: Temporary<'a, 'library>,
    _source: Option<ArchiveSource<'a, 'library>>,
    custody: Custody<'library>,
    retired: bool,
}
impl Resources<'_, '_> {
    fn retire(&mut self) -> bool {
        if self.retired {
            return true;
        }
        let result = match &mut self.temporary {
            Temporary::Ready(t) => t.retire_in_place(&self.custody),
            Temporary::Acquisition(t) => t.retire_in_place(&self.custody),
        };
        self.retired = result.is_ok();
        self.retired
    }
}
fn pause() {
    thread::sleep(Duration::from_millis(10));
}
fn transient(error: &StorageError) -> bool {
    matches!(
        error,
        StorageError::ProfileBusy | StorageError::QueueFull | StorageError::CustodyBusy
    )
}
fn report(work: &WorkItem, state: ItemState, code: &str, message: &str, bytes: i64) -> Report {
    Report {
        state,
        story_id: work.item.identity.historical_story_id(),
        completed_bytes: bytes,
        outcome: RawOutcome {
            code: Some(code.into()),
            message: Some(message.into()),
        },
    }
}
fn failed(work: &WorkItem, error: &StorageError) -> Report {
    match error {
        StorageError::Cancelled => report(
            work,
            ItemState::Cancelled,
            "cancelled",
            "Story export cancelled.",
            0,
        ),
        StorageError::DestinationCollision => report(
            work,
            ItemState::Conflicted,
            "filename_conflict",
            "A file with this name already exists in the destination.",
            0,
        ),
        StorageError::SnapshotChanged => report(
            work,
            ItemState::Failed,
            "checksum_mismatch",
            "The copied archive failed checksum verification.",
            0,
        ),
        _ => report(
            work,
            ItemState::Failed,
            "export_failed",
            "The story archive could not be exported.",
            0,
        ),
    }
}
impl<'a, 'run, 'db, 'library> Executor for CopyExecutor<'a, 'run, 'db, 'library> {
    type Resources = Resources<'a, 'db>;
    fn execute(
        &self,
        work: &WorkItem,
        cancel: &Cancellation,
        progress: &ProgressSink,
    ) -> Execution<Self::Resources> {
        let before = |report| Execution {
            boundary: Boundary::BeforeCommit,
            report,
            resources: None,
        };
        if work.operation.identity.id != self.operation
            || work.operation.kind != OperationKind::Export
        {
            return before(failed(work, &StorageError::SnapshotChanged));
        }
        let Some(index) = self
            .running
            .plan
            .scope
            .stories
            .iter()
            .position(|s| Some(&s.uuid) == work.item.identity.story_uuid.as_ref())
        else {
            return before(failed(work, &StorageError::SnapshotChanged));
        };
        let disposition = loop {
            match self.running.check_item(index) {
                Ok(d) => break d,
                Err(e) if transient(&e) && !cancel.is_cancelled() => pause(),
                Err(e) => return before(failed(work, &e)),
            }
        };
        use librairii_core::export::Disposition;
        match disposition {
            Disposition::Skipped(issue) => {
                return before(report(
                    work,
                    ItemState::Skipped,
                    issue.code(),
                    issue_message(issue),
                    0,
                ));
            }
            Disposition::Conflicted => {
                return before(failed(work, &StorageError::DestinationCollision));
            }
            Disposition::Ready => {}
        }
        let story = &self.running.plan.scope.stories[index];
        let mut source = loop {
            match ArchiveSource::open(self.files, &story.managed_relative_path) {
                Ok(v) => break v,
                Err(e) if transient(&e) && !cancel.is_cancelled() => pause(),
                Err(e) => return before(failed(work, &e)),
            }
        };
        let custody = loop {
            match self.files.try_custody() {
                Ok(v) => break v,
                Err(e) if transient(&e) && !cancel.is_cancelled() => pause(),
                Err(e) => return before(failed(work, &e)),
            }
        };
        if cancel.is_cancelled() {
            return before(failed(work, &StorageError::Cancelled));
        }
        let mut output = loop {
            match self.files.create_export_temporary(
                &custody,
                self.operation.clone(),
                work.item.identity.id,
            ) {
                Ok(output) => break output,
                Err(failure)
                    if failure.unacquired()
                        && transient(&failure.cause)
                        && !cancel.is_cancelled() =>
                {
                    pause();
                    continue;
                }
                Err(failure) => {
                    let result = failed(work, &failure.cause);
                    return Execution {
                        boundary: Boundary::Compensating,
                        report: result,
                        resources: Some(Resources {
                            temporary: Temporary::Acquisition(failure),
                            _source: Some(source),
                            custody,
                            retired: false,
                        }),
                    };
                }
            }
        };
        let copied = (|| {
            output.matches_destination(&self.running.plan.destination)?;
            let hash = story
                .sha256
                .parse()
                .map_err(|_| StorageError::SnapshotChanged)?;
            let bytes =
                u64::try_from(story.byte_size).map_err(|_| StorageError::SnapshotChanged)?;
            if source.bytes() != bytes {
                return Err(StorageError::SnapshotChanged);
            }
            output.copy_verified(
                &mut source,
                hash,
                bytes,
                || cancel.is_cancelled(),
                |v| {
                    progress.advance(v as i64);
                },
            )?;
            if cancel.is_cancelled() {
                return Err(StorageError::Cancelled);
            }
            progress.phase(Phase::Committing);
            loop {
                match output.publish_registered(&custody, &story.original_filename) {
                    Ok(()) => break Ok(()),
                    Err(e)
                        if !output.publication_pending()
                            && transient(&e)
                            && !cancel.is_cancelled() =>
                    {
                        pause();
                    }
                    Err(_) if output.publication_pending() => {
                        pause();
                    }
                    Err(e) => break Err(e),
                }
            }
        })();
        let (boundary, result) = match copied {
            Ok(()) => (
                Boundary::Committed,
                report(
                    work,
                    ItemState::Succeeded,
                    "exported",
                    "Story archive exported.",
                    story.byte_size,
                ),
            ),
            Err(e) => (Boundary::Compensating, failed(work, &e)),
        };
        Execution {
            boundary,
            report: result,
            resources: Some(Resources {
                temporary: Temporary::Ready(Box::new(output)),
                _source: Some(source),
                custody,
                retired: false,
            }),
        }
    }
    fn compensate(&self, r: &mut Self::Resources) -> bool {
        r.retire()
    }
    fn settle(
        &self,
        mut r: Self::Resources,
        _: &DurableReport<'_>,
    ) -> std::result::Result<(), Self::Resources> {
        if r.retire() { Ok(()) } else { Err(r) }
    }
}

fn issue_message(issue: librairii_core::export::Issue) -> &'static str {
    use librairii_core::export::Issue;
    match issue {
        Issue::StoryMarkedBroken => "This story is marked as broken.",
        Issue::StoryUnavailable => "This approved story is unavailable in the library.",
        Issue::ArchiveMissing => "Managed archive bytes are missing.",
        Issue::ArchiveInvalid => "Managed archive verification failed.",
        Issue::ArchiveChanged => "Managed archive bytes no longer match the imported archive.",
        Issue::UnsupportedExtension => {
            "The stored archive does not have a supported Lunii.QT extension."
        }
        _ => "Story is not eligible for export.",
    }
}

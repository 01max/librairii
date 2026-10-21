//! Blocking-worker orchestration; file reads never run inside a SQL callback.
use super::*;
use crate::platform::unix::transfer::Output;
use crate::{
    StorageError,
    database::WorkCancellation,
    platform::{
        hash_stream,
        unix::{
            LibraryFiles,
            transfer::{ArchiveSource, Destination},
        },
    },
};
use librairii_core::export::{
    Disposition, Issue, PreparationId, Preview, PreviewItem, supported_name,
};
use std::{collections::VecDeque, path::Path, sync::Arc};
#[derive(Debug)]
pub(super) struct Prepared {
    pub id: PreparationId,
    pub scope: Resolved,
    pub destination: Destination,
    pub preview: Preview,
    pub bytes: usize,
}
#[derive(Debug)]
pub struct Exports<'db, 'library> {
    pub(super) database: &'db Database<'library>,
    pub(super) prepared: VecDeque<Prepared>,
    pub(super) retained: Vec<Output>,
}
pub const MAX_PREPARED: usize = 8;
pub const MAX_PREPARED_BYTES: usize = 64 * 1024 * 1024;
#[derive(Debug)]
pub struct Running<'run, 'db, 'library> {
    pub(super) owner: &'run mut Exports<'db, 'library>,
    pub(super) plan: Prepared,
    cursor: usize,
}
impl<'db, 'library> Exports<'db, 'library> {
    pub fn preview(&self, id: &PreparationId) -> Result<Preview> {
        let plan = self
            .prepared
            .iter()
            .find(|p| p.id.same(id))
            .ok_or(ExportError::Stale)?;
        plan.destination.revalidate()?;
        if plan.scope.stories.len() != plan.preview.items.len() {
            return Err(StorageError::CompatibilityInvariant);
        }
        Ok(plan.preview.clone())
    }
    pub fn new(database: &'db Database<'library>) -> Self {
        Self {
            database,
            prepared: VecDeque::new(),
            retained: Vec::new(),
        }
    }
    pub fn retained_outputs(&self) -> usize {
        self.retained.len()
    }
    pub fn preparation_count(&self) -> usize {
        self.prepared.len()
    }
    pub fn invalidate(&mut self) {
        self.prepared.clear();
    }
    /// Consume a plan once. Validation is one coherent snapshot before any
    /// destination effect; later per-item checks do not expand its UUID set.
    pub fn start(
        &mut self,
        id: &PreparationId,
        cancel: &WorkCancellation,
    ) -> Result<Running<'_, 'db, 'library>> {
        if cancel.is_cancelled() {
            return Err(StorageError::Cancelled);
        }
        if !self.retained.is_empty() {
            return Err(StorageError::CustodyRecoveryRequired);
        }
        let index = self
            .prepared
            .iter()
            .position(|p| p.id.same(id))
            .ok_or(ExportError::Stale)?;
        let plan = self.prepared.remove(index).ok_or(ExportError::Stale)?;
        plan.destination.revalidate()?;
        let states = plan
            .scope
            .stories
            .iter()
            .map(|s| (s.uuid.clone(), s.id, s.broken))
            .collect::<Vec<_>>();
        self.database.read(move|tx|{
            use rusqlite::OptionalExtension;
            let mut stmt=tx.prepare("SELECT s.id,s.is_broken FROM stories s JOIN story_archives a ON a.story_id=s.id WHERE s.uuid=?")?;
            for (uuid,id,broken) in states {
                let current=stmt.query_row([uuid],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,bool>(1)?))).optional()?.unwrap_or((0,false));
                if current!=(id,broken) { return Err(ExportError::Stale.into()); }
            }Ok(())
        })?.wait()?;
        Ok(Running {
            owner: self,
            plan,
            cursor: 0,
        })
    }
    pub fn prepare(
        &mut self,
        scope: Scope,
        destination: &Path,
        locale: String,
        cancel: &WorkCancellation,
    ) -> Result<Preview> {
        if cancel.is_cancelled() {
            return Err(StorageError::Cancelled);
        }
        let scope = self.database.resolve_export(scope, locale)?.wait()?;
        let destination = Destination::open(destination)?;
        if !self.retained.is_empty() {
            return Err(StorageError::CustodyRecoveryRequired);
        }
        let files = self.database.metadata_files()?;
        if destination
            .path()
            .starts_with(self.database.library_layout().root())
        {
            return Err(ExportError::Destination.into());
        }
        let mut probe = match destination.create_output() {
            Ok(out) => out,
            Err(failure) => {
                let failure = *failure;
                if let Some(out) = failure.output {
                    self.retained.push(out);
                }
                return Err(failure.cause);
            }
        };
        if let Err(e) = probe.cleanup() {
            self.retained.push(probe);
            return Err(e);
        }
        let mut preview = Preview {
            preparation: None,
            source: scope.source.clone(),
            destination_label: destination.label(),
            items: Arc::from([]),
            ready: 0,
            bytes: 0,
            formats: Vec::new(),
            collapsed_overlap: scope.collapsed_overlap,
            issue: None,
        };
        let mut names = BTreeSet::new();
        let mut formats = BTreeSet::new();
        let mut items = Vec::new();
        for s in &scope.stories {
            if cancel.is_cancelled() {
                return Err(StorageError::Cancelled);
            }
            formats.insert(s.detected_format.clone());
            let disposition = match inspect(&files, s, cancel)? {
                Some(issue) => Disposition::Skipped(issue),
                None => {
                    if !names.insert(
                        librairii_core::export::filename_key(&s.original_filename)
                            .map_err(|_| StorageError::CompatibilityInvariant)?,
                    ) || destination.exists(&s.original_filename)?
                    {
                        Disposition::Conflicted
                    } else {
                        Disposition::Ready
                    }
                }
            };
            if disposition == Disposition::Ready {
                preview.ready += 1;
                preview.bytes = preview
                    .bytes
                    .checked_add(s.byte_size)
                    .ok_or(ExportError::ResourceLimit)?;
            }
            items.push(PreviewItem {
                uuid: s.uuid.clone(),
                title: s.title.clone(),
                output_name: s.original_filename.clone(),
                format: s.detected_format.clone(),
                bytes: s.byte_size,
                disposition,
            });
        }
        preview.items = items.into();
        preview.formats = formats.into_iter().collect();
        preview.issue = if scope.stories.is_empty() {
            Some(Issue::EmptyScope)
        } else if preview.ready == 0 {
            Some(Issue::NoExportableStories)
        } else {
            None
        };
        if preview.can_export() {
            let id = PreparationId::fresh();
            preview.preparation = Some(id.clone());
            let bytes = scope.stories.iter().try_fold(0usize, |n, s| {
                n.checked_add(super::row_bytes(s) * 2)
                    .ok_or(ExportError::ResourceLimit)
            })?;
            if bytes > MAX_PREPARED_BYTES {
                return Err(ExportError::ResourceLimit.into());
            }
            while self.prepared.len() >= MAX_PREPARED
                || self.prepared.iter().map(|p| p.bytes).sum::<usize>() + bytes > MAX_PREPARED_BYTES
            {
                self.prepared.pop_front();
            }
            self.prepared.push_back(Prepared {
                id,
                scope,
                destination,
                preview: preview.clone(),
                bytes,
            });
        }
        Ok(preview)
    }
}
impl Running<'_, '_, '_> {
    /// One blocking copy at a time. Returned rows contain no file/destination
    /// authority. Durable operation reporting/scheduling is owned by Task 6.
    pub fn copy_next(
        &mut self,
        cancel: &WorkCancellation,
        progress: impl FnMut(u64),
    ) -> Result<Option<librairii_core::export::CopyItem>> {
        use librairii_core::export::{CopyItem, CopyStatus};
        if !self.owner.retained.is_empty() {
            return Err(StorageError::CustodyRecoveryRequired);
        }
        let Some(story) = self.plan.scope.stories.get(self.cursor) else {
            return Ok(None);
        };
        let story = story.clone();
        let mut result = CopyItem {
            uuid: story.uuid.clone(),
            output_name: story.original_filename.clone(),
            status: CopyStatus::Failed,
            bytes: 0,
            reason: None,
        };
        let disposition = self.check_item(self.cursor)?;
        self.cursor += 1;
        result.status = match disposition {
            Disposition::Skipped(i) => CopyStatus::Skipped(i),
            Disposition::Conflicted => CopyStatus::Conflicted,
            Disposition::Ready => CopyStatus::Failed,
        };
        if disposition != Disposition::Ready {
            return Ok(Some(result));
        }
        if cancel.is_cancelled() {
            result.status = CopyStatus::Cancelled;
            return Ok(Some(result));
        }
        self.plan.destination.revalidate()?;
        let files = self.owner.database.metadata_files()?;
        let mut source = match ArchiveSource::open(&files, &story.managed_relative_path) {
            Ok(source) => source,
            Err(e) => {
                result.reason = Some(e.code());
                return Ok(Some(result));
            }
        };
        let mut output = match self.plan.destination.create_output() {
            Ok(out) => out,
            Err(f) => {
                let f = *f;
                if let Some(out) = f.output {
                    self.owner.retained.push(out);
                }
                return Err(f.cause);
            }
        };
        let copied = (|| {
            let hash = story
                .sha256
                .parse()
                .map_err(|_| StorageError::CompatibilityInvariant)?;
            let bytes = story
                .byte_size
                .try_into()
                .map_err(|_| StorageError::CompatibilityInvariant)?;
            if source.bytes() != bytes {
                return Err(StorageError::SnapshotChanged);
            }
            output.stream(&mut source, hash, bytes, cancel, progress)?;
            source.validate()?;
            if cancel.is_cancelled() {
                return Err(StorageError::Cancelled);
            }
            output.publish(&story.original_filename)
        })();
        match copied {
            Ok(()) => {
                result.status = CopyStatus::Exported;
                result.bytes = story.byte_size as u64;
            }
            Err(cause) => {
                if output.published || output.uncertain {
                    self.owner.retained.push(output);
                    return Err(cause);
                }
                if let Err(cleanup) = output.cleanup() {
                    self.owner.retained.push(output);
                    return Err(cleanup);
                }
                result.reason = Some(cause.code());
                result.status = match cause {
                    StorageError::Cancelled => CopyStatus::Cancelled,
                    StorageError::DestinationCollision => CopyStatus::Conflicted,
                    _ => CopyStatus::Failed,
                };
            }
        }
        Ok(Some(result))
    }
    pub fn preview(&self) -> &Preview {
        &self.plan.preview
    }
    /// Observation only. A copier calls this immediately before source open.
    /// Previously skipped/conflicted rows retain their original disposition.
    pub fn check_item(&self, index: usize) -> Result<Disposition> {
        let preview = self
            .plan
            .preview
            .items
            .get(index)
            .ok_or(ExportError::InvalidScope)?;
        if preview.disposition != Disposition::Ready {
            return Ok(preview.disposition);
        }
        let original = self.plan.scope.stories[index].clone();
        self.owner.database.read(move|tx|{
            use rusqlite::OptionalExtension;
            let current=tx.query_row("SELECT s.id,s.is_broken,a.sha256,a.byte_size,a.managed_path,a.original_filename,a.detected_format,a.validation_state FROM stories s JOIN story_archives a ON a.story_id=s.id WHERE s.uuid=?",[&original.uuid],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,bool>(1)?,r.get::<_,String>(2)?,r.get::<_,i64>(3)?,r.get::<_,String>(4)?,r.get::<_,String>(5)?,r.get::<_,String>(6)?,r.get::<_,String>(7)?))).optional()?;
            let Some((id,broken,hash,bytes,path,name,format,validation))=current else {return Ok(Disposition::Skipped(Issue::StoryUnavailable));};
            Ok(if id!=original.id {Disposition::Skipped(Issue::StoryUnavailable)}
            else if broken {Disposition::Skipped(Issue::StoryMarkedBroken)}
            else if hash!=original.sha256 || bytes!=original.byte_size || path!=original.managed_relative_path || name!=original.original_filename || format!=original.detected_format {Disposition::Skipped(Issue::ArchiveChanged)}
            else if validation!="valid" {Disposition::Skipped(if validation=="missing"{Issue::ArchiveMissing}else{Issue::ArchiveInvalid})}
            else{Disposition::Ready})
        })?.wait()
    }
}
pub(super) fn inspect(
    files: &LibraryFiles<'_>,
    s: &ExportStory,
    cancel: &WorkCancellation,
) -> Result<Option<Issue>> {
    if s.id == 0 {
        return Ok(Some(Issue::StoryUnavailable));
    }
    if s.broken {
        return Ok(Some(Issue::StoryMarkedBroken));
    }
    match s.verification.as_str() {
        "compatible" => (),
        "missing" => return Ok(Some(Issue::ArchiveMissing)),
        _ => return Ok(Some(Issue::ArchiveInvalid)),
    }
    if !supported_name(&s.original_filename, &s.detected_format) {
        return Ok(Some(Issue::UnsupportedExtension));
    }
    let mut source = match ArchiveSource::open(files, &s.managed_relative_path) {
        Ok(s) => s,
        Err(StorageError::PathEscape | StorageError::InvalidManagedPath) => {
            return Ok(Some(Issue::ArchiveMissing));
        }
        Err(StorageError::Io { source, .. })
            if matches!(
                source.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::InvalidInput
            ) =>
        {
            return Ok(Some(Issue::ArchiveMissing));
        }
        Err(e) => return Err(e),
    };
    if s.byte_size < 0 || source.bytes() != s.byte_size as u64 {
        return Ok(Some(Issue::ArchiveChanged));
    }
    let digest = hash_stream(&mut source, s.byte_size as u64, cancel)?;
    source.validate()?;
    Ok(
        (digest.checksum().to_string() != s.sha256 || digest.bytes() != s.byte_size as u64)
            .then_some(Issue::ArchiveChanged),
    )
}

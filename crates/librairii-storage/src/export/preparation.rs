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
}
#[derive(Debug)]
pub struct Exports<'db, 'library> {
    pub(super) database: &'db Database<'library>,
    pub(super) prepared: VecDeque<Prepared>,
    pub(super) retained: Vec<Output>,
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
            self.prepared.push_back(Prepared {
                id,
                scope,
                destination,
                preview: preview.clone(),
            });
        }
        Ok(preview)
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

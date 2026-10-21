//! Explicit export intent and observations. Neither grants filesystem authority.
use crate::{CollectionId, ShelfId, browsing::ExportStory, query::QueryRequest};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub enum Scope {
    CurrentQuery(QueryRequest),
    Selection(Vec<i64>),
    Collections(Vec<CollectionId>),
    Shelves(Vec<ShelfId>),
    ShelfResult { shelf: ShelfId, query: QueryRequest },
    ShelfSelection { shelf: ShelfId, uuids: Vec<String> },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Source {
    pub kind: crate::ExportScopeKind,
    pub ids: Vec<i64>,
    pub names: Vec<Vec<u8>>,
}

#[derive(Debug)]
pub struct Resolved {
    pub source: Source,
    pub stories: Vec<ExportStory>,
    pub collapsed_overlap: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ExportError {
    #[error("export scope is invalid")]
    InvalidScope,
    #[error("export preparation exceeds its byte budget")]
    ResourceLimit,
    #[error("stories changed after export preview; refresh the preview")]
    Stale,
    #[error("export destination is invalid or unavailable")]
    Destination,
    #[error("export has no ready stories")]
    NoReadyStories,
}
impl ExportError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidScope => "export.invalid_scope",
            Self::ResourceLimit => "export.resource_limit",
            Self::Stale => "export.prepared_stale",
            Self::Destination => "export.destination_invalid",
            Self::NoReadyStories => "export.no_exportable_stories",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Issue {
    StoryMarkedBroken,
    StoryUnavailable,
    EmptyScope,
    NoExportableStories,
    ArchiveMissing,
    ArchiveInvalid,
    ArchiveChanged,
    UnsupportedExtension,
    FilenameConflict,
}
impl Issue {
    pub const fn code(self) -> &'static str {
        match self {
            Self::StoryMarkedBroken => "story_marked_broken",
            Self::StoryUnavailable => "story_unavailable",
            Self::EmptyScope => "empty_scope",
            Self::NoExportableStories => "no_exportable_stories",
            Self::ArchiveMissing => "archive_missing",
            Self::ArchiveInvalid => "archive_invalid",
            Self::ArchiveChanged => "archive_changed",
            Self::UnsupportedExtension => "unsupported_extension",
            Self::FilenameConflict => "filename_conflict",
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Disposition {
    Ready,
    Skipped(Issue),
    Conflicted,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreviewItem {
    pub uuid: String,
    pub title: String,
    pub output_name: String,
    pub format: String,
    pub bytes: i64,
    pub disposition: Disposition,
}
/// Process-local unforgeable identity, owned by exactly one preparation registry.
/// Serializing a preview never produces a start/copy capability.
#[derive(Clone, Debug)]
pub struct PreparationId(Arc<()>);
impl PreparationId {
    pub fn fresh() -> Self {
        Self(Arc::new(()))
    }
    pub fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
#[derive(Clone, Debug)]
pub struct Preview {
    pub preparation: Option<PreparationId>,
    pub source: Source,
    pub destination_label: std::ffi::OsString,
    pub items: Arc<[PreviewItem]>,
    pub ready: usize,
    pub bytes: i64,
    pub formats: Vec<String>,
    pub collapsed_overlap: usize,
    pub issue: Option<Issue>,
}
impl Preview {
    pub fn can_export(&self) -> bool {
        self.ready > 0 && self.issue.is_none()
    }
}

/// Frozen passthrough suffix policy, applied without renaming original names.
pub fn supported_name(name: &str, format: &str) -> bool {
    if name.is_empty()
        || crate::collection_codec::tag_trim(name) != name
        || matches!(name, "." | "..")
        || name.contains(['/', '\\', '\0'])
    {
        return false;
    }
    let extension = match format {
        "plain_pk" => ".plain.pk",
        "v1_pk" => ".v1.pk",
        "v2_pk" => ".v2.pk",
        "generic_pk" => ".pk",
        "zip" | "studio_zip" => ".zip",
        "seven_zip" => ".7z",
        _ => return false,
    };
    name.to_ascii_lowercase().ends_with(extension)
}
pub fn filename_key(name: &str) -> Result<String, crate::collection_codec::CodecError> {
    crate::collection_codec::filename_lower(name)
}

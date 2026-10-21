//! Explicit export intent and observations. Neither grants filesystem authority.
use crate::{CollectionId, ShelfId, browsing::ExportStory, query::QueryRequest};

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

//! Dynamic saved criteria; raw stored text is preserved independently of display.
use crate::collection_codec::{self, Query};
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum CollectionError {
    #[error("collection.invalid_name")]
    InvalidName,
    #[error("collection.duplicate_name")]
    DuplicateName,
    #[error("collection.not_found")]
    NotFound,
    #[error("collection.invalid_order")]
    InvalidOrder,
    #[error("collection.needs_attention")]
    NeedsAttention,
    #[error("collection.criteria_unavailable")]
    CriteriaUnavailable,
    #[error("collection.in_use")]
    InUse,
    #[error("collection.invalid_selection")]
    InvalidSelection,
    #[error("collection.stale_preview")]
    StalePreview,
    #[error("collection.invalid_query")]
    InvalidQuery,
}
impl CollectionError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidName => "collection.invalid_name",
            Self::DuplicateName => "collection.duplicate_name",
            Self::NotFound => "collection.not_found",
            Self::InvalidOrder => "collection.invalid_order",
            Self::NeedsAttention => "collection.needs_attention",
            Self::CriteriaUnavailable => "collection.criteria_unavailable",
            Self::InUse => "collection.in_use",
            Self::InvalidSelection => "collection.invalid_selection",
            Self::StalePreview => "collection.stale_preview",
            Self::InvalidQuery => "collection.invalid_query",
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Validity {
    Valid,
    NeedsAttention,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Attention {
    MissingCriteria,
    UnmigratableQuery,
}
/// Lossless SQLite text bytes, including legacy spelling and invalid query text.
/// Presentation may interpret valid UTF-8; these bytes never grant authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Collection {
    pub id: i64,
    pub name: Vec<u8>,
    pub normalized_name: Vec<u8>,
    pub position: i64,
    pub query_version: i64,
    pub query_payload: Vec<u8>,
    pub validity: Validity,
    pub revision: i64,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Inspection {
    pub collection: Collection,
    pub query: Option<Query>,
    pub attention: Option<Attention>,
}
impl Inspection {
    pub fn is_openable(&self) -> bool {
        self.collection.validity == Validity::Valid && self.query.is_some()
    }
}
pub fn normalize_name(value: &str) -> Result<(String, String), CollectionError> {
    collection_codec::collection_name(value).map_err(|_| CollectionError::InvalidName)
}

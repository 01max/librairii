//! Optional metadata identities and errors. No network or SQLite authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum MetadataError {
    #[error("metadata.invalid_configuration")]
    InvalidConfiguration,
    #[error("metadata.network_failed")]
    Network,
    #[error("metadata.unexpected_redirect")]
    Redirect,
    #[error("metadata.unexpected_status")]
    Status,
    #[error("metadata.response_too_large")]
    TooLarge,
    #[error("metadata.invalid_json")]
    InvalidJson,
    #[error("metadata.invalid_guest_token")]
    InvalidToken,
    #[error("metadata.cancelled")]
    Cancelled,
    #[error("metadata.busy")]
    Busy,
    #[error("metadata.catalog_invalid")]
    InvalidCatalog,
    #[error("metadata.invalid_transition")]
    InvalidTransition,
    #[error("metadata.artwork_invalid")]
    InvalidArtwork,
    #[error("metadata.artwork_not_found")]
    ArtworkNotFound,
}
impl MetadataError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidConfiguration => "metadata.invalid_configuration",
            Self::Network => "metadata.network_failed",
            Self::Redirect => "metadata.unexpected_redirect",
            Self::Status => "metadata.unexpected_status",
            Self::TooLarge => "metadata.response_too_large",
            Self::InvalidJson => "metadata.invalid_json",
            Self::InvalidToken => "metadata.invalid_guest_token",
            Self::Cancelled => "metadata.cancelled",
            Self::Busy => "metadata.busy",
            Self::InvalidCatalog => "metadata.catalog_invalid",
            Self::InvalidTransition => "metadata.invalid_transition",
            Self::InvalidArtwork => "metadata.artwork_invalid",
            Self::ArtworkNotFound => "metadata.artwork_not_found",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct Locale(String);
impl Locale {
    pub fn parse(value: &str) -> Result<Self, MetadataError> {
        Ok(Self(crate::collection_codec::metadata_locale(value)?))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
pub fn title_key(value: &str) -> Result<String, MetadataError> {
    crate::collection_codec::metadata_text(value)
}
pub fn trim(value: &str) -> &str {
    crate::collection_codec::tag_trim(value)
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogRecord {
    pub uuid: crate::StoryUuid,
    pub title: String,
    pub description: String,
    pub author: String,
    pub publisher: String,
    pub language: Locale,
    pub duration_seconds: Option<i64>,
    pub minimum_age: Option<i64>,
    pub maximum_age: Option<i64>,
    pub artwork_id: Option<crate::Checksum>,
    pub source_record_id: String,
    pub source_updated_at: Option<String>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogArtwork {
    pub id: crate::Checksum,
    pub source_url: String,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Freshness {
    NeverSynced,
    Fresh,
    StaleCache,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogStatus {
    pub state: Freshness,
    pub locale: Locale,
    pub matched_stories: i64,
    pub fetched_at: Option<String>,
    pub activated_at: Option<String>,
    pub last_attempt_status: Option<SyncStatus>,
    pub last_attempt_at: Option<String>,
    pub error_code: Option<CatalogFailure>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyncStatus {
    Running,
    Succeeded,
    Failed,
    Cancelled,
}
impl SyncStatus {
    pub fn from_stored(value: &str) -> Result<Self, MetadataError> {
        match value {
            "running" => Ok(Self::Running),
            "succeeded" => Ok(Self::Succeeded),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err(MetadataError::InvalidTransition),
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CatalogFailure {
    FetchFailed,
    InvalidCatalog,
    StageFailed,
    PersistenceFailed,
    Cancelled,
    OutcomeUncertain,
    Interrupted,
    Unknown(String),
}
impl CatalogFailure {
    pub fn from_stored(value: String) -> Self {
        match value.as_str() {
            "catalog_fetch_failed" => Self::FetchFailed,
            "catalog_invalid" => Self::InvalidCatalog,
            "catalog_stage_failed" => Self::StageFailed,
            "catalog_persist_failed" => Self::PersistenceFailed,
            "catalog_cancelled" => Self::Cancelled,
            "catalog_outcome_uncertain" => Self::OutcomeUncertain,
            "catalog_interrupted" => Self::Interrupted,
            _ => Self::Unknown(value),
        }
    }
    pub fn as_str(&self) -> &str {
        match self {
            Self::FetchFailed => "catalog_fetch_failed",
            Self::InvalidCatalog => "catalog_invalid",
            Self::StageFailed => "catalog_stage_failed",
            Self::PersistenceFailed => "catalog_persist_failed",
            Self::Cancelled => "catalog_cancelled",
            Self::OutcomeUncertain => "catalog_outcome_uncertain",
            Self::Interrupted => "catalog_interrupted",
            Self::Unknown(value) => value,
        }
    }
}

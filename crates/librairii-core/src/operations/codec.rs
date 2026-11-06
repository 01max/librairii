use crate::{OperationKind, OperationState};

/// Invalid closed legacy discriminators. Original input belongs in the data
/// carrier, not in a diagnostic that could expose private payloads.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum OperationCodecError {
    #[error("unknown operation kind")]
    UnknownKind,
    #[error("unknown operation state")]
    UnknownState,
    #[error("unknown operation item state")]
    UnknownItemState,
    #[error("unknown export source type")]
    UnknownExportSourceType,
}

pub const fn encode_kind(value: OperationKind) -> &'static str {
    match value {
        OperationKind::Import => "import",
        OperationKind::MetadataSync => "metadata_sync",
        OperationKind::Export => "export",
    }
}

pub fn decode_kind(value: &str) -> Result<OperationKind, OperationCodecError> {
    match value {
        "import" => Ok(OperationKind::Import),
        "metadata_sync" => Ok(OperationKind::MetadataSync),
        "export" => Ok(OperationKind::Export),
        _ => Err(OperationCodecError::UnknownKind),
    }
}

pub const fn encode_state(value: OperationState) -> &'static str {
    match value {
        OperationState::Queued => "queued",
        OperationState::Running => "running",
        OperationState::Succeeded => "succeeded",
        OperationState::PartiallySucceeded => "partially_succeeded",
        OperationState::Failed => "failed",
        OperationState::Cancelled => "cancelled",
        OperationState::Interrupted => "interrupted",
    }
}

pub fn decode_state(value: &str) -> Result<OperationState, OperationCodecError> {
    match value {
        "queued" => Ok(OperationState::Queued),
        "running" => Ok(OperationState::Running),
        "succeeded" => Ok(OperationState::Succeeded),
        "partially_succeeded" => Ok(OperationState::PartiallySucceeded),
        "failed" => Ok(OperationState::Failed),
        "cancelled" => Ok(OperationState::Cancelled),
        "interrupted" => Ok(OperationState::Interrupted),
        _ => Err(OperationCodecError::UnknownState),
    }
}

pub const fn is_terminal(value: OperationState) -> bool {
    !matches!(value, OperationState::Queued | OperationState::Running)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ItemState {
    Pending,
    Running,
    Succeeded,
    Skipped,
    Conflicted,
    Failed,
    Cancelled,
}

impl ItemState {
    pub fn from_persisted(value: &str) -> Result<Self, OperationCodecError> {
        match value {
            "pending" => Ok(Self::Pending),
            "running" => Ok(Self::Running),
            "succeeded" => Ok(Self::Succeeded),
            "skipped" => Ok(Self::Skipped),
            "conflicted" => Ok(Self::Conflicted),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err(OperationCodecError::UnknownItemState),
        }
    }

    pub const fn as_persisted(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Skipped => "skipped",
            Self::Conflicted => "conflicted",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    pub const fn is_terminal(self) -> bool {
        !matches!(self, Self::Pending | Self::Running)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExportGroupKind {
    Collection,
    CuratedShelf,
}

/// Literal legacy discriminators remain distinct even when their meaning is
/// the same. In particular, `shelf` is a historical dynamic Collection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExportSourceType {
    Selection,
    CurrentQuery,
    Shelf,
    Shelves,
    Collection,
    Collections,
    CuratedShelf,
    CuratedShelves,
}

impl ExportSourceType {
    pub fn from_persisted(value: &str) -> Result<Self, OperationCodecError> {
        match value {
            "selection" => Ok(Self::Selection),
            "current_query" => Ok(Self::CurrentQuery),
            "shelf" => Ok(Self::Shelf),
            "shelves" => Ok(Self::Shelves),
            "collection" => Ok(Self::Collection),
            "collections" => Ok(Self::Collections),
            "curated_shelf" => Ok(Self::CuratedShelf),
            "curated_shelves" => Ok(Self::CuratedShelves),
            _ => Err(OperationCodecError::UnknownExportSourceType),
        }
    }

    pub const fn as_persisted(self) -> &'static str {
        match self {
            Self::Selection => "selection",
            Self::CurrentQuery => "current_query",
            Self::Shelf => "shelf",
            Self::Shelves => "shelves",
            Self::Collection => "collection",
            Self::Collections => "collections",
            Self::CuratedShelf => "curated_shelf",
            Self::CuratedShelves => "curated_shelves",
        }
    }

    pub const fn group_kind(self) -> Option<ExportGroupKind> {
        match self {
            Self::Selection | Self::CurrentQuery => None,
            Self::Shelf | Self::Shelves | Self::Collection | Self::Collections => {
                Some(ExportGroupKind::Collection)
            }
            Self::CuratedShelf | Self::CuratedShelves => Some(ExportGroupKind::CuratedShelf),
        }
    }
}

use super::{
    ExportGroupKind, ExportSourceType, OperationCodecError, OperationPolicyError, is_terminal,
};
use crate::{BoundaryError, JobId, OperationState, StoryUuid};
use std::collections::BTreeSet;

/// Raw storage spelling is authoritative for history. Go permits nonempty
/// operation IDs that are not UUIDs; a typed projection must never rewrite them.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredOperationIdentity {
    pub id: String,
    pub created_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
}

impl StoredOperationIdentity {
    pub fn typed_id(&self) -> Result<JobId, BoundaryError> {
        self.id.parse()
    }

    pub fn history_key(&self) -> HistoryKey {
        HistoryKey {
            created_at: self.created_at.clone(),
            id: self.id.clone(),
        }
    }
}

/// Ordering is the raw `(created_at, id)` tuple, not parsed time/UUID order.
/// Paging and authoritative database reads are not implemented here.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct HistoryKey {
    pub created_at: String,
    pub id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredItemIdentity {
    pub id: i64,
    pub story_id: Option<i64>,
    pub resolved_story_id: Option<i64>,
    pub story_uuid: Option<String>,
}

impl StoredItemIdentity {
    pub fn typed_story_uuid(&self) -> Result<Option<StoryUuid>, BoundaryError> {
        self.story_uuid.as_deref().map(str::parse).transpose()
    }

    /// Match `COALESCE(resolved_story_id, story_id, 0)` without relinking an
    /// historical item after its current story has been removed.
    pub fn historical_story_id(&self) -> i64 {
        self.resolved_story_id.or(self.story_id).unwrap_or_default()
    }
}

/// Codes/messages are open strings. NULL and an explicitly empty value remain
/// distinct. Only the aggregate policy applies its source-defined fallbacks.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RawOutcome {
    pub code: Option<String>,
    pub message: Option<String>,
}

/// Exact nullable legacy source columns, including original JSON text. This
/// carrier does not parse, normalize or regenerate JSON, or confer path authority.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StoredExportScope {
    pub discriminator: Option<String>,
    pub group_ids: Option<String>,
    pub group_names: Option<String>,
}

impl StoredExportScope {
    pub fn source_type(&self) -> Result<Option<ExportSourceType>, OperationCodecError> {
        self.discriminator
            .as_deref()
            .map(ExportSourceType::from_persisted)
            .transpose()
    }
}

/// Value-level source policy from Go ExportSource. Raw columns above remain
/// separate from this decoded command projection. No sorting or renaming occurs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportSource {
    pub source_type: ExportSourceType,
    pub collection_ids: Vec<i64>,
    pub collection_names: Vec<String>,
    pub shelf_ids: Vec<i64>,
    pub shelf_names: Vec<String>,
}

impl ExportSource {
    pub fn group_ids(&self) -> &[i64] {
        if matches!(
            self.source_type,
            ExportSourceType::Collection | ExportSourceType::Collections
        ) {
            &self.collection_ids
        } else {
            &self.shelf_ids
        }
    }

    pub fn group_names(&self) -> &[String] {
        if matches!(
            self.source_type,
            ExportSourceType::Collection | ExportSourceType::Collections
        ) {
            &self.collection_names
        } else {
            &self.shelf_names
        }
    }

    /// Go history also exposes Collection aliases for legacy shelf/shelves.
    /// This borrowed view leaves the literal discriminator and original vectors.
    pub fn collection_history_ids(&self) -> &[i64] {
        if self.source_type.group_kind() == Some(ExportGroupKind::Collection) {
            self.group_ids()
        } else {
            &[]
        }
    }

    pub fn collection_history_names(&self) -> &[String] {
        if self.source_type.group_kind() == Some(ExportGroupKind::Collection) {
            self.group_names()
        } else {
            &[]
        }
    }

    pub fn validate(&self) -> Result<(), OperationPolicyError> {
        let collection_fields = matches!(
            self.source_type,
            ExportSourceType::Collection | ExportSourceType::Collections
        );
        if (collection_fields && (!self.shelf_ids.is_empty() || !self.shelf_names.is_empty()))
            || (!collection_fields
                && (!self.collection_ids.is_empty() || !self.collection_names.is_empty()))
        {
            return Err(OperationPolicyError::InvalidExportSource);
        }
        let ids = self.group_ids();
        let names = self.group_names();
        let valid_count = match self.source_type {
            ExportSourceType::Selection | ExportSourceType::CurrentQuery => ids.is_empty(),
            ExportSourceType::Shelf
            | ExportSourceType::Collection
            | ExportSourceType::CuratedShelf => ids.len() == 1,
            ExportSourceType::Shelves
            | ExportSourceType::Collections
            | ExportSourceType::CuratedShelves => ids.len() >= 2,
        };
        let mut seen = BTreeSet::new();
        if !valid_count
            || ids.len() != names.len()
            || ids
                .iter()
                .zip(names)
                .any(|(id, name)| *id <= 0 || name.trim().is_empty() || !seen.insert(*id))
        {
            return Err(OperationPolicyError::InvalidExportSource);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObservationKind {
    StoredSnapshot,
    ProvisionalPersistenceFailure,
}

/// Describes the provenance supplied by the caller, not a durable receipt or a
/// storage capability. A terminal-shaped persistence fallback is not saved truth.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OperationObservation {
    pub state: OperationState,
    pub kind: ObservationKind,
}

impl OperationObservation {
    pub fn is_saved_terminal(self) -> bool {
        self.kind == ObservationKind::StoredSnapshot && is_terminal(self.state)
    }
}

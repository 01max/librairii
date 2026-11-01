use crate::{CollectionId, ShelfId};

/// Internal boundary messages; these are not persisted Go outcome codecs or catalogs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MessageId {
    InvalidUuid,
    InvalidGroupId,
    InvalidChecksum,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum BoundaryError {
    #[error("invalid UUID identity")]
    InvalidUuid,
    #[error("invalid positive group identity")]
    InvalidGroupId,
    #[error("invalid SHA-256 identity")]
    InvalidChecksum,
}
impl BoundaryError {
    pub const fn message_id(self) -> MessageId {
        match self {
            Self::InvalidUuid => MessageId::InvalidUuid,
            Self::InvalidGroupId => MessageId::InvalidGroupId,
            Self::InvalidChecksum => MessageId::InvalidChecksum,
        }
    }
}

/// Equal numeric group IDs retain separate meaning.
/// ```compile_fail
/// use librairii_core::{BrowseScope, CollectionId};
/// let collection = CollectionId::new(1).unwrap();
/// let wrong_scope = BrowseScope::Shelf(collection);
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BrowseScope {
    Library,
    Collection(CollectionId),
    Shelf(ShelfId),
}
/// Scope vocabulary, not export planning, serialized history or an eligibility policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExportScopeKind {
    CurrentQuery,
    Selection,
    CollectionUnion,
    ShelfUnion,
    RefinedShelf,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperationKind {
    Import,
    MetadataSync,
    Export,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperationState {
    Queued,
    Running,
    Succeeded,
    PartiallySucceeded,
    Failed,
    Cancelled,
    Interrupted,
}
/// A value cannot imply approval merely by being initialized.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ReviewDecision {
    #[default]
    NotNow,
    Add,
    Remove,
    Keep,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn group_scope_cannot_conflate_collection_with_curated_shelf() -> Result<(), BoundaryError> {
        let collection = BrowseScope::Collection(CollectionId::new(7)?);
        let shelf = BrowseScope::Shelf(ShelfId::new(7)?);
        assert_ne!(collection, shelf);
        assert_eq!(ReviewDecision::default(), ReviewDecision::NotNow);
        Ok(())
    }
    #[test]
    fn invalid_input_is_a_typed_message_without_the_input_payload() {
        let error = BoundaryError::InvalidChecksum;
        assert_eq!(error.message_id(), MessageId::InvalidChecksum);
        assert_eq!(error.to_string(), "invalid SHA-256 identity");
    }
}

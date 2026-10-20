//! Effect-free typed foundations, not persisted codecs or implemented workflows.
#![forbid(unsafe_code)]
mod identity;
mod vocabulary;
pub use identity::{Checksum, CollectionId, JobId, LibraryId, ShelfId, StoryUuid};
pub use vocabulary::{
    BoundaryError, BrowseScope, ExportScopeKind, MessageId, OperationKind, OperationState,
    ReviewDecision,
};

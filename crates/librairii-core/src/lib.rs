//! Effect-free typed foundations and frozen saved-query compatibility codecs.
#![forbid(unsafe_code)]
pub mod collection_codec;
mod identity;
mod vocabulary;
pub use identity::{Checksum, CollectionId, JobId, LibraryId, ShelfId, StoryUuid};
pub use vocabulary::{
    BoundaryError, BrowseScope, ExportScopeKind, MessageId, OperationKind, OperationState,
    ReviewDecision,
};

//! Effect-free typed foundations and frozen saved-query compatibility codecs.
#![forbid(unsafe_code)]
pub mod broken;
pub mod browsing;
pub mod collection_codec;
pub mod collections;
mod identity;
pub mod import;
pub mod metadata;
pub mod navigation;
pub mod operations;
pub mod query;
pub mod shelves;
pub mod tagging;
mod vocabulary;
pub use identity::{Checksum, CollectionId, JobId, LibraryId, ShelfId, StoryUuid};
pub use vocabulary::{
    BoundaryError, BrowseScope, ExportScopeKind, MessageId, OperationKind, OperationState,
    ReviewDecision,
};

//! Effect-free typed foundations and frozen saved-query compatibility codecs.
#![forbid(unsafe_code)]
pub mod broken;
pub mod browsing;
pub mod collection_codec;
mod identity;
pub mod import;
pub mod navigation;
pub mod operations;
pub mod query;
pub mod tagging;
mod vocabulary;
pub use identity::{Checksum, CollectionId, JobId, LibraryId, ShelfId, StoryUuid};
pub use vocabulary::{
    BoundaryError, BrowseScope, ExportScopeKind, MessageId, OperationKind, OperationState,
    ReviewDecision,
};

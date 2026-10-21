//! Compatible library persistence and owned filesystem effects.
//!
//! Root selection is explicit. The isolated override never discovers a profile.
pub mod adoption;
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub mod artwork;
pub mod backup;
pub mod broken;
pub mod collections;
pub mod database;
pub mod error;
pub mod identity;
pub mod layout;
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub mod metadata;
pub mod migrations;
pub mod native;
pub mod platform;
pub mod portability;
pub mod profile;
pub mod query;
pub mod shelves;
pub mod tagging;

pub use error::{Result, StorageError};
pub use identity::LibraryIdentity;
pub use layout::{DATA_ROOT_OVERRIDE, IsolatedLibrary, LibraryLayout, ManagedPath};

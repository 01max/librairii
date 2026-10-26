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
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub mod diagnostics;
pub mod error;
pub mod export;
pub mod identity;
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub mod imports;
pub mod layout;
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub mod metadata;
pub mod migrations;
pub mod native;
pub mod operations_repository;
pub mod platform;
pub mod portability;
pub mod profile;
pub mod query;
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub mod removal;
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub mod settings;
pub mod shelves;
pub mod tagging;

pub use error::{Result, StorageError};
pub use identity::LibraryIdentity;
pub use layout::{DATA_ROOT_OVERRIDE, IsolatedLibrary, LibraryLayout, ManagedPath};

#[cfg(any(target_os = "macos", target_os = "linux"))]
mod native_effects;

mod native_path;

#[cfg(all(test, any(target_os = "macos", target_os = "linux")))]
mod test_support;

//! Compatible library persistence and owned filesystem effects.
//!
//! Root selection is explicit. The isolated override never discovers a profile.
pub mod adoption;
pub mod backup;
pub mod database;
pub mod error;
pub mod identity;
pub mod layout;
pub mod migrations;
pub mod native;
pub mod operations_repository;
pub mod platform;
pub mod portability;
pub mod profile;

pub use error::{Result, StorageError};
pub use identity::LibraryIdentity;
pub use layout::{DATA_ROOT_OVERRIDE, IsolatedLibrary, LibraryLayout, ManagedPath};

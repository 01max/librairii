//! Fail-closed legacy handover policy, distinct from a cooperative kernel lock.
use crate::{IsolatedLibrary, Result, StorageError};
use std::path::Path;

/// No currently admitted bridge/exclusion mechanism can stop a later
/// unmodified Go launch through its retained root/profile/aliases. Therefore
/// this route refuses before any I/O, including lock-file creation or recovery.
/// A PID scan, user promise, Rust OS lease or bridge-installation flag cannot
/// manufacture the missing lifetime exclusion proof.
///
/// Production use requires the separately verified independent clone/profile/
/// recovery route. Its streaming hash/whole-manifest and rebound journal proof
/// must be implemented and qualified before any existing source is writable.
pub fn open_legacy_in_place(_root: &Path) -> Result<IsolatedLibrary> {
    Err(StorageError::LegacyLifetimeExclusionUnproven)
}

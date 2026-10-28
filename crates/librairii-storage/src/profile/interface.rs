//! Versioned library-bound native interface choices. Invalid/future documents
//! stay untouched; callers may keep session defaults without losing a library.
use super::NativeProfileStore;
use crate::{Result, StorageError};
use librairii_core::{LibraryId, navigation::MAX_STATE_BYTES};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InterfacePreferences {
    format: String,
    version: u64,
    library: String,
    pub sync_panel_hidden: bool,
    #[serde(default)]
    pub dismissed_operations: BTreeSet<String>,
}
impl InterfacePreferences {
    pub fn new(library: LibraryId) -> Self {
        Self {
            format: "librairii.native-interface".into(),
            version: 1,
            library: library.to_string(),
            sync_panel_hidden: false,
            dismissed_operations: BTreeSet::new(),
        }
    }
    pub fn library_id(&self) -> Result<LibraryId> {
        self.library
            .parse()
            .map_err(|_| StorageError::InvalidNavigation)
    }
    pub fn decode(bytes: &[u8], library: LibraryId) -> Result<Self> {
        let value: Self =
            serde_json::from_slice(bytes).map_err(|_| StorageError::InvalidNavigation)?;
        if bytes.len() > MAX_STATE_BYTES
            || value.format != "librairii.native-interface"
            || value.version != 1
            || value.library != library.to_string()
            || value.dismissed_operations.len() > 4096
            || value
                .dismissed_operations
                .iter()
                .any(|id| id.is_empty() || id.len() > 256)
        {
            return Err(StorageError::InvalidNavigation);
        }
        Ok(value)
    }
}
impl NativeProfileStore {
    pub fn load_interface(&self, library: LibraryId) -> Result<Option<InterfacePreferences>> {
        let _state = self.reserve()?;
        self.revalidate_namespace()?;
        let bytes = match self.ui_bytes("interface.json") {
            Ok(bytes) => bytes,
            Err(StorageError::Io { .. }) => {
                // Unreadable optional choices do not invalidate library authority.
                // A replaced namespace or non-regular leaf still fails closed.
                self.revalidate_namespace()?;
                self.namespace
                    .regular_entry(std::ffi::OsStr::new("interface.json"))?;
                return Err(StorageError::InvalidNavigation);
            }
            Err(error) => return Err(error),
        };
        bytes
            .map(|(bytes, _, _)| InterfacePreferences::decode(&bytes, library))
            .transpose()
    }
    pub fn save_interface(&self, value: &InterfacePreferences) -> Result<()> {
        let library = value
            .library
            .parse()
            .map_err(|_| StorageError::InvalidNavigation)?;
        let bytes = serde_json::to_vec(value).map_err(|_| StorageError::InvalidNavigation)?;
        InterfacePreferences::decode(&bytes, library)?;
        self.save_ui_document("interface.json", &bytes, |prior| {
            InterfacePreferences::decode(prior, library).map(|_| ())
        })
    }
}

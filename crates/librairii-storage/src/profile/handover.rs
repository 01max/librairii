//! The receipt and imported fallbacks are ONE atomic native preference file.
//! Existing native choices take precedence, and the receipt is never replayed.
use super::{NativeProfileStore, interface::InterfacePreferences};
use crate::{Result, StorageError};
use librairii_core::{LibraryId, handover, navigation::Navigation};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;
#[cfg(unix)]
use std::{fs::File, io::Read};

const NAME: &str = "handover.json";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HandoverImport {
    Imported,
    ImportedPreservingNativeChoices,
    AlreadyImported,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    format: String,
    version: u64,
    library: String,
    source_sha256: String,
    navigation: serde_json::Value,
    interface: InterfacePreferences,
    navigation_applied: bool,
    interface_applied: bool,
}

impl Receipt {
    fn decode(bytes: &[u8], library: LibraryId) -> Result<Self> {
        if bytes.len() > librairii_core::navigation::MAX_STATE_BYTES {
            return Err(StorageError::InvalidNavigation);
        }
        let value: Self =
            serde_json::from_slice(bytes).map_err(|_| StorageError::InvalidNavigation)?;
        if value.format != "librairii.native-handover"
            || value.version != 1
            || value.library != library.to_string()
            || value.source_sha256.len() != 64
            || !value.source_sha256.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(StorageError::InvalidNavigation);
        }
        Navigation::decode(
            &serde_json::to_vec(&value.navigation).map_err(|_| StorageError::InvalidNavigation)?,
            library,
            1,
        )
        .map_err(|_| StorageError::InvalidNavigation)?;
        InterfacePreferences::decode(
            &serde_json::to_vec(&value.interface).map_err(|_| StorageError::InvalidNavigation)?,
            library,
        )?;
        Ok(value)
    }
}

impl NativeProfileStore {
    #[cfg(unix)]
    pub fn import_handover_file(&self, path: &Path, library: LibraryId) -> Result<HandoverImport> {
        let file = File::from(
            rustix::fs::open(
                path,
                rustix::fs::OFlags::RDONLY
                    | rustix::fs::OFlags::NOFOLLOW
                    | rustix::fs::OFlags::NONBLOCK
                    | rustix::fs::OFlags::CLOEXEC,
                rustix::fs::Mode::empty(),
            )
            .map_err(|e| StorageError::io("open selected preference transfer", e.into()))?,
        );
        let before = file
            .metadata()
            .map_err(|e| StorageError::io("inspect preference transfer", e))?;
        if !before.is_file() || before.len() > librairii_core::navigation::MAX_STATE_BYTES as u64 {
            return Err(StorageError::InvalidNavigation);
        }
        let mut bytes = Vec::new();
        (&file)
            .take((librairii_core::navigation::MAX_STATE_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|e| StorageError::io("read bounded preference transfer", e))?;
        let after = file
            .metadata()
            .map_err(|e| StorageError::io("recheck preference transfer", e))?;
        if before.len() != after.len() || before.modified().ok() != after.modified().ok() {
            return Err(StorageError::InvalidNavigation);
        }
        self.import_handover(&bytes, library)
    }

    #[cfg(not(unix))]
    pub fn import_handover_file(
        &self,
        _path: &Path,
        _library: LibraryId,
    ) -> Result<HandoverImport> {
        Err(StorageError::InvalidNavigation)
    }

    fn handover_receipt(&self, library: LibraryId) -> Result<Option<Receipt>> {
        self.ui_bytes(NAME)?
            .map(|(bytes, _, _)| Receipt::decode(&bytes, library))
            .transpose()
    }

    pub(super) fn handover_navigation(
        &self,
        library: LibraryId,
        generation: u64,
    ) -> Result<Option<Navigation>> {
        let Some(receipt) = self.handover_receipt(library)? else {
            return Ok(None);
        };
        if !receipt.navigation_applied {
            return Ok(None);
        }
        Navigation::decode(
            &serde_json::to_vec(&receipt.navigation)
                .map_err(|_| StorageError::InvalidNavigation)?,
            library,
            generation,
        )
        .map(Some)
        .map_err(|_| StorageError::InvalidNavigation)
    }

    pub(super) fn handover_interface(
        &self,
        library: LibraryId,
    ) -> Result<Option<InterfacePreferences>> {
        Ok(self
            .handover_receipt(library)?
            .filter(|r| r.interface_applied)
            .map(|r| r.interface))
    }

    pub fn import_handover(&self, bytes: &[u8], library: LibraryId) -> Result<HandoverImport> {
        let imported =
            handover::decode(bytes, library, 1).map_err(|_| StorageError::InvalidNavigation)?;
        let mut state = self.reserve()?;
        self.load_locked(&mut state)?;
        self.revalidate_settings(&state)?;
        let profile = state
            .document
            .as_ref()
            .ok_or(StorageError::InvalidNavigation)?
            .profile();
        if !profile.complete || profile.library_id.parse::<LibraryId>().ok() != Some(library) {
            return Err(StorageError::InvalidNavigation);
        }
        if self.handover_receipt(library)?.is_some() {
            return Ok(HandoverImport::AlreadyImported);
        }
        let prior_navigation = self.ui_bytes("navigation.json")?;
        if let Some((bytes, _, _)) = &prior_navigation {
            Navigation::decode(bytes, library, 1).map_err(|_| StorageError::InvalidNavigation)?;
        }
        let prior_interface = self.ui_bytes("interface.json")?;
        if let Some((bytes, _, _)) = &prior_interface {
            InterfacePreferences::decode(bytes, library)?;
        }
        let mut interface = InterfacePreferences::new(library);
        interface.dismissed_operations = imported.dismissed_operations;
        interface.sync_panel_hidden = imported.sync_panel_hidden;
        let receipt = Receipt {
            format: "librairii.native-handover".into(),
            version: 1,
            library: library.to_string(),
            source_sha256: librairii_core::Checksum::from_bytes(Sha256::digest(bytes).into())
                .to_string(),
            navigation: serde_json::from_slice(
                &imported
                    .navigation
                    .encode()
                    .map_err(|_| StorageError::InvalidNavigation)?,
            )
            .map_err(|_| StorageError::InvalidNavigation)?,
            interface,
            navigation_applied: prior_navigation.is_none(),
            interface_applied: prior_interface.is_none(),
        };
        let preserving = prior_navigation.is_some() || prior_interface.is_some();
        let bytes = serde_json::to_vec(&receipt).map_err(|_| StorageError::InvalidNavigation)?;
        Receipt::decode(&bytes, library)?;
        self.save_ui_document_locked(&mut state, NAME, &bytes, |prior| {
            Receipt::decode(prior, library).map(|_| ())
        })?;
        Ok(if preserving {
            HandoverImport::ImportedPreservingNativeChoices
        } else {
            HandoverImport::Imported
        })
    }
}

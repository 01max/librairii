//! Independent native UI state in the already leased profile namespace.
use super::{NativeProfileStore, State, require_same};
use crate::{Result, StorageError, platform::unix::durable_file_flush};
use librairii_core::{
    LibraryId,
    navigation::{MAX_STATE_BYTES, Navigation},
};
use std::{
    ffi::OsStr,
    fs::{File, Metadata},
    io::{Read, Write},
};
const NAME: &str = "navigation.json";

impl NativeProfileStore {
    pub(super) fn ui_bytes(&self, name: &'static str) -> Result<Option<(Vec<u8>, File, Metadata)>> {
        let file = match self
            .namespace
            .open_regular(OsStr::new(name), rustix::fs::OFlags::RDONLY)
        {
            Ok(file) => file,
            Err(StorageError::Io { source, .. })
                if source.kind() == std::io::ErrorKind::NotFound =>
            {
                return Ok(None);
            }
            Err(error) => return Err(error),
        };
        let metadata = file
            .metadata()
            .map_err(|e| StorageError::io("inspect navigation preferences", e))?;
        let mut bytes = Vec::new();
        (&file)
            .take((MAX_STATE_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|e| StorageError::io("read navigation preferences", e))?;
        if bytes.len() > MAX_STATE_BYTES {
            return Err(StorageError::InvalidNavigation);
        }
        self.revalidate_namespace()?;
        Ok(Some((bytes, file, metadata)))
    }

    /// Unknown/corrupt or differently bound preferences remain untouched.
    /// The caller can present a reset/transfer choice; no library is replaced.
    pub fn load_navigation(
        &self,
        library: LibraryId,
        generation: u64,
    ) -> Result<Option<Navigation>> {
        let _state = self.reserve()?;
        self.revalidate_namespace()?;
        let saved = self.ui_bytes(NAME)?;
        if saved.is_none() {
            return self.handover_navigation(library, generation);
        }
        saved
            .map(|(bytes, _, _)| {
                Navigation::decode(&bytes, library, generation)
                    .map_err(|_| StorageError::InvalidNavigation)
            })
            .transpose()
    }

    pub fn save_navigation(&self, navigation: &Navigation) -> Result<()> {
        let bytes = navigation
            .encode()
            .map_err(|_| StorageError::InvalidNavigation)?;
        self.save_ui_document(NAME, &bytes, |prior| {
            Navigation::decode(prior, navigation.library(), 1)
                .map(|_| ())
                .map_err(|_| StorageError::InvalidNavigation)
        })
    }

    pub fn load_shelf_warnings(
        &self,
        library: LibraryId,
    ) -> Result<Option<librairii_core::shelves::WarningAcknowledgements>> {
        let _state = self.reserve()?;
        self.revalidate_namespace()?;
        self.ui_bytes("shelf-warnings.json")?
            .map(|(bytes, _, _)| {
                Ok(librairii_core::shelves::WarningAcknowledgements::decode(
                    &bytes, library,
                )?)
            })
            .transpose()
    }
    pub fn save_shelf_warnings(
        &self,
        warnings: &librairii_core::shelves::WarningAcknowledgements,
    ) -> Result<()> {
        let bytes = warnings.encode()?;
        self.save_ui_document("shelf-warnings.json", &bytes, |prior| {
            librairii_core::shelves::WarningAcknowledgements::decode(prior, warnings.library())
                .map(|_| ())
                .map_err(Into::into)
        })
    }
    pub(super) fn save_ui_document(
        &self,
        name: &'static str,
        bytes: &[u8],
        validate: impl Fn(&[u8]) -> Result<()>,
    ) -> Result<()> {
        let mut state = self.reserve()?;
        self.save_ui_document_locked(&mut state, name, bytes, validate)
    }

    pub(super) fn save_ui_document_locked(
        &self,
        state: &mut State,
        name: &'static str,
        bytes: &[u8],
        validate: impl Fn(&[u8]) -> Result<()>,
    ) -> Result<()> {
        self.revalidate_namespace()?;
        let previous = self.ui_bytes(name)?;
        // Preserve invalid/future documents rather than overwriting them.
        if let Some((prior, _, _)) = &previous {
            validate(prior)?;
        }
        let mut staged = match self.namespace.create_config_temp() {
            Ok(stage) => stage,
            Err(failure) => {
                let failure = *failure;
                if !failure.creation_refused || failure.allocation.file.is_some() {
                    state.poison();
                    state.recovery_allocations.push(failure.allocation);
                }
                return Err(failure.cause);
            }
        };
        let before = (|| {
            staged
                .file
                .write_all(bytes)
                .map_err(|e| StorageError::io("write navigation preferences", e))?;
            durable_file_flush(&staged.file)?;
            self.revalidate_namespace()?;
            match (&previous, self.ui_bytes(name)?) {
                (None, None) => Ok(()),
                (Some((prior, _retained, metadata)), Some((actual, _current_file, current)))
                    if prior == &actual =>
                {
                    require_same(metadata, &current)
                }
                _ => Err(StorageError::ProfileChanged),
            }
        })();
        if let Err(work) = before {
            return match self.namespace.remove_config_temp(&staged) {
                Ok(()) => Err(work),
                Err(cleanup) => {
                    state.poison();
                    state.recovery_witnesses.push(staged);
                    Err(StorageError::OwnedTempCleanupFailed {
                        work: Box::new(work),
                        cleanup: Box::new(cleanup),
                    })
                }
            };
        }
        let (published, old) = match self.namespace.replace_owned_config(
            staged,
            OsStr::new(name),
            previous.as_ref().map(|(_, _, m)| m),
        ) {
            Ok(receipt) => receipt,
            Err(failure) => {
                let failure = *failure;
                state.poison();
                state.recovery_witnesses.push(failure.temporary);
                return Err(StorageError::ProfilePublicationUncertain {
                    cause: Box::new(failure.cause),
                });
            }
        };
        let after = (|| {
            if let Some(old) = &old {
                let mut prior = Vec::new();
                old.file
                    .try_clone()
                    .map_err(|e| StorageError::io("retain old navigation preferences", e))?
                    .take((MAX_STATE_BYTES + 1) as u64)
                    .read_to_end(&mut prior)
                    .map_err(|e| StorageError::io("read old navigation preferences", e))?;
                if previous.as_ref().map(|(bytes, _, _)| bytes.as_slice()) != Some(prior.as_slice())
                {
                    return Err(StorageError::ProfileChanged);
                }
            }
            self.namespace.flush()?;
            self.revalidate_namespace()?;
            let (actual, _file, metadata) =
                self.ui_bytes(name)?.ok_or(StorageError::ProfileChanged)?;
            if actual != bytes {
                return Err(StorageError::ProfileChanged);
            }
            require_same(
                &published
                    .file
                    .metadata()
                    .map_err(|e| StorageError::io("inspect navigation publication", e))?,
                &metadata,
            )?;
            if let Some(old) = &old {
                self.namespace.remove_config_temp(old)?;
                self.namespace.flush()?;
            }
            Ok(())
        })();
        if let Err(cause) = after {
            retain(state, published, old);
            return Err(StorageError::ProfilePublicationUncertain {
                cause: Box::new(cause),
            });
        }
        Ok(())
    }
}
fn retain(
    state: &mut State,
    published: crate::platform::unix::ConfigTemp,
    old: Option<crate::platform::unix::ConfigTemp>,
) {
    state.poison();
    state.recovery_witnesses.push(published);
    if let Some(old) = old {
        state.recovery_witnesses.push(old);
    }
}

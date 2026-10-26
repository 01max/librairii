//! Native relocation journal custody. A journal admits only its own namespace
//! entry; it grants no authority to any root mentioned in its bytes.
use super::*;
use crate::platform::unix::ConfigTemp;
use std::{
    ffi::OsStr,
    io::{Read, Seek},
};
pub(crate) const JOURNAL: &str = "native-relocation-v1.json";
pub(crate) const MAX_JOURNAL: usize = crate::settings::manifest::MAX_MANIFEST_BYTES;
#[derive(Debug)]
pub(super) struct JournalWitness {
    file: ConfigTemp,
    bytes: Vec<u8>,
    initialized: bool,
}
impl NativeProfileStore {
    pub(crate) fn read_move_journal(&self) -> Result<Option<Vec<u8>>> {
        self.revalidate_namespace()?;
        let Some(mut file) = self.namespace.read_regular(OsStr::new(JOURNAL))? else {
            return Ok(None);
        };
        let mut bytes = Vec::new();
        (&mut file)
            .take((MAX_JOURNAL + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|e| StorageError::io("read native move journal", e))?;
        if bytes.len() > MAX_JOURNAL {
            return Err(StorageError::RecoveryBindingRequired);
        }
        Ok(Some(bytes))
    }
    pub(crate) fn create_move_journal(&self, bytes: &[u8]) -> Result<()> {
        self.require_unrecovered_namespace()?;
        if bytes.len() > MAX_JOURNAL.saturating_sub(80) {
            return Err(StorageError::HashSizeLimit);
        }
        let mut guard = self
            .move_journal
            .lock()
            .map_err(|_| StorageError::ProfileRestartRequired)?;
        if guard.is_none() {
            let file = match self.namespace.create_control_record(OsStr::new(JOURNAL)) {
                Ok(file) => file,
                Err(failure) => {
                    let failure = *failure;
                    if failure.allocation.file.is_some() {
                        if let Ok(file) = self.namespace.recover_control(&failure.allocation) {
                            *guard = Some(JournalWitness {
                                file,
                                bytes: bytes.to_vec(),
                                initialized: false,
                            });
                        } else {
                            let mut state = self.reserve_inner(true)?;
                            state.recovery_allocations.push(failure.allocation);
                            state.poison();
                        }
                    }
                    return Err(failure.cause);
                }
            };
            // Retain the actual file before the first write/flush can fail.
            *guard = Some(JournalWitness {
                file,
                bytes: bytes.to_vec(),
                initialized: false,
            });
        }
        let witness = guard.as_mut().ok_or(StorageError::ProfileRestartRequired)?;
        if witness.bytes != bytes {
            return Err(StorageError::RecoveryBindingRequired);
        }
        witness
            .file
            .file
            .rewind()
            .map_err(|e| StorageError::io("rewind own move intent", e))?;
        witness
            .file
            .file
            .write_all(bytes)
            .map_err(|e| StorageError::io("write native move intent", e))?;
        durable_file_flush(&witness.file.file)?;
        self.namespace.flush()?;
        witness.initialized = true;
        drop(guard);
        self.revalidate_move_journal()
    }
    // A small append-only acknowledgement follows the immutable JSON intent.
    // A torn footer cannot authorize control retirement. Controls are removed
    // only after this complete footer and its directory have durably flushed.
    pub(crate) fn acknowledge_move_retirement(&self, intent: &[u8]) -> Result<()> {
        self.revalidate_move_journal()?;
        let footer = format!("\nretired:{}\n", crate::native::digest(intent));
        let mut guard = self
            .move_journal
            .lock()
            .map_err(|_| StorageError::ProfileRestartRequired)?;
        let witness = guard
            .as_mut()
            .ok_or(StorageError::RecoveryBindingRequired)?;
        if !witness.bytes.starts_with(intent)
            || !footer
                .as_bytes()
                .starts_with(&witness.bytes[intent.len()..])
        {
            return Err(StorageError::RecoveryBindingRequired);
        }
        let mut expected = intent.to_vec();
        expected.extend_from_slice(footer.as_bytes());
        witness.bytes = expected;
        witness.initialized = false;
        witness
            .file
            .file
            .seek(std::io::SeekFrom::Start(intent.len() as u64))
            .map_err(|e| StorageError::io("seek move retirement receipt", e))?;
        witness
            .file
            .file
            .write_all(footer.as_bytes())
            .map_err(|e| StorageError::io("write move retirement receipt", e))?;
        durable_file_flush(&witness.file.file)?;
        self.namespace.flush()?;
        witness.initialized = true;
        drop(guard);
        self.revalidate_move_journal()
    }
    // Only the native recovery verifier calls this AFTER root/profile/marker
    // proof. Neither opening a store nor parsing a journal invokes it.
    pub(crate) fn admit_move_journal(&self, expected: &[u8]) -> Result<()> {
        if self.read_move_journal()?.as_deref() != Some(expected) {
            return Err(StorageError::RecoveryBindingRequired);
        }
        let file = self.namespace.read_config(OsStr::new(JOURNAL))?;
        *self
            .move_journal
            .lock()
            .map_err(|_| StorageError::ProfileRestartRequired)? = Some(JournalWitness {
            file,
            bytes: expected.to_vec(),
            initialized: true,
        });
        self.revalidate_move_journal()
    }
    pub(super) fn revalidate_move_journal(&self) -> Result<()> {
        let mut guard = self
            .move_journal
            .lock()
            .map_err(|_| StorageError::ProfileRestartRequired)?;
        let witness = guard
            .as_mut()
            .ok_or(StorageError::RecoveryBindingRequired)?;
        let named = self
            .namespace
            .open_regular(OsStr::new(JOURNAL), rustix::fs::OFlags::RDONLY)?;
        if !crate::settings::manifest::same_file(&named, &witness.file.file)? {
            return Err(StorageError::RecoveryBindingRequired);
        }
        witness
            .file
            .file
            .rewind()
            .map_err(|e| StorageError::io("rewind retained move intent", e))?;
        let mut bytes = Vec::new();
        (&mut witness.file.file)
            .take((MAX_JOURNAL + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|e| StorageError::io("check retained move intent", e))?;
        if if witness.initialized {
            bytes != witness.bytes
        } else {
            !witness.bytes.starts_with(&bytes)
        } {
            return Err(StorageError::RecoveryBindingRequired);
        }
        Ok(())
    }
    pub(crate) fn retire_move_journal(&self) -> Result<()> {
        self.revalidate_move_journal()?;
        let mut guard = self
            .move_journal
            .lock()
            .map_err(|_| StorageError::ProfileRestartRequired)?;
        let witness = guard
            .as_ref()
            .ok_or(StorageError::RecoveryBindingRequired)?;
        self.namespace.remove_config_temp(&witness.file)?;
        *guard = None;
        self.namespace.flush()?;
        Ok(())
    }
}
impl<'profile> ProfileBinding<'profile> {
    pub(crate) fn store(&self) -> &'profile NativeProfileStore {
        self.store
    }
    pub(crate) fn retarget(
        &mut self,
        verified: &crate::native::VerifiedMoveTarget,
    ) -> Result<ProfileBinding<'profile>> {
        self.revalidate()?;
        let mut state = self.store.reserve_inner(true)?;
        let mut next = state
            .document
            .as_ref()
            .ok_or(StorageError::ProfileChanged)?
            .profile()
            .clone();
        if !self.state.matches(&next) || next.library_id != verified.raw_id() {
            return Err(StorageError::ProfileChanged);
        }
        next.root = verified.root().into();
        let bytes = encode(&next)?;
        self.store
            .update_unix(&mut state, next.clone(), bytes, |_| Ok(()), |_| Ok(()))?;
        let binding = Arc::new(BindingState {
            root: next.root,
            raw_id: next.library_id,
            fenced: Arc::new(AtomicBool::new(false)),
            frozen: AtomicBool::new(false),
        });
        state.binding = Some(binding.clone());
        self.released = true;
        Ok(ProfileBinding {
            store: self.store,
            state: binding,
            released: false,
        })
    }
}

impl NativeProfileStore {
    pub(crate) fn commit_located<'profile>(
        &'profile self,
        verified: &crate::native::VerifiedMoveTarget,
        previous: &Profile,
    ) -> Result<ProfileBinding<'profile>> {
        self.require_unrecovered_namespace()?;
        let mut state = self.reserve()?;
        self.load_locked(&mut state)?;
        self.revalidate_settings(&state)?;
        if state.binding.is_some() {
            return Err(StorageError::ProfileBindingActive);
        }
        let mut next = state
            .document
            .as_ref()
            .ok_or(StorageError::ProfileChanged)?
            .profile()
            .clone();
        if next.root != previous.root
            || next.library_id != previous.library_id
            || next.library_id != verified.raw_id()
            || !next.complete
            || next.recovering
        {
            return Err(StorageError::ProfileChanged);
        }
        next.root = verified.root().into();
        let bytes = encode(&next)?;
        self.update_unix(&mut state, next.clone(), bytes, |_| Ok(()), |_| Ok(()))?;
        let binding = Arc::new(BindingState {
            root: next.root,
            raw_id: next.library_id,
            fenced: Arc::new(AtomicBool::new(false)),
            frozen: AtomicBool::new(false),
        });
        state.binding = Some(binding.clone());
        Ok(ProfileBinding {
            store: self,
            state: binding,
            released: false,
        })
    }
}

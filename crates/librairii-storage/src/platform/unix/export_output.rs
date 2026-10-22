//! Publication from the actual native registered temporary. Final names never
//! enter the restart cleanup registry and are never unlinked by retirement.
use super::*;
use sha2::{Digest, Sha256};
impl OwnedExportTemporary<'_, '_> {
    pub(crate) fn copy_verified(
        &mut self,
        source: &mut transfer::ArchiveSource<'_, '_>,
        expected: librairii_core::Checksum,
        bytes: u64,
        cancelled: impl Fn() -> bool,
        progress: impl FnMut(u64),
    ) -> Result<()> {
        copy_bytes(source, self, expected, bytes, &cancelled, progress)?;
        loop {
            if cancelled() {
                return Err(StorageError::Cancelled);
            }
            match source.validate() {
                Ok(()) => break,
                Err(StorageError::ProfileBusy | StorageError::QueueFull) => {
                    std::thread::sleep(std::time::Duration::from_millis(10))
                }
                Err(e) => return Err(e),
            }
        }
        self.flush_durable()?;
        let file = self
            .allocation
            .file
            .as_mut()
            .ok_or(StorageError::CompatibilityInvariant)?;
        file.rewind()
            .map_err(|e| StorageError::io("rewind registered export", e))?;
        let cancel = crate::database::WorkCancellation::default();
        let checked = hash_stream(file, bytes, &cancel)?;
        if checked.bytes() != bytes || checked.checksum() != expected {
            return Err(StorageError::SnapshotChanged);
        }
        self.revalidate()
    }
    /// Returns only after both rename directory entries are durable. On a lost
    /// acknowledgement, retain this owner and retry this SAME name uncancelled.
    pub fn publish_registered(
        &mut self,
        guard: &Custody<'_>,
        name: impl AsRef<OsStr>,
    ) -> Result<()> {
        self.allocation.files.check(guard)?;
        let name = name.as_ref();
        if !librairii_core::operations::valid_output_name(
            &librairii_app::ports::operations::display_native_text(name.as_bytes()),
        ) {
            return Err(StorageError::InvalidManagedPath);
        }
        let record = self
            .allocation
            .record
            .as_ref()
            .ok_or(StorageError::CompatibilityInvariant)?;
        let FilesAuthority::Native { database, .. } = self.allocation.files.authority else {
            return Err(StorageError::IdentityMismatch);
        };
        let operation = record.operation.clone();
        let item = record.item;
        let actual = database
            .barrier_read(move |tx| {
                Ok(tx.query_row(
                    "SELECT output_name FROM file_operation_items WHERE operation_id=? AND id=?",
                    rusqlite::params![operation, item],
                    |r| Ok(r.get_ref(0)?.as_bytes()?.to_vec()),
                )?)
            })?
            .wait()?;
        if actual != name.as_bytes() {
            return Err(StorageError::SnapshotChanged);
        }
        let final_name = EntryName::parse(name)?;
        if self
            .allocation
            .final_name
            .as_ref()
            .is_some_and(|n| n != final_name.as_os_str())
        {
            return Err(StorageError::SnapshotChanged);
        }
        let parent = self
            .allocation
            .parent
            .as_ref()
            .ok_or(StorageError::CompatibilityInvariant)?;
        let directory = self
            .allocation
            .directory
            .as_ref()
            .ok_or(StorageError::CompatibilityInvariant)?;
        let identity = record.payload.ok_or(StorageError::CompatibilityInvariant)?;
        if self.allocation.uncertain {
            // Reconcile against the actual retained file, never a saved DTO.
            if parent
                .require_name(final_name.as_os_str(), identity)
                .is_ok()
                && matches!(
                    native::statat(&directory.file, PAYLOAD, AtFlags::SYMLINK_NOFOLLOW),
                    Err(rustix::io::Errno::NOENT)
                )
            {
                self.allocation.final_name = Some(final_name.as_os_str().to_owned());
                self.allocation.uncertain = false;
            } else if directory
                .require_name(OsStr::new(PAYLOAD), identity)
                .is_ok()
            {
                self.allocation.uncertain = false;
            } else {
                return Err(StorageError::CustodyRecoveryRequired);
            }
        }
        if self.allocation.final_name.is_none() {
            self.revalidate()?;
            self.allocation.uncertain = true;
            match native::renameat_with(
                &directory.file,
                PAYLOAD,
                &parent.file,
                final_name.as_os_str(),
                RenameFlags::NOREPLACE,
            ) {
                Ok(()) => {
                    self.allocation.final_name = Some(final_name.as_os_str().to_owned());
                    self.allocation.uncertain = false;
                }
                Err(e) if e == rustix::io::Errno::EXIST => {
                    self.allocation.uncertain = false;
                    return Err(StorageError::DestinationCollision);
                }
                Err(e) => return Err(StorageError::io("publish registered export", e.into())),
            }
        }
        parent.require_name(final_name.as_os_str(), identity)?;
        directory.flush()?;
        parent.flush()
    }
    pub(crate) fn publication_pending(&self) -> bool {
        self.allocation.uncertain || self.allocation.final_name.is_some()
    }
    pub(crate) fn retire_in_place(&mut self, guard: &Custody<'_>) -> Result<()> {
        self.allocation.retire_registered(guard)
    }
}
impl ExportAllocation<'_, '_> {
    fn retire_registered(&mut self, guard: &Custody<'_>) -> Result<()> {
        if self.record.is_none() && self.directory.is_none() && self.file.is_none() {
            return Ok(());
        }
        if self.uncertain {
            return Err(StorageError::CustodyRecoveryRequired);
        }
        let FilesAuthority::Native { library, .. } = self.files.authority else {
            return Err(StorageError::IdentityMismatch);
        };
        let prior = self
            .record
            .as_ref()
            .ok_or(StorageError::CustodyRecoveryRequired)?
            .clone();
        let mut current = prior.clone();
        if let Some(parent) = &self.parent {
            if parent.identity() != current.parent {
                return Err(StorageError::SnapshotChanged);
            }
            if let Some(directory) = &self.directory {
                if !self.workspace_retired {
                    parent.require_name(&self.name, directory.identity())?;
                } else {
                    require_absent(parent, &self.name)?;
                }
                current.directory = Some(directory.identity());
                if let Some(file) = &self.file {
                    let identity = FileIdentity::from_file(file)?;
                    if self.final_name.is_none() {
                        require_owned_or_unlinked(file, directory, OsStr::new(PAYLOAD), identity)?;
                    } else {
                        require_regular(file)?;
                    }
                    current.payload = Some(identity);
                }
            }
        }
        library.reconcile_export_registration(self, &current)?;
        if current != prior {
            library.register_temporary(current.clone())?;
            self.record = Some(current.clone());
        }
        if !self.files.authority.same_generation(guard.authority) {
            return Err(StorageError::IdentityMismatch);
        }
        library.revalidate_database_ownership()?;
        if let (Some(parent), Some(directory)) = (&self.parent, &self.directory) {
            if !self.workspace_retired {
                parent.require_name(&self.name, directory.identity())?;
                for name in directory.entries(1, &WorkCancellation::default())? {
                    if name != OsStr::new(PAYLOAD) {
                        return Err(StorageError::CustodyRecoveryRequired);
                    }
                }
                if let Some(file) = &self.file {
                    let identity = FileIdentity::from_file(file)?;
                    if self.final_name.is_none()
                        && file
                            .metadata()
                            .map_err(|e| StorageError::io("observe retiring payload", e))?
                            .nlink()
                            != 0
                    {
                        require_regular(file)?;
                        directory.require_name(OsStr::new(PAYLOAD), identity)?;
                        native::unlinkat(&directory.file, PAYLOAD, AtFlags::empty()).map_err(
                            |e| StorageError::io("retire actual export payload", e.into()),
                        )?;
                    } else {
                        require_absent(directory, OsStr::new(PAYLOAD))?;
                    }
                }
                directory.flush()?;
                parent.require_name(&self.name, directory.identity())?;
                native::unlinkat(&parent.file, &self.name, AtFlags::REMOVEDIR)
                    .map_err(|e| StorageError::io("retire actual export workspace", e.into()))?;
                self.workspace_retired = true;
            } else {
                require_absent(parent, &self.name)?;
            }
            parent.flush()?;
        } else {
            self.files.compensate_export_allocation(guard, self)?;
        }
        library.forget_temporary(&current)
    }
}
impl ExportTemporaryFailure<'_, '_> {
    pub(crate) fn retire_in_place(&mut self, guard: &Custody<'_>) -> Result<()> {
        self.allocation.retire_registered(guard)
    }
}

// A retained unlinked descriptor proves retirement of that acquired object.
// A replacement at its former name must still refuse; it is never adopted.
fn require_owned_or_unlinked(
    file: &File,
    parent: &Directory,
    name: &OsStr,
    identity: FileIdentity,
) -> Result<()> {
    if FileIdentity::from_file(file)? != identity {
        return Err(StorageError::SnapshotChanged);
    }
    let metadata = file
        .metadata()
        .map_err(|e| StorageError::io("observe retiring export owner", e))?;
    if metadata.nlink() == 0 {
        return require_absent(parent, name);
    }
    if metadata.is_file() {
        require_regular(file)?;
    }
    parent.require_name(name, identity)
}
fn require_absent(parent: &Directory, name: &OsStr) -> Result<()> {
    match native::statat(&parent.file, name, AtFlags::SYMLINK_NOFOLLOW) {
        Err(rustix::io::Errno::NOENT) => Ok(()),
        Ok(_) => Err(StorageError::SnapshotChanged),
        Err(e) => Err(StorageError::io("observe retired export name", e.into())),
    }
}

fn copy_bytes(
    source: &mut impl Read,
    target: &mut impl Write,
    expected: librairii_core::Checksum,
    bytes: u64,
    cancelled: impl Fn() -> bool,
    mut progress: impl FnMut(u64),
) -> Result<()> {
    let mut digest = Sha256::new();
    let mut count = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        if cancelled() {
            return Err(StorageError::Cancelled);
        }
        let n = match source.read(&mut buffer) {
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e)
                if e.get_ref()
                    .and_then(|e| e.downcast_ref::<StorageError>())
                    .is_some_and(|e| {
                        matches!(e, StorageError::ProfileBusy | StorageError::QueueFull)
                    }) =>
            {
                std::thread::sleep(std::time::Duration::from_millis(10));
                continue;
            }
            Err(e) => return Err(StorageError::io("read export source", e)),
        };
        if n == 0 {
            break;
        }
        count = count
            .checked_add(n as u64)
            .ok_or(StorageError::HashSizeLimit)?;
        if count > bytes {
            return Err(StorageError::SnapshotChanged);
        }
        target
            .write_all(&buffer[..n])
            .map_err(|e| StorageError::io("write registered export", e))?;
        digest.update(&buffer[..n]);
        progress(count);
    }
    if count != bytes || librairii_core::Checksum::from_bytes(digest.finalize().into()) != expected
    {
        return Err(StorageError::SnapshotChanged);
    }
    Ok(())
}
impl OwnedExportTemporary<'_, '_> {
    pub(crate) fn matches_destination(&self, selected: &transfer::Destination) -> Result<()> {
        selected.revalidate()?;
        if self
            .allocation
            .parent
            .as_ref()
            .ok_or(StorageError::CompatibilityInvariant)?
            .identity()
            != selected.directory.identity()
        {
            return Err(StorageError::SnapshotChanged);
        }
        Ok(())
    }
}

impl ExportTemporaryFailure<'_, '_> {
    pub(crate) fn unacquired(&self) -> bool {
        self.allocation.record.is_none()
            && self.allocation.directory.is_none()
            && self.allocation.file.is_none()
    }
}

impl ExportAllocation<'_, '_> {
    pub(crate) fn matches_registration(&self, record: &crate::native_effects::Temporary) -> bool {
        self.record.as_ref().is_some_and(|r| {
            r.operation == record.operation
                && r.item == record.item
                && r.scope_sha256 == record.scope_sha256
                && r.destination == record.destination
                && r.parent == record.parent
                && r.name == record.name
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::native_copy;
    use std::cell::Cell;
    struct Fragmented {
        bytes: Vec<u8>,
        limit: usize,
    }
    impl Write for Fragmented {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.bytes.len() >= self.limit {
                return Err(io::Error::from_raw_os_error(
                    rustix::io::Errno::NOSPC.raw_os_error(),
                ));
            }
            let n = bytes.len().min(7).min(self.limit - self.bytes.len());
            self.bytes.extend_from_slice(&bytes[..n]);
            Ok(n)
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    #[test]
    fn native_export_partial_acquisition_compensates_with_actual_owner_after_fence()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        use crate::{
            database::{Database, TransactionOutcome},
            native::NativeLibrary,
            operations_repository::TerminalRepository,
            profile::NativeProfileStore,
        };
        use librairii_app::ports::{
            creation::{NewItem, NewOperation},
            operations::*,
        };
        use librairii_core::{
            OperationKind, OperationState,
            operations::{ExportSource, ExportSourceType, ItemState, RawOutcome},
        };
        for fault in 0..4 {
            let (roots, copy, locations) = native_copy()?;
            let destination = roots.profile().join("destination");
            std::fs::create_dir(&destination)?;
            let profile = NativeProfileStore::create_fresh(&locations.native)?;
            let native =
                NativeLibrary::bind_independent(copy, &profile, &locations, &Default::default())
                    .map_err(|e| e.to_string())?;
            let mut db = Database::open_native(&native)?;
            let created = match db
                .create_operation(
                    NewOperation {
                        id: "a0000000-0000-4000-8000-000000012500".into(),
                        kind: OperationKind::Export,
                        source: Some(ExportSource {
                            source_type: ExportSourceType::Selection,
                            collection_ids: Vec::new(),
                            collection_names: Vec::new(),
                            shelf_ids: Vec::new(),
                            shelf_names: Vec::new(),
                        }),
                        destination: Some(destination.clone().into()),
                        items: vec![NewItem {
                            source_name: "test.zip".into(),
                            output_name: "test.zip".into(),
                            story_uuid: "e0000000-0000-4000-8000-000000012500".into(),
                            story_title: "Unavailable".into(),
                            export_skip_code: "story_unavailable".into(),
                            ..Default::default()
                        }],
                    },
                    "now".into(),
                    Default::default(),
                )?
                .wait()?
            {
                TransactionOutcome::Committed(v) => v,
                _ => return Err("creation".into()),
            };
            match db
                .mark_operation_item_running(
                    created.operation.identity.id.clone(),
                    created.items[0].identity.id,
                    "later".into(),
                    Default::default(),
                )?
                .wait()?
            {
                TransactionOutcome::Committed(_) => {}
                _ => return Err("start".into()),
            };
            {
                let files = LibraryFiles::open_native(&db)?;
                let custody = files.try_custody()?;
                let mut failure = files
                    .create_export_temporary_with(
                        &custody,
                        created.operation.identity.id.clone(),
                        created.items[0].identity.id,
                        || {
                            if fault == 1 {
                                native.lose_temporary_acknowledgement_for_test()
                            } else {
                                Err(StorageError::DirectoryDurabilityUnavailable)
                            }
                        },
                    )
                    .err()
                    .ok_or("injected acquisition must fail")?;
                assert!(matches!(
                    files.try_custody(),
                    Err(StorageError::CustodyRecoveryRequired)
                ));
                if fault >= 2 {
                    let allocation = &mut failure.allocation;
                    let directory = allocation.directory.as_ref().ok_or("directory")?;
                    native::unlinkat(&directory.file, PAYLOAD, AtFlags::empty())?;
                    // Inject an acknowledgement loss after actual payload retirement.
                    if fault == 3 {
                        let parent = allocation.parent.as_ref().ok_or("parent")?;
                        native::unlinkat(&parent.file, &allocation.name, AtFlags::REMOVEDIR)?;
                        allocation.workspace_retired = true;
                        assert!(
                            native
                                .lose_forget_acknowledgement_for_test(
                                    allocation.record.as_ref().ok_or("record")?
                                )
                                .is_err()
                        );
                    }
                }
                failure.retire_in_place(&custody)?;
                failure.retire_in_place(&custody)?;
                assert!(std::fs::read_dir(&destination)?.next().is_none());
                let repository = TerminalRepository::new(&db, 1);
                let command = CompleteItemCommand {
                    generation: 1,
                    operation: created.operation.immutable_scope(),
                    item: created.items[0].immutable_scope(),
                    story_id: 0,
                    state: ItemState::Failed,
                    completed_bytes: 0,
                    outcome: RawOutcome {
                        code: Some("export_failed".into()),
                        message: Some("The story archive could not be exported.".into()),
                    },
                };
                assert!(matches!(
                    repository.complete_item(&command).disposition,
                    AttemptDisposition::Committed { .. }
                ));
                assert!(matches!(
                    repository
                        .finish(&FinishCommand {
                            generation: 1,
                            operation: created.operation.immutable_scope(),
                            state: OperationState::Failed,
                            outcome: command.outcome.clone(),
                            finished_at: "finished".into()
                        })
                        .disposition,
                    AttemptDisposition::Committed { .. }
                ));
            }
            db.close()?;
            drop(db);
            let failure = native
                .finish()
                .err()
                .ok_or("fenced generation must retain explicit recovery")?;
            drop(failure);
            drop(profile);
            let profile = NativeProfileStore::open_native(&locations.native)?;
            let recovery =
                NativeLibrary::recover_interrupted(&profile, &locations, &Default::default())?;
            let mut db = recovery.open()?;
            assert_eq!(
                db.operation_snapshot(created.operation.identity.id)?
                    .wait()?
                    .ok_or("saved")?
                    .operation
                    .state,
                OperationState::Failed
            );
            db.close()?;
            drop(db);
            recovery.finish().map_err(|e| e.to_string())?;
        }
        Ok(())
    }
    #[test]
    fn native_export_copy_handles_short_writes_and_retains_exact_disk_full_prefix() -> Result<()> {
        let bytes = vec![73u8; 128 * 1024];
        let hash = librairii_core::Checksum::from_bytes(Sha256::digest(&bytes).into());
        let mut complete = Fragmented {
            bytes: Vec::new(),
            limit: bytes.len(),
        };
        copy_bytes(
            &mut bytes.as_slice(),
            &mut complete,
            hash,
            bytes.len() as u64,
            || false,
            |_| {},
        )?;
        assert_eq!(complete.bytes, bytes);
        let mut full = Fragmented {
            bytes: Vec::new(),
            limit: 777,
        };
        assert!(matches!(
            copy_bytes(
                &mut bytes.as_slice(),
                &mut full,
                hash,
                bytes.len() as u64,
                || false,
                |_| {}
            ),
            Err(StorageError::Io { .. })
        ));
        assert_eq!(full.bytes, bytes[..777]);
        Ok(())
    }
    #[test]
    fn native_export_copy_stops_between_chunks_without_claiming_complete_hash() -> Result<()> {
        let bytes = vec![91u8; 256 * 1024];
        let hash = librairii_core::Checksum::from_bytes(Sha256::digest(&bytes).into());
        let cancelled = Cell::new(false);
        let mut target = Vec::new();
        assert!(matches!(
            copy_bytes(
                &mut bytes.as_slice(),
                &mut target,
                hash,
                bytes.len() as u64,
                || cancelled.get(),
                |_| cancelled.set(true)
            ),
            Err(StorageError::Cancelled)
        ));
        assert_eq!(target, bytes[..64 * 1024]);
        Ok(())
    }

    #[test]
    fn native_export_refused_source_never_writes_destination_bytes() {
        struct Locked;
        impl Read for Locked {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::from(io::ErrorKind::PermissionDenied))
            }
        }
        let mut output = Vec::new();
        let hash = librairii_core::Checksum::from_bytes(Sha256::digest([]).into());
        assert!(matches!(
            copy_bytes(&mut Locked, &mut output, hash, 0, || false, |_| {}),
            Err(StorageError::Io { .. })
        ));
        assert!(output.is_empty());
    }
}

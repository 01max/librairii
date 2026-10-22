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
    pub fn publish_registered(&mut self, guard: &Custody<'_>, name: &str) -> Result<()> {
        self.allocation.files.check(guard)?;
        if !librairii_core::operations::valid_output_name(name) {
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
                    |r| r.get::<_, String>(0),
                )?)
            })?
            .wait()?;
        if actual != name {
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
        let record = self
            .record
            .as_ref()
            .ok_or(StorageError::CustodyRecoveryRequired)?;
        self.files.recover_registered_temporary(guard, record)?;
        library.forget_temporary(record)
    }
}
impl ExportTemporaryFailure<'_, '_> {
    pub(crate) fn retire_in_place(&mut self, guard: &Custody<'_>) -> Result<()> {
        self.allocation.retire_registered(guard)
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

#[cfg(test)]
mod tests {
    use super::*;
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

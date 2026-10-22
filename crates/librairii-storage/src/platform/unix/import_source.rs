//! Explicit read-only picker admission and content-addressed stage publication.
use super::*;
use sha2::{Digest, Sha256};
use std::{fs, sync::Arc};

/// Explicit user-selected source. It has no write, delete, cleanup or library
/// authority. Symlink/hardlink aliases may be read, matching legacy import;
/// the opened regular FD and resolved parent stay retained through copy/hash.
#[derive(Debug)]
pub struct PickedSource {
    file: File,
    parent: Directory,
    name: EntryName,
    initial: ReadSourceObservation,
}
impl PickedSource {
    pub fn open(path: &Path) -> Result<Self> {
        let name = EntryName::parse(path.file_name().ok_or(StorageError::InvalidManagedPath)?)?;
        // Resolve a selected source alias once. This path is never reused for
        // publication or cleanup, and no source mutation capability is issued.
        let parent_path = fs::canonicalize(
            path.parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new(".")),
        )
        .map_err(|e| StorageError::io("resolve selected source parent", e))?;
        let parent = Directory::open(&parent_path)?;
        let file: File = native::openat(
            &parent.file,
            name.as_os_str(),
            OFlags::RDONLY | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|e| StorageError::io("open selected source", e.into()))?
        .into();
        let initial = observe(&file, &parent, &name)?;
        Ok(Self {
            file,
            parent,
            name,
            initial,
        })
    }
    pub fn name(&self) -> &EntryName {
        &self.name
    }
    pub const fn bytes(&self) -> u64 {
        self.initial.bytes
    }
    pub fn revalidate(&self) -> Result<()> {
        if observe(&self.file, &self.parent, &self.name)? != self.initial {
            return Err(StorageError::SnapshotChanged);
        }
        Ok(())
    }
    /// One bounded buffer streams source bytes into the actual private stage
    /// while computing SHA-256. Errors retain the caller's actual stage owner.
    /// The stage is independently measured before its later publication.
    pub fn copy_into(
        &mut self,
        stage: &mut OwnedStage<'_, '_>,
        cancel: &WorkCancellation,
    ) -> Result<StreamDigest> {
        self.revalidate()?;
        self.file
            .rewind()
            .map_err(|e| StorageError::io("rewind selected source", e))?;
        let mut buffer = [0u8; super::super::HASH_BUFFER_BYTES];
        let mut hash = Sha256::new();
        let mut bytes = 0u64;
        loop {
            if cancel.is_cancelled() {
                return Err(StorageError::Cancelled);
            }
            self.revalidate()?;
            let request = self
                .initial
                .bytes
                .saturating_sub(bytes)
                .saturating_add(1)
                .min(buffer.len() as u64) as usize;
            let count = self
                .file
                .read(&mut buffer[..request])
                .map_err(|e| StorageError::io("read selected source", e))?;
            if count == 0 {
                break;
            }
            if count as u64 > self.initial.bytes - bytes {
                return Err(StorageError::SnapshotChanged);
            }
            stage
                .write_all(&buffer[..count])
                .map_err(|e| StorageError::io("copy selected source", e))?;
            hash.update(&buffer[..count]);
            bytes = bytes
                .checked_add(count as u64)
                .ok_or(StorageError::HashSizeLimit)?;
        }
        self.revalidate()?;
        if cancel.is_cancelled() {
            return Err(StorageError::Cancelled);
        }
        if bytes != self.initial.bytes {
            return Err(StorageError::SnapshotChanged);
        }
        let measured = StreamDigest {
            checksum: librairii_core::Checksum::from_bytes(hash.finalize().into()),
            bytes,
        };
        if stage.measure(bytes, cancel)? != measured {
            return Err(StorageError::SnapshotChanged);
        }
        Ok(measured)
    }
}
fn observe(file: &File, parent: &Directory, name: &EntryName) -> Result<ReadSourceObservation> {
    let stat = file
        .metadata()
        .map_err(|e| StorageError::io("observe selected source", e))?;
    if !stat.is_file() || stat.len() > i64::MAX as u64 {
        return Err(StorageError::PathEscape);
    }
    let by_name = native::statat(&parent.file, name.as_os_str(), AtFlags::empty())
        .map_err(|e| StorageError::io("revalidate selected source", e.into()))?;
    let identity = FileIdentity::from_file(file)?;
    if FileIdentity::from_stat(&by_name) != identity {
        return Err(StorageError::SnapshotChanged);
    }
    Ok(ReadSourceObservation {
        parent: parent.identity,
        file: identity,
        bytes: stat.len(),
        links: stat.nlink(),
        modified: (stat.mtime(), stat.mtime_nsec()),
        changed: (stat.ctime(), stat.ctime_nsec()),
    })
}

impl<'files, 'library> OwnedStage<'files, 'library> {
    /// Publish exactly archives/<SHA-256>/<original native filename>. The
    /// directory route is created from the actual library/custody owner and
    /// retained through no-clobber publication and SQL acknowledgement.
    pub fn publish_archive(
        mut self,
        guard: &Custody<'_>,
        name: EntryName,
        expected: StreamDigest,
        cancel: &WorkCancellation,
    ) -> std::result::Result<Publication<'files, 'library>, Box<PublishFailure<'files, 'library>>>
    {
        let routed = (|| {
            self.revalidate(guard)?;
            if name.as_os_str().as_bytes().contains(&b'\\') {
                return Err(StorageError::InvalidManagedPath);
            }
            let mut files = LibraryFiles {
                authority: self.files.authority,
                root: self.files.root.duplicate()?,
                staging: self.files.staging.duplicate()?,
                archives: self.files.archives.duplicate()?,
                catalog: self.files.catalog.duplicate()?,
                catalog_route: None,
                archive_route: Some(ArchiveRoute {
                    hash: expected.checksum(),
                    directory: None,
                }),
                trash: self.files.trash.duplicate()?,
            };
            let result = (|| {
                let key = expected.checksum().to_string();
                let directory = match files.archives.child(OsStr::new(&key)) {
                    Ok(d) => d,
                    Err(StorageError::Io { source, .. })
                        if source.kind() == io::ErrorKind::NotFound =>
                    {
                        files.archives.create_child(OsStr::new(&key))?
                    }
                    Err(e) => return Err(e),
                };
                files
                    .archive_route
                    .as_mut()
                    .ok_or(StorageError::CompatibilityInvariant)?
                    .directory = Some(directory);
                files.archives.flush()?;
                files.area(FileArea::Archives).flush()?;
                files.check(guard)
            })();
            // Preserve the new actual route, including partial directory work,
            // in the original stage even when the effect acknowledgement fails.
            self.files = FileLoan::Owned(Arc::new(files));
            if result.is_err() {
                self.files.fence();
            }
            result
        })();
        if let Err(cause) = routed {
            return Err(Box::new(PublishFailure {
                staged: self,
                area: FileArea::Archives,
                name,
                cause,
                publish_uncertain: false,
            }));
        }
        self.publish_no_clobber(guard, FileArea::Archives, name, expected, cancel)
    }
}

#[cfg(test)]
mod tests;

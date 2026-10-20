//! Cooperative OS lease. A legacy executable that ignores it remains excluded
//! only by an independently qualified adoption route, never by this lock alone.
use crate::{Result, StorageError};
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
use std::fs::OpenOptions;
use std::{
    fs::{self, File, Metadata},
    path::{Path, PathBuf},
};

/// Shared with a future explicitly coordinated compatibility bridge. The file
/// is never deleted on unlock; presence or PID contents are not ownership.
pub const LIBRARY_LOCK_FILE: &str = ".librairii-library.lock";

/// Retained by the library authority, beyond DB/read/artwork drain. Dropping
/// the OS handle releases the lock; there is no expiry, detached renewer, or
/// destructive Drop. Constructors are storage-private: a bare path or this
/// cooperative lock is not production adoption permission.
#[derive(Debug)]
pub struct LibraryLease {
    root: PathBuf,
    root_metadata: Metadata,
    file: File,
    file_metadata: Metadata,
    #[cfg(windows)]
    file_identity: super::windows::FileIdentity,
    #[cfg(windows)]
    file_pin: File,
    #[cfg(windows)]
    root_identity: super::windows::FileIdentity,
    #[cfg(windows)]
    root_pin: File,
}

impl LibraryLease {
    pub(crate) fn create_fresh(root: &Path) -> Result<Self> {
        Self::acquire(root, true)
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    pub(crate) fn create_fresh_in(root: &Path, directory: &super::unix::Directory) -> Result<Self> {
        let root_metadata = fs::symlink_metadata(root)
            .map_err(|e| StorageError::io("inspect retained copy lease root", e))?;
        directory.matches_metadata(&root_metadata)?;
        // Creation affects the original NEW directory even if its parent name
        // is replaced after validation. Never create a lock in that substitute.
        let file = directory.open_regular(
            std::ffi::OsStr::new(LIBRARY_LOCK_FILE),
            rustix::fs::OFlags::RDWR | rustix::fs::OFlags::CREATE | rustix::fs::OFlags::EXCL,
        )?;
        Self::finish_acquire(root, root_metadata, file, true)
    }

    fn acquire(root: &Path, create_new: bool) -> Result<Self> {
        if !root.is_absolute() {
            return Err(StorageError::RootNotAbsolute);
        }
        let root_metadata = fs::symlink_metadata(root)
            .map_err(|e| StorageError::io("inspect library lease root", e))?;
        if !root_metadata.is_dir() || root_metadata.file_type().is_symlink() {
            return Err(StorageError::PathEscape);
        }
        let path = root.join(LIBRARY_LOCK_FILE);
        if !create_new {
            require_regular(
                &fs::symlink_metadata(&path)
                    .map_err(|e| StorageError::io("inspect existing library lock", e))?,
            )?;
        }
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        let file = {
            let root_directory = super::unix::Directory::open(root)?;
            root_directory.matches_metadata(&root_metadata)?;
            let mut flags = rustix::fs::OFlags::RDWR;
            if create_new {
                flags |= rustix::fs::OFlags::CREATE | rustix::fs::OFlags::EXCL;
            }
            root_directory.open_regular(std::ffi::OsStr::new(LIBRARY_LOCK_FILE), flags)?
        };
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        let file = {
            let mut options = OpenOptions::new();
            options.read(true).write(true).create_new(create_new);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            options
                .open(&path)
                .map_err(|e| StorageError::io("open library lock", e))?
        };
        Self::finish_acquire(root, root_metadata, file, create_new)
    }

    fn finish_acquire(
        root: &Path,
        root_metadata: Metadata,
        file: File,
        create_new: bool,
    ) -> Result<Self> {
        let file_metadata = file
            .metadata()
            .map_err(|e| StorageError::io("inspect open library lock", e))?;
        require_regular(&file_metadata)?;
        file.try_lock().map_err(|error| match error {
            fs::TryLockError::WouldBlock => StorageError::LibraryInUse,
            fs::TryLockError::Error(error) => StorageError::io("lock library", error),
        })?;
        if create_new {
            file.sync_all()
                .map_err(|e| StorageError::io("flush fresh library lock", e))?;
        }
        #[cfg(windows)]
        let (root_pin, root_identity) = super::windows::metadata_witness(root, true)?;
        #[cfg(windows)]
        let (file_pin, file_identity) =
            super::windows::metadata_witness(&root.join(LIBRARY_LOCK_FILE), false)?;
        #[cfg(windows)]
        if super::windows::query_identity(&file)?.identity != file_identity {
            return Err(StorageError::LibraryLeaseChanged);
        }
        let lease = Self {
            root: root.to_path_buf(),
            root_metadata,
            file,
            file_metadata,
            #[cfg(windows)]
            root_identity,
            #[cfg(windows)]
            root_pin,
            #[cfg(windows)]
            file_identity,
            #[cfg(windows)]
            file_pin,
        };
        lease.revalidate()?;
        Ok(lease)
    }

    /// Checks retained handles and their current names. Descriptor-relative
    /// effects/ancestor-race proof and Windows stable IDs remain4.8/4.9; this
    /// fresh-root capability does not qualify any legacy in-place adoption.
    pub fn revalidate(&self) -> Result<()> {
        #[cfg(windows)]
        {
            use super::windows::{metadata_witness, query_identity};
            let (_, root) = metadata_witness(&self.root, true)?;
            let (_, file) = metadata_witness(&self.root.join(LIBRARY_LOCK_FILE), false)?;
            if root != self.root_identity
                || file != self.file_identity
                || query_identity(&self.root_pin)?.identity != root
                || query_identity(&self.file_pin)?.identity != file
                || query_identity(&self.file)?.identity != file
            {
                return Err(StorageError::LibraryLeaseChanged);
            }
        }
        let root = fs::symlink_metadata(&self.root)
            .map_err(|e| StorageError::io("revalidate library lease root", e))?;
        if !root.is_dir() || root.file_type().is_symlink() {
            return Err(StorageError::LibraryLeaseChanged);
        }
        require_same(&self.root_metadata, &root)?;
        let named = fs::symlink_metadata(self.root.join(LIBRARY_LOCK_FILE))
            .map_err(|e| StorageError::io("revalidate library lock name", e))?;
        require_regular(&named)?;
        require_same(&self.file_metadata, &named)?;
        let opened = self
            .file
            .metadata()
            .map_err(|e| StorageError::io("revalidate retained library lock", e))?;
        require_regular(&opened)?;
        require_same(&self.file_metadata, &opened)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}

fn require_regular(metadata: &Metadata) -> Result<()> {
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(StorageError::PathEscape);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.nlink() != 1 {
            return Err(StorageError::PathEscape);
        }
    }
    Ok(())
}

fn require_same(before: &Metadata, after: &Metadata) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if before.dev() == after.dev() && before.ino() == after.ino() {
            return Ok(());
        }
    }
    #[cfg(not(unix))]
    {
        // Existing/adopted libraries are not opened by this fresh-root route.
        // This is a stability check only, not Windows stable-file-ID proof.
        if before.is_dir() == after.is_dir()
            && before
                .created()
                .ok()
                .zip(after.created().ok())
                .is_some_and(|(a, b)| a == b)
        {
            return Ok(());
        }
    }
    Err(StorageError::LibraryLeaseChanged)
}

#[cfg(test)]
mod tests {
    use super::*;
    use librairii_testkit::TestRoots;
    use std::error::Error;

    #[test]
    fn retained_library_lock_excludes_reopen_and_stale_file_does_not_imply_ownership()
    -> std::result::Result<(), Box<dyn Error>> {
        let roots = TestRoots::new()?;
        let first = LibraryLease::create_fresh(roots.library())?;
        assert!(matches!(
            LibraryLease::acquire(roots.library(), false),
            Err(StorageError::LibraryInUse)
        ));
        drop(first);
        let lock_path = roots.library().join(LIBRARY_LOCK_FILE);
        assert!(lock_path.exists());
        let second = LibraryLease::acquire(roots.library(), false)?;
        second.revalidate()?;
        drop(second);
        assert!(lock_path.exists());
        Ok(())
    }

    #[test]
    #[cfg(unix)]
    fn replaced_or_aliased_library_lock_cannot_refresh_old_lease_authority()
    -> std::result::Result<(), Box<dyn Error>> {
        let roots = TestRoots::new()?;
        let lease = LibraryLease::create_fresh(roots.library())?;
        let path = roots.library().join(LIBRARY_LOCK_FILE);
        fs::hard_link(&path, roots.profile().join("lock-alias"))?;
        assert!(matches!(lease.revalidate(), Err(StorageError::PathEscape)));
        fs::remove_file(roots.profile().join("lock-alias"))?;
        lease.revalidate()?;
        fs::rename(&path, roots.library().join("retained-original-lock"))?;
        fs::write(&path, b"")?;
        assert!(matches!(
            lease.revalidate(),
            Err(StorageError::LibraryLeaseChanged)
        ));
        Ok(())
    }
}

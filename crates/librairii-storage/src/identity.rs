//! A parsed UUID is not its persisted encoding.
use crate::{Result, StorageError};
use librairii_core::LibraryId;
use std::{fs, io::Write, path::Path};

pub const IDENTITY_FILE: &str = ".librairii-library";

/// Retains both the Go TrimSpace identity spelling and the exact original file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LibraryIdentity {
    typed: LibraryId,
    spelling: String,
    bytes: Vec<u8>,
}

impl LibraryIdentity {
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self> {
        let text = std::str::from_utf8(&bytes).map_err(|_| StorageError::InvalidIdentity)?;
        let spelling = text.trim().to_owned();
        let typed = spelling
            .parse()
            .map_err(|_| StorageError::InvalidIdentity)?;
        Ok(Self {
            typed,
            spelling,
            bytes,
        })
    }

    /// Read only: absence is distinct from an invalid file and never repairs it.
    pub fn probe(root: &Path) -> Result<Option<Self>> {
        if !root.is_absolute() {
            return Err(StorageError::RootNotAbsolute);
        }
        let path = root.join(IDENTITY_FILE);
        let metadata = match fs::symlink_metadata(&path) {
            Ok(value) => value,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(StorageError::io("inspect library identity", error)),
        };
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(StorageError::InvalidIdentity);
        }
        let bytes = fs::read(path).map_err(|e| StorageError::io("read library identity", e))?;
        Self::from_bytes(bytes).map(Some)
    }

    /// Only fresh-root initialization calls this; an existing identity is never replaced.
    pub(crate) fn create_new(root: &Path, typed: LibraryId) -> Result<Self> {
        let bytes = format!("{typed}\n").into_bytes();
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(root.join(IDENTITY_FILE))
            .map_err(|e| StorageError::io("create library identity", e))?;
        file.write_all(&bytes)
            .map_err(|e| StorageError::io("write library identity", e))?;
        file.sync_all()
            .map_err(|e| StorageError::io("flush library identity", e))?;
        Self::from_bytes(bytes)
    }

    pub const fn typed(&self) -> LibraryId {
        self.typed
    }
    pub fn spelling(&self) -> &str {
        &self.spelling
    }
    pub fn original_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Profile authority compares the stored spelling, as the Go implementation does.
    pub fn require_spelling(&self, expected: &str) -> Result<()> {
        if self.spelling == expected {
            Ok(())
        } else {
            Err(StorageError::IdentityMismatch)
        }
    }
}

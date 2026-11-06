//! Complete closed-tree data. Serialized entries alone confer no cleanup authority.
use crate::{
    Result, StorageError,
    database::WorkCancellation,
    platform::{
        hash_stream,
        unix::{Directory, EntryName, FileIdentity},
    },
};
use serde::{Deserialize, Serialize};
use std::{
    ffi::OsString,
    fs::{File, Metadata},
    os::unix::{ffi::OsStringExt, fs::MetadataExt},
};
pub(crate) const MAX_MANIFEST_BYTES: usize = 64 * 1024 * 1024;
const MAX_ENTRIES: usize = MAX_MANIFEST_BYTES / 128;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MoveEntry {
    /// Lossless native components. Effects always start at a verified directory.
    pub(crate) path: Vec<Vec<u8>>,
    pub(crate) directory: bool,
    pub(crate) bytes: u64,
    pub(crate) sha256: Option<String>,
    pub(crate) identity: FileIdentity,
    pub(crate) modified: [i64; 4],
}
impl MoveEntry {
    pub(crate) fn names(&self) -> Result<Vec<OsString>> {
        if self.path.is_empty() {
            return Err(StorageError::SettingsDestination);
        }
        self.path
            .iter()
            .map(|name| {
                EntryName::parse(OsString::from_vec(name.clone())).map(|n| n.as_os_str().to_owned())
            })
            .collect()
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ManifestSummary {
    pub files: usize,
    pub directories: usize,
    pub bytes: u64,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelocationManifest {
    pub(crate) entries: Vec<MoveEntry>,
}
impl RelocationManifest {
    pub fn summary(&self) -> Result<ManifestSummary> {
        let mut summary = ManifestSummary {
            files: 0,
            directories: 0,
            bytes: 0,
        };
        for entry in &self.entries {
            if entry.directory {
                summary.directories += 1;
            } else {
                summary.files += 1;
                summary.bytes = summary
                    .bytes
                    .checked_add(entry.bytes)
                    .ok_or(StorageError::HashSizeLimit)?;
            }
        }
        Ok(summary)
    }
    pub fn bytes(&self) -> Result<Vec<u8>> {
        self.validate()?;
        let bytes = serde_json::to_vec(self).map_err(|_| StorageError::SettingsDestination)?;
        if bytes.len() > MAX_MANIFEST_BYTES {
            return Err(StorageError::HashSizeLimit);
        }
        Ok(bytes)
    }
    pub(crate) fn validate(&self) -> Result<()> {
        if self.entries.len() > MAX_ENTRIES {
            return Err(StorageError::HashSizeLimit);
        }
        let mut seen = std::collections::BTreeSet::new();
        for entry in &self.entries {
            entry.names()?;
            if !seen.insert(&entry.path) {
                return Err(StorageError::SettingsDestination);
            }
            if entry.directory {
                if entry.bytes != 0 || entry.sha256.is_some() {
                    return Err(StorageError::SettingsDestination);
                }
            } else if entry
                .sha256
                .as_ref()
                .is_none_or(|hash| hash.parse::<librairii_core::Checksum>().is_err())
            {
                return Err(StorageError::SettingsDestination);
            }
        }
        Ok(())
    }
    pub(crate) fn capture(root: &Directory, cancellation: &WorkCancellation) -> Result<Self> {
        struct Frame {
            directory: Directory,
            path: Vec<Vec<u8>>,
            names: std::vec::IntoIter<OsString>,
            metadata: Metadata,
        }
        fn new_frame(
            directory: Directory,
            path: Vec<Vec<u8>>,
            cancellation: &WorkCancellation,
        ) -> Result<Frame> {
            let metadata = directory.metadata()?;
            let names = directory.entries(MAX_ENTRIES, cancellation)?.into_iter();
            Ok(Frame {
                directory,
                path,
                names,
                metadata,
            })
        }
        let mut stack = vec![new_frame(root.duplicate()?, Vec::new(), cancellation)?];
        let mut entries = Vec::new();
        let mut budget = 0usize;
        while let Some(frame) = stack.last_mut() {
            if cancellation.is_cancelled() {
                return Err(StorageError::Cancelled);
            }
            let Some(name) = frame.names.next() else {
                let after = frame.directory.metadata()?;
                if modified(&frame.metadata) != modified(&after) {
                    return Err(StorageError::SnapshotChanged);
                }
                stack.pop();
                continue;
            };
            let mut path = frame.path.clone();
            path.push(name.as_encoded_bytes().to_vec());
            budget = budget
                .checked_add(
                    160 + path
                        .iter()
                        .map(|p| p.len().saturating_mul(4))
                        .sum::<usize>(),
                )
                .ok_or(StorageError::HashSizeLimit)?;
            if entries.len() >= MAX_ENTRIES || budget > MAX_MANIFEST_BYTES {
                return Err(StorageError::HashSizeLimit);
            }
            match frame.directory.child(&name) {
                Ok(directory) => {
                    let metadata = directory.metadata()?;
                    entries.push(MoveEntry {
                        path: path.clone(),
                        directory: true,
                        bytes: 0,
                        sha256: None,
                        identity: directory.identity(),
                        modified: modified(&metadata),
                    });
                    stack.push(new_frame(directory, path, cancellation)?);
                }
                Err(StorageError::Io { source, .. })
                    if source.kind() == std::io::ErrorKind::NotADirectory =>
                {
                    let mut file = frame
                        .directory
                        .open_regular(&name, rustix::fs::OFlags::RDONLY)?;
                    let before = file
                        .metadata()
                        .map_err(|e| StorageError::io("observe closed manifest file", e))?;
                    let measurement = hash_stream(&mut file, before.len(), cancellation)?;
                    let after = file
                        .metadata()
                        .map_err(|e| StorageError::io("reobserve closed manifest file", e))?;
                    let named = frame
                        .directory
                        .open_regular(&name, rustix::fs::OFlags::RDONLY)?;
                    let named_metadata = named
                        .metadata()
                        .map_err(|e| StorageError::io("reobserve manifest name", e))?;
                    if !same_file(&file, &named)?
                        || before.len() != after.len()
                        || before.len() != named_metadata.len()
                        || measurement.bytes() != before.len()
                        || modified(&before) != modified(&after)
                        || modified(&before) != modified(&named_metadata)
                        || after.nlink() != 1
                    {
                        return Err(StorageError::SnapshotChanged);
                    }
                    entries.push(MoveEntry {
                        path,
                        directory: false,
                        bytes: measurement.bytes(),
                        sha256: Some(measurement.checksum().to_string()),
                        identity: FileIdentity::from_file(&file)?,
                        modified: modified(&after),
                    });
                }
                Err(error) => return Err(error),
            }
        }
        let manifest = Self { entries };
        manifest.bytes()?;
        Ok(manifest)
    }
}
pub(crate) fn modified(metadata: &Metadata) -> [i64; 4] {
    [
        metadata.mtime(),
        metadata.mtime_nsec(),
        metadata.ctime(),
        metadata.ctime_nsec(),
    ]
}
pub(crate) fn same_file(a: &File, b: &File) -> Result<bool> {
    Ok(FileIdentity::from_file(a)? == FileIdentity::from_file(b)?)
}

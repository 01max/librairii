//! Fail-closed legacy handover policy, distinct from a cooperative kernel lock.
use crate::{IsolatedLibrary, Result, StorageError};
use std::path::Path;

const MAX_MANIFEST_BYTES: usize = 64 * 1024 * 1024;

/// Facts from a declared closed export, not ownership or mutation authority.
/// The original Go Manifest bytes remain unchanged; ambiguous/unsafe manifests
/// are refused before any source effect. Complete tree verification is separate.
#[derive(Debug)]
pub struct ClosedManifest {
    bytes: Vec<u8>,
    entries: Vec<ManifestEntry>,
}
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestEntry {
    path: String,
    #[serde(default)]
    size: i64,
    #[serde(default)]
    sha256: Option<String>,
    #[serde(default)]
    directory: bool,
}
impl ClosedManifest {
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > MAX_MANIFEST_BYTES {
            return Err(StorageError::AdoptionInputInvalid);
        }
        let entries: Vec<ManifestEntry> =
            serde_json::from_slice(bytes).map_err(|_| StorageError::AdoptionInputInvalid)?;
        let mut paths = std::collections::BTreeMap::new();
        for entry in &entries {
            // A closed Manifest contains literal canonical relative paths, not
            // instructions to clean/normalize paths before claiming files.
            if entry.path.is_empty()
                || entry.path.contains('\0')
                || entry.path.split('/').any(|p| matches!(p, "" | "." | ".."))
                || entry.size < 0
                || paths.insert(entry.path.as_str(), entry.directory).is_some()
            {
                return Err(StorageError::AdoptionInputInvalid);
            }
            if entry.directory {
                if entry.size != 0 || entry.sha256.as_deref().is_some_and(|h| !h.is_empty()) {
                    return Err(StorageError::AdoptionInputInvalid);
                }
            } else if entry
                .sha256
                .as_deref()
                .and_then(|h| h.parse::<librairii_core::Checksum>().ok())
                .is_none()
            {
                return Err(StorageError::AdoptionInputInvalid);
            }
            if matches!(
                entry.path.as_str(),
                "db/librairii.sqlite3-wal"
                    | "db/librairii.sqlite3-shm"
                    | "db/librairii.sqlite3-journal"
            ) {
                return Err(StorageError::ClosedSnapshotRequired);
            }
        }
        for entry in &entries {
            if let Some((parent, _)) = entry.path.rsplit_once('/')
                && paths.get(parent) != Some(&true)
            {
                return Err(StorageError::AdoptionInputInvalid);
            }
        }
        if paths.get(crate::identity::IDENTITY_FILE) != Some(&false)
            || paths.get("db/librairii.sqlite3") != Some(&false)
        {
            return Err(StorageError::AdoptionInputInvalid);
        }
        Ok(Self {
            bytes: bytes.to_vec(),
            entries,
        })
    }
    pub fn original_bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
mod closed {
    use super::*;
    use crate::{
        LibraryIdentity, LibraryLayout,
        database::WorkCancellation,
        migrations::SchemaPrefix,
        platform::{hash_stream, unix::Directory},
        profile::{MAX_PROFILE_BYTES, ProfileDocument},
    };
    use librairii_core::Checksum;
    use std::{
        collections::BTreeMap,
        ffi::{OsStr, OsString},
        fs::{self, File},
        io::{Read, Seek, SeekFrom, Write},
        os::unix::fs::MetadataExt,
        path::PathBuf,
    };

    /// A retained read-only INPUT witness. It is deliberately not an
    /// IsolatedLibrary, lease, writable root, profile publication permission or
    /// cleanup capability. Independent clone/activation/recovery must separately
    /// establish fresh target ownership, no aliases and durable native authority.
    ///
    /// ```compile_fail
    /// use librairii_storage::{adoption::ClosedLibraryInput, database::Database};
    /// fn input_is_not_a_runtime(input: &ClosedLibraryInput) {
    ///     let _ = Database::open_isolated(input);
    /// }
    /// ```
    #[derive(Debug)]
    pub struct ClosedLibraryInput {
        root: PathBuf,
        directory: Directory,
        manifest: ClosedManifest,
        profile: ProfileDocument,
        identity: LibraryIdentity,
        prefix: SchemaPrefix,
    }
    impl ClosedLibraryInput {
        /// Verify ONLY an explicitly supplied independent closed input. Never
        /// discovers settings, locks/recoveries a source, or opens its SQLite
        /// file: even read-only WAL SQLite could otherwise update source SHM.
        pub fn verify(
            root: &Path,
            profile: ProfileDocument,
            manifest: ClosedManifest,
            cancellation: &WorkCancellation,
        ) -> Result<Self> {
            if cancellation.is_cancelled() {
                return Err(StorageError::Cancelled);
            }
            if !profile.profile().complete && !profile.profile().recovering {
                return Err(StorageError::AdoptionInputInvalid);
            }
            let layout = LibraryLayout::probe(root)?;
            let profile_root = fs::canonicalize(&profile.profile().root)
                .map_err(|e| StorageError::io("resolve explicit input profile root", e))?;
            if profile_root != layout.root() {
                return Err(StorageError::IdentityMismatch);
            }
            let directory = Directory::open(layout.root())?;
            scan(&directory, &manifest, cancellation)?;
            let mut marker = directory
                .read_regular(OsStr::new(crate::identity::IDENTITY_FILE))?
                .ok_or(StorageError::InvalidIdentity)?;
            let mut bytes = Vec::new();
            (&mut marker)
                .take((MAX_PROFILE_BYTES + 1) as u64)
                .read_to_end(&mut bytes)
                .map_err(|e| StorageError::io("read bounded input identity", e))?;
            if bytes.len() > MAX_PROFILE_BYTES {
                return Err(StorageError::AdoptionInputInvalid);
            }
            let identity = LibraryIdentity::from_bytes(bytes)?;
            identity.require_spelling(&profile.profile().library_id)?;

            // Probe only a NEW private copy. A valid known main snapshot with
            // WAL header bytes is accepted when complete and sidecar-free;
            // the presence/absence of an old PID/lock file never proves that.
            let scratch = tempfile::tempdir()
                .map_err(|e| StorageError::io("create independent input probe", e))?;
            let staging = scratch.path().join("staging");
            fs::create_dir(&staging)
                .map_err(|e| StorageError::io("create private probe staging", e))?;
            let db = manifest
                .entries
                .iter()
                .find(|e| e.path == "db/librairii.sqlite3")
                .ok_or(StorageError::AdoptionInputInvalid)?;
            let parent = directory.child(OsStr::new("db"))?;
            let mut source = parent
                .read_regular(OsStr::new("librairii.sqlite3"))?
                .ok_or(StorageError::AdoptionInputChanged)?;
            verify_file(&mut source, db, cancellation)?;
            source
                .seek(SeekFrom::Start(0))
                .map_err(|e| StorageError::io("rewind verified input database", e))?;
            let probe = scratch.path().join("input.sqlite3");
            let mut options = fs::OpenOptions::new();
            use std::os::unix::fs::OpenOptionsExt;
            options.write(true).create_new(true).mode(0o600);
            let mut output = options
                .open(&probe)
                .map_err(|e| StorageError::io("create independent input database", e))?;
            let limit = u64::try_from(db.size).map_err(|_| StorageError::AdoptionInputInvalid)?;
            let mut reader = InputCopyReader {
                file: &mut source,
                cancellation,
                cancelled: false,
            };
            let count = match std::io::copy(&mut reader.by_ref().take(limit + 1), &mut output) {
                Ok(count) => count,
                Err(_) if reader.cancelled => return Err(StorageError::Cancelled),
                Err(error) => {
                    return Err(StorageError::io("copy independent input database", error));
                }
            };
            if count != limit {
                return Err(StorageError::AdoptionInputChanged);
            }
            output
                .flush()
                .map_err(|e| StorageError::io("flush private probe stream", e))?;
            drop(output);
            // Hash the COPY too; the source may have changed during copying.
            let mut copy = File::open(&probe)
                .map_err(|e| StorageError::io("read independent input database", e))?;
            verify_file(&mut copy, db, cancellation)?;
            drop(copy);
            let prefix = crate::backup::probe_owned(&probe, &staging)?;
            scan(&directory, &manifest, cancellation)?;
            directory.matches_metadata(
                &fs::symlink_metadata(layout.root())
                    .map_err(|e| StorageError::io("revalidate explicit input root", e))?,
            )?;
            Ok(Self {
                root: layout.root().to_path_buf(),
                directory,
                manifest,
                profile,
                identity,
                prefix,
            })
        }
        pub fn root(&self) -> &Path {
            &self.root
        }
        pub fn identity(&self) -> &LibraryIdentity {
            &self.identity
        }
        pub fn profile(&self) -> &ProfileDocument {
            &self.profile
        }
        pub fn manifest(&self) -> &ClosedManifest {
            &self.manifest
        }
        pub const fn prefix(&self) -> SchemaPrefix {
            self.prefix
        }
        /// Fresh content/name proof before any eventual clone action. Read-only
        /// verification never converts error, manifest tuple or path into source
        /// deletion authority; legacy in-place opening remains refused.
        pub fn revalidate(&self, cancellation: &WorkCancellation) -> Result<()> {
            self.directory.matches_metadata(
                &fs::symlink_metadata(&self.root)
                    .map_err(|e| StorageError::io("revalidate retained input root", e))?,
            )?;
            scan(&self.directory, &self.manifest, cancellation)
        }
    }

    struct InputCopyReader<'a> {
        file: &'a mut File,
        cancellation: &'a WorkCancellation,
        cancelled: bool,
    }
    impl Read for InputCopyReader<'_> {
        fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
            if self.cancellation.is_cancelled() {
                self.cancelled = true;
                // copy retries Interrupted indefinitely; cancellation must be
                // a terminal transport outcome that this owner classifies.
                return Err(std::io::Error::other("closed input copy cancelled"));
            }
            self.file.read(bytes)
        }
    }

    fn directory_at(
        root: &Directory,
        path: &str,
        cancellation: &WorkCancellation,
    ) -> Result<Directory> {
        let mut current = root.duplicate()?;
        if !path.is_empty() {
            for part in path.split('/') {
                if cancellation.is_cancelled() {
                    return Err(StorageError::Cancelled);
                }
                current = current.child(OsStr::new(part))?;
            }
        }
        Ok(current)
    }
    fn scan(
        root: &Directory,
        manifest: &ClosedManifest,
        cancellation: &WorkCancellation,
    ) -> Result<()> {
        if cancellation.is_cancelled() {
            return Err(StorageError::Cancelled);
        }
        let mut children: BTreeMap<&str, Vec<OsString>> = BTreeMap::new();
        children.insert("", Vec::new());
        for entry in &manifest.entries {
            let (parent, name) = entry.path.rsplit_once('/').unwrap_or(("", &entry.path));
            children
                .entry(parent)
                .or_default()
                .push(OsString::from(name));
            if entry.directory {
                children.entry(&entry.path).or_default();
            }
        }
        for (parent, mut expected) in children {
            expected.sort();
            let directory = directory_at(root, parent, cancellation)?;
            if directory.entries(expected.len(), cancellation)? != expected {
                return Err(StorageError::AdoptionInputChanged);
            }
        }
        for entry in &manifest.entries {
            if entry.directory {
                continue;
            }
            if cancellation.is_cancelled() {
                return Err(StorageError::Cancelled);
            }
            let (parent, name) = entry.path.rsplit_once('/').unwrap_or(("", &entry.path));
            let directory = directory_at(root, parent, cancellation)?;
            let mut file = directory
                .read_regular(OsStr::new(name))?
                .ok_or(StorageError::AdoptionInputChanged)?;
            verify_file(&mut file, entry, cancellation)?;
        }
        Ok(())
    }
    fn verify_file(
        file: &mut File,
        entry: &ManifestEntry,
        cancellation: &WorkCancellation,
    ) -> Result<()> {
        let before = file
            .metadata()
            .map_err(|e| StorageError::io("inspect closed input file", e))?;
        let size = u64::try_from(entry.size).map_err(|_| StorageError::AdoptionInputInvalid)?;
        if !before.is_file() || before.nlink() != 1 || before.len() != size {
            return Err(StorageError::AdoptionInputChanged);
        }
        let expected: Checksum = entry
            .sha256
            .as_deref()
            .ok_or(StorageError::AdoptionInputInvalid)?
            .parse()
            .map_err(|_| StorageError::AdoptionInputInvalid)?;
        let actual = hash_stream(file, before.len(), cancellation)?;
        let after = file
            .metadata()
            .map_err(|e| StorageError::io("revalidate closed input file", e))?;
        if actual.checksum() != expected
            || actual.bytes() != before.len()
            || after.len() != before.len()
            || after.dev() != before.dev()
            || after.ino() != before.ino()
            || after.nlink() != 1
            || after.modified().ok() != before.modified().ok()
        {
            return Err(StorageError::AdoptionInputChanged);
        }
        Ok(())
    }
}
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub use closed::ClosedLibraryInput;

/// No currently admitted bridge/exclusion mechanism can stop a later
/// unmodified Go launch through its retained root/profile/aliases. Therefore
/// this route refuses before any I/O, including lock-file creation or recovery.
/// A PID scan, user promise, Rust OS lease or bridge-installation flag cannot
/// manufacture the missing lifetime exclusion proof.
///
/// Production use requires the separately verified independent clone/profile/
/// recovery route. Its streaming hash/whole-manifest and rebound journal proof
/// must be implemented and qualified before any existing source is writable.
pub fn open_legacy_in_place(_root: &Path) -> Result<IsolatedLibrary> {
    Err(StorageError::LegacyLifetimeExclusionUnproven)
}

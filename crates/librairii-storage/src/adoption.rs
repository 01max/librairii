//! Fail-closed legacy handover policy, distinct from a cooperative kernel lock.
use crate::{IsolatedLibrary, Result, StorageError};
use std::path::Path;

const MAX_MANIFEST_BYTES: usize = 64 * 1024 * 1024;

/// Facts from a declared closed export, not ownership or mutation authority.
/// The original Go Manifest bytes remain unchanged; ambiguous/unsafe manifests
/// are refused before any source effect. Complete tree verification is separate.
#[derive(Clone, Debug)]
pub struct ClosedManifest {
    bytes: Vec<u8>,
    entries: Vec<ManifestEntry>,
}
#[derive(Clone, Debug, serde::Deserialize)]
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
            if entry
                .path
                .split('/')
                .next()
                .is_some_and(crate::native::reserved_control_name)
                || entry.path.is_empty()
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
    pub(crate) fn relative_paths(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|entry| entry.path.as_str())
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
mod closed {
    use super::*;
    use crate::{
        LibraryIdentity, LibraryLayout,
        database::WorkCancellation,
        migrations::SchemaPrefix,
        platform::{
            hash_stream,
            lock::{LIBRARY_LOCK_FILE, LibraryLease},
            unix::{Directory, EntryName, FileIdentity, durable_file_flush},
        },
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

        /// Copy to a NEW explicit root with independent files and a retained
        /// cooperative OS lease. The result is INACTIVE: source/profile/journal
        /// authority is unchanged, and copied recovery paths grant no effects.
        /// All literal names and empty-file/directory durability are exercised
        /// on the target before copying payloads; aliases or changed spellings
        /// refuse, never rename/truncate. This observes only this actual target.
        /// Native profile/recovery separation and activation are distinct gates.
        /// Failure never recursively removes a root or the only partial copy.
        pub fn copy_to_new_root(
            &self,
            target: &Path,
            cancellation: &WorkCancellation,
        ) -> std::result::Result<IndependentCopy, Box<IndependentCopyFailure>> {
            self.copy_with_checkpoints(target, cancellation, |_| Ok(()), || Ok(()))
        }

        #[cfg(test)]
        fn copy_with_checkpoint(
            &self,
            target: &Path,
            cancellation: &WorkCancellation,
            before_final_flush: impl FnOnce() -> Result<()>,
        ) -> std::result::Result<IndependentCopy, Box<IndependentCopyFailure>> {
            self.copy_with_checkpoints(target, cancellation, |_| Ok(()), before_final_flush)
        }

        fn copy_with_checkpoints(
            &self,
            target: &Path,
            cancellation: &WorkCancellation,
            after_name_preflight: impl FnOnce(&CopyCandidate) -> Result<()>,
            before_final_flush: impl FnOnce() -> Result<()>,
        ) -> std::result::Result<IndependentCopy, Box<IndependentCopyFailure>> {
            let mut retained = None;
            let mut attempted_target = None;
            let result = (|| {
                self.revalidate(cancellation)?;
                if !target.is_absolute() {
                    return Err(StorageError::RootNotAbsolute);
                }
                // The new lock is an owned runtime control object, not an old
                // source file to replace. Such an input needs another route.
                if self
                    .manifest
                    .entries
                    .iter()
                    .any(|e| e.path == LIBRARY_LOCK_FILE)
                {
                    return Err(StorageError::AdoptionInputInvalid);
                }
                let name =
                    EntryName::parse(target.file_name().ok_or(StorageError::InvalidManagedPath)?)?;
                let parent_path =
                    fs::canonicalize(target.parent().ok_or(StorageError::RootNotAbsolute)?)
                        .map_err(|e| StorageError::io("resolve independent copy parent", e))?;
                let root = parent_path.join(name.as_os_str());
                if root.starts_with(&self.root) || self.root.starts_with(&root) {
                    return Err(StorageError::PathEscape);
                }
                let parent = Directory::open(&parent_path)?;
                // mkdir may succeed even if obtaining/validating the child
                // handle then fails. No retained child is NOT no-effect proof.
                attempted_target = Some(root.clone());
                let directory = parent.create_child(name.as_os_str())?;
                retained = Some(CopyCandidate {
                    root,
                    directory,
                    parent,
                    name,
                    preflight_directories: BTreeMap::new(),
                    preflight_files: BTreeMap::new(),
                    lease: None,
                });
                let candidate = retained
                    .as_mut()
                    .ok_or(StorageError::AdoptionInputInvalid)?;
                candidate.revalidate()?;
                candidate.lease = Some(LibraryLease::create_fresh_in(
                    &candidate.root,
                    &candidate.directory,
                )?);
                candidate.revalidate()?;

                // Parents precede their children regardless of manifest order.
                let mut directories: Vec<_> = self
                    .manifest
                    .entries
                    .iter()
                    .filter(|e| e.directory)
                    .collect();
                directories.sort_by_key(|e| e.path.split('/').count());
                for entry in &directories {
                    if cancellation.is_cancelled() {
                        return Err(StorageError::Cancelled);
                    }
                    let (parent, name) = entry.path.rsplit_once('/').unwrap_or(("", &entry.path));
                    let child = target_directory_at(candidate, parent, cancellation)?
                        .create_child(OsStr::new(name))?;
                    candidate
                        .preflight_directories
                        .insert(entry.path.clone(), child);
                }
                // Exercise EVERY literal component on this actual target
                // filesystem before copying any source payload. Exclusive
                // creation rejects case/normalization aliases and real length
                // limits; enumeration rejects transformed spellings. These are
                // the copy's actual empty files, not disposable probe names.
                for entry in self.manifest.entries.iter().filter(|e| !e.directory) {
                    if cancellation.is_cancelled() {
                        return Err(StorageError::Cancelled);
                    }
                    let (parent, name) = entry.path.rsplit_once('/').unwrap_or(("", &entry.path));
                    let file = target_directory_at(candidate, parent, cancellation)?.open_regular(
                        OsStr::new(name),
                        rustix::fs::OFlags::RDWR
                            | rustix::fs::OFlags::CREATE
                            | rustix::fs::OFlags::EXCL,
                    )?;
                    // Capture the actual exclusive-create owner before any
                    // further fallible boundary; a saved inode is not authority.
                    candidate.preflight_files.insert(entry.path.clone(), file);
                    durable_file_flush(
                        candidate
                            .preflight_files
                            .get(&entry.path)
                            .ok_or(StorageError::AdoptionInputChanged)?,
                    )?;
                }
                check_names(
                    &candidate.directory,
                    &self.manifest,
                    cancellation,
                    true,
                    false,
                )?;
                // Observe the existing durability primitives on this actual
                // filesystem before payload copying as well. This is a finite
                // acknowledgement, not a guarantee against future I/O failure.
                for entry in directories.iter().rev() {
                    target_directory_at(candidate, &entry.path, cancellation)?.flush()?;
                }
                candidate.directory.flush()?;
                candidate.parent.flush()?;
                candidate.revalidate()?;
                after_name_preflight(candidate)?;
                candidate.revalidate_targets(cancellation)?;
                for entry in self.manifest.entries.iter().filter(|e| !e.directory) {
                    if cancellation.is_cancelled() {
                        return Err(StorageError::Cancelled);
                    }
                    let (parent, name) = entry.path.rsplit_once('/').unwrap_or(("", &entry.path));
                    let source_parent = directory_at(&self.directory, parent, cancellation)?;
                    let mut source = source_parent
                        .read_regular(OsStr::new(name))?
                        .ok_or(StorageError::AdoptionInputChanged)?;
                    verify_file(&mut source, entry, cancellation)?;
                    source
                        .seek(SeekFrom::Start(0))
                        .map_err(|e| StorageError::io("rewind independent copy source", e))?;
                    candidate.revalidate_target_file(&entry.path, cancellation)?;
                    // Write through the SAME actual File returned by CREATE|
                    // EXCL. Reopened names are observations only, never writers.
                    let output = candidate
                        .preflight_files
                        .get_mut(&entry.path)
                        .ok_or(StorageError::AdoptionInputChanged)?;
                    if output
                        .metadata()
                        .map_err(|e| StorageError::io("revalidate empty target file", e))?
                        .len()
                        != 0
                    {
                        return Err(StorageError::AdoptionInputChanged);
                    }
                    let mut reader = InputCopyReader {
                        file: &mut source,
                        cancellation,
                        cancelled: false,
                    };
                    let limit = u64::try_from(entry.size)
                        .map_err(|_| StorageError::AdoptionInputInvalid)?;
                    match std::io::copy(&mut reader.by_ref().take(limit + 1), output) {
                        Ok(count) if count == limit => (),
                        Ok(_) => return Err(StorageError::AdoptionInputChanged),
                        Err(_) if reader.cancelled => return Err(StorageError::Cancelled),
                        Err(error) => {
                            return Err(StorageError::io("stream independent copy", error));
                        }
                    }
                    output
                        .seek(SeekFrom::Start(0))
                        .map_err(|e| StorageError::io("rewind independent copy output", e))?;
                    verify_file(output, entry, cancellation)?;
                    durable_file_flush(output)?;
                }
                self.revalidate(cancellation)?;
                candidate.revalidate_targets(cancellation)?;
                scan_with_lock(&candidate.directory, &self.manifest, cancellation, true)?;
                let scratch = tempfile::tempdir()
                    .map_err(|e| StorageError::io("create copied database probe", e))?;
                if crate::backup::probe_owned(
                    &candidate.root.join("db/librairii.sqlite3"),
                    scratch.path(),
                )? != self.prefix
                {
                    return Err(StorageError::AdoptionInputChanged);
                }
                before_final_flush()?;
                // Flush children before parents, then the NEW root's name in
                // its retained parent. A failed acknowledgement retains both
                // actual directory witnesses and the OS lease in the failure.
                for entry in directories.iter().rev() {
                    target_directory_at(candidate, &entry.path, cancellation)?.flush()?;
                }
                candidate.directory.flush()?;
                candidate.parent.flush()?;
                candidate.revalidate_targets(cancellation)?;
                Ok(())
            })();
            match (result, retained) {
                (Ok(()), Some(mut candidate)) => {
                    // Copy effects and final name/flush checks are complete.
                    // No writer can be reconstructed from these observations.
                    // Failures never reach this release point.
                    candidate.preflight_files.clear();
                    candidate.preflight_directories.clear();
                    Ok(IndependentCopy {
                        candidate,
                        manifest: self.manifest.clone(),
                        source_profile: self.profile.clone(),
                        source_root: self.root.clone(),
                        identity: self.identity.clone(),
                        prefix: self.prefix,
                        native_record: None,
                    })
                }
                (Err(cause), retained) => Err(Box::new(IndependentCopyFailure {
                    cause,
                    retained,
                    attempted_target,
                })),
                (Ok(()), None) => Err(Box::new(IndependentCopyFailure {
                    cause: StorageError::AdoptionInputInvalid,
                    retained: None,
                    attempted_target,
                })),
            }
        }
    }

    #[derive(Debug)]
    struct CopyCandidate {
        root: PathBuf,
        directory: Directory,
        parent: Directory,
        name: EntryName,
        // Physical exclusive-create witnesses survive the whole copy interval
        // and every failed outcome, including unlink/substitution of the name.
        preflight_directories: BTreeMap<String, Directory>,
        preflight_files: BTreeMap<String, File>,
        // Last: the copy lease outlives every retained object/stream.
        lease: Option<LibraryLease>,
    }
    impl CopyCandidate {
        fn revalidate_target_file(
            &self,
            path: &str,
            cancellation: &WorkCancellation,
        ) -> Result<()> {
            let held = self
                .preflight_files
                .get(path)
                .ok_or(StorageError::AdoptionInputChanged)?;
            let metadata = held
                .metadata()
                .map_err(|e| StorageError::io("inspect held copy target", e))?;
            if !metadata.is_file() || metadata.nlink() != 1 {
                return Err(StorageError::AdoptionInputChanged);
            }
            let (parent, name) = path.rsplit_once('/').unwrap_or(("", path));
            let named = target_directory_at(self, parent, cancellation)?
                .open_regular(OsStr::new(name), rustix::fs::OFlags::RDONLY)?;
            if FileIdentity::from_file(held)? != FileIdentity::from_file(&named)? {
                return Err(StorageError::AdoptionInputChanged);
            }
            Ok(())
        }
        fn revalidate_targets(&self, cancellation: &WorkCancellation) -> Result<()> {
            self.revalidate()?;
            for path in self.preflight_directories.keys() {
                target_directory_at(self, path, cancellation)?;
            }
            for path in self.preflight_files.keys() {
                self.revalidate_target_file(path, cancellation)?;
            }
            Ok(())
        }
        fn revalidate(&self) -> Result<()> {
            self.directory.matches_metadata(
                &fs::symlink_metadata(&self.root)
                    .map_err(|e| StorageError::io("revalidate independent copy root", e))?,
            )?;
            let named = self.parent.child(self.name.as_os_str())?;
            named.matches_metadata(
                &fs::symlink_metadata(&self.root)
                    .map_err(|e| StorageError::io("revalidate independent copy name", e))?,
            )?;
            if let Some(lease) = &self.lease {
                lease.revalidate()?;
            }
            Ok(())
        }
    }

    /// A verified independent byte copy, deliberately not an active library.
    /// Raw source profile/recovery paths remain evidence, NEVER local cleanup
    /// authority. No conversion to IsolatedLibrary or Database is provided.
    #[derive(Debug)]
    pub struct IndependentCopy {
        manifest: ClosedManifest,
        source_profile: ProfileDocument,
        source_root: PathBuf,
        identity: LibraryIdentity,
        prefix: SchemaPrefix,
        candidate: CopyCandidate,
        native_record: Option<(File, Vec<u8>)>,
    }
    impl IndependentCopy {
        pub(crate) fn attach_native_record(&mut self, file: File, bytes: Vec<u8>) -> Result<()> {
            if self.native_record.is_some() || bytes.len() > MAX_PROFILE_BYTES {
                return Err(StorageError::AdoptionInputChanged);
            }
            self.native_record = Some((file, bytes));
            self.revalidate(&WorkCancellation::default())
        }
        pub(crate) fn manifest_digest(&self) -> String {
            crate::native::digest(self.manifest.original_bytes())
        }
        pub fn root(&self) -> &Path {
            &self.candidate.root
        }
        pub fn identity(&self) -> &LibraryIdentity {
            &self.identity
        }
        pub fn source_profile(&self) -> &ProfileDocument {
            &self.source_profile
        }
        pub(crate) fn source_root(&self) -> &Path {
            &self.source_root
        }
        pub(crate) fn retain_root(&self) -> Result<Directory> {
            self.candidate.directory.duplicate()
        }
        pub(crate) fn revalidate_root_lease(&self) -> Result<()> {
            self.candidate.revalidate()
        }
        pub(crate) fn require_no_recovery_input(&self) -> Result<()> {
            if !self.source_profile.profile().complete
                || self.source_profile.profile().recovering
                || self.manifest.entries.iter().any(|entry| {
                    entry.path.split('/').any(|part| {
                        part == "relocation.json" || part.starts_with(".librairii-move-")
                    })
                })
            {
                return Err(StorageError::RecoveryBindingRequired);
            }
            Ok(())
        }
        pub const fn prefix(&self) -> SchemaPrefix {
            self.prefix
        }
        pub fn revalidate(&self, cancellation: &WorkCancellation) -> Result<()> {
            self.candidate.revalidate()?;
            scan_owned_controls(
                &self.candidate.directory,
                &self.manifest,
                cancellation,
                true,
                self.native_record.as_ref(),
            )
        }
    }

    /// Inactive partial bytes survive failure. If `retained_root` is present,
    /// this object retains acquired preflight files/directories and the lease,
    /// not just numeric identities or a path receipt. It never deletes in Drop or activates
    /// a copied profile/journal. Release/abandonment leaves all bytes in place.
    #[derive(Debug)]
    pub struct IndependentCopyFailure {
        cause: StorageError,
        retained: Option<CopyCandidate>,
        attempted_target: Option<PathBuf>,
    }
    impl IndependentCopyFailure {
        pub fn storage_error(&self) -> &StorageError {
            &self.cause
        }
        /// Creation was attempted at this last observed name; an empty
        /// unqualified root MAY remain even if no child handle was obtained.
        /// This is never no-effect, cleanup or retry authority.
        pub fn attempted_target(&self) -> Option<&Path> {
            self.attempted_target.as_deref()
        }
        /// Last observed name only. Substitution can make it point elsewhere;
        /// neither this path nor `retains_lease` authorizes cleanup/activation.
        pub fn retained_root(&self) -> Option<&Path> {
            self.retained.as_ref().map(|c| c.root.as_path())
        }
        pub fn retains_lease(&self) -> bool {
            self.retained.as_ref().is_some_and(|c| c.lease.is_some())
        }
    }
    impl std::fmt::Display for IndependentCopyFailure {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "inactive independent copy failed: {}", self.cause)
        }
    }
    impl std::error::Error for IndependentCopyFailure {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            Some(&self.cause)
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
    fn target_directory_at<'candidate>(
        candidate: &'candidate CopyCandidate,
        path: &str,
        cancellation: &WorkCancellation,
    ) -> Result<&'candidate Directory> {
        candidate.revalidate()?;
        let mut current = &candidate.directory;
        let mut end = 0;
        for part in path.split('/').filter(|part| !part.is_empty()) {
            if cancellation.is_cancelled() {
                return Err(StorageError::Cancelled);
            }
            end += part.len();
            let held = candidate
                .preflight_directories
                .get(&path[..end])
                .ok_or(StorageError::AdoptionInputChanged)?;
            let named = current.child(OsStr::new(part))?;
            // Both original and named objects are live during comparison;
            // the returned authority is the ORIGINAL retained directory.
            if named.identity() != held.identity() {
                return Err(StorageError::AdoptionInputChanged);
            }
            current = held;
            end += 1;
        }
        Ok(current)
    }
    fn scan(
        root: &Directory,
        manifest: &ClosedManifest,
        cancellation: &WorkCancellation,
    ) -> Result<()> {
        scan_with_lock(root, manifest, cancellation, false)
    }
    fn scan_with_lock(
        root: &Directory,
        manifest: &ClosedManifest,
        cancellation: &WorkCancellation,
        owned_lock: bool,
    ) -> Result<()> {
        scan_owned_controls(root, manifest, cancellation, owned_lock, None)
    }
    fn scan_owned_controls(
        root: &Directory,
        manifest: &ClosedManifest,
        cancellation: &WorkCancellation,
        owned_lock: bool,
        record: Option<&(File, Vec<u8>)>,
    ) -> Result<()> {
        if cancellation.is_cancelled() {
            return Err(StorageError::Cancelled);
        }
        if let Some((witness, bytes)) = record {
            let mut file = root.open_regular(
                OsStr::new(crate::native::NATIVE_STATE_FILE),
                rustix::fs::OFlags::RDONLY,
            )?;
            let before = witness
                .metadata()
                .map_err(|e| StorageError::io("inspect copied native record witness", e))?;
            let actual = file
                .metadata()
                .map_err(|e| StorageError::io("inspect copied native record name", e))?;
            if before.nlink() != 1
                || actual.nlink() != 1
                || before.dev() != actual.dev()
                || before.ino() != actual.ino()
            {
                return Err(StorageError::AdoptionInputChanged);
            }
            let mut found = Vec::new();
            Read::by_ref(&mut file)
                .take((MAX_PROFILE_BYTES + 1) as u64)
                .read_to_end(&mut found)
                .map_err(|e| StorageError::io("read copied native record", e))?;
            if &found != bytes {
                return Err(StorageError::AdoptionInputChanged);
            }
        }
        check_names(root, manifest, cancellation, owned_lock, record.is_some())?;
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
    fn check_names(
        root: &Directory,
        manifest: &ClosedManifest,
        cancellation: &WorkCancellation,
        owned_lock: bool,
        owned_record: bool,
    ) -> Result<()> {
        let mut children: BTreeMap<&str, Vec<OsString>> = BTreeMap::new();
        let controls = children.entry("").or_default();
        if owned_lock {
            controls.push(OsString::from(LIBRARY_LOCK_FILE));
        }
        if owned_record {
            controls.push(OsString::from(crate::native::NATIVE_STATE_FILE));
        }
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

    #[cfg(test)]
    mod copy_failures {
        use super::*;
        use librairii_testkit::TestRoots;
        use std::error::Error;

        fn input(roots: &TestRoots) -> std::result::Result<ClosedLibraryInput, Box<dyn Error>> {
            let root = roots.library().join("closed-input");
            fs::create_dir(&root)?;
            fs::create_dir(root.join("db"))?;
            let corpus = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../preservation/corpus/libraries/schema018");
            let mut entries = vec![serde_json::json!({"path":"db","size":0,"directory":true})];
            for name in [".librairii-library", "db/librairii.sqlite3"] {
                fs::copy(corpus.join(name), root.join(name))?;
                let mut file = File::open(root.join(name))?;
                let size = file.metadata()?.len();
                let digest = hash_stream(&mut file, size, &WorkCancellation::default())?;
                entries.push(serde_json::json!({"path":name,"size":size,"sha256":digest.checksum().to_string()}));
            }
            let name = "raw Éte # %2F : ?.bin";
            let payload = b"literal filename payload";
            fs::write(root.join(name), payload)?;
            let digest = hash_stream(
                &mut payload.as_slice(),
                payload.len() as u64,
                &WorkCancellation::default(),
            )?;
            entries.push(serde_json::json!({"path":name,"size":payload.len(),"sha256":digest.checksum().to_string()}));
            let profile = serde_json::json!({"version":1,"language":"fr","root":root,"libraryId":"a0000000-0000-4000-8000-000000000001","complete":true});
            Ok(ClosedLibraryInput::verify(
                &root,
                ProfileDocument::decode(&serde_json::to_vec(&profile)?)?,
                ClosedManifest::decode(&serde_json::to_vec(&entries)?)?,
                &WorkCancellation::default(),
            )?)
        }

        #[test]
        fn failed_copy_flush_retains_real_root_lease_and_every_recoverable_byte()
        -> std::result::Result<(), Box<dyn Error>> {
            let roots = TestRoots::new()?;
            let input = input(&roots)?;
            let original = fs::read(input.root.join("db/librairii.sqlite3"))?;
            let preflight = roots.library().join("preflight-no-payload");
            let failed = input
                .copy_with_checkpoints(
                    &preflight,
                    &WorkCancellation::default(),
                    |candidate| {
                        // Every actual target spelling exists before ANY source
                        // payload is copied, including raw punctuation/Unicode.
                        for entry in input.manifest.entries.iter().filter(|e| !e.directory) {
                            assert_eq!(
                                candidate
                                    .preflight_files
                                    .get(&entry.path)
                                    .ok_or(StorageError::AdoptionInputChanged)?
                                    .metadata()
                                    .map_err(|e| StorageError::io("test held preflight file", e))?
                                    .len(),
                                0
                            );
                            assert_eq!(
                                fs::metadata(preflight.join(&entry.path))
                                    .map_err(|e| StorageError::io(
                                        "test inspect preflight payload",
                                        e
                                    ))?
                                    .len(),
                                0
                            );
                        }
                        Err(StorageError::DirectoryDurabilityUnavailable)
                    },
                    || Ok(()),
                )
                .err()
                .ok_or("preflight boundary must fail")?;
            assert!(failed.retains_lease());
            assert_eq!(
                fs::metadata(preflight.join("db/librairii.sqlite3"))?.len(),
                0
            );
            assert_eq!(
                fs::metadata(preflight.join("raw Éte # %2F : ?.bin"))?.len(),
                0
            );
            assert_eq!(fs::read(input.root.join("db/librairii.sqlite3"))?, original);
            // Retaining/restoring fixture resources never cleans the candidate.
            drop(failed);
            assert!(preflight.join(LIBRARY_LOCK_FILE).is_file());

            let replaced = roots.library().join("preflight-replaced");
            let failed = input
                .copy_with_checkpoints(
                    &replaced,
                    &WorkCancellation::default(),
                    |_| {
                        fs::rename(
                            replaced.join("db/librairii.sqlite3"),
                            replaced.join("held-original"),
                        )
                        .map_err(|e| StorageError::io("test retain preflight file", e))?;
                        fs::write(replaced.join("db/librairii.sqlite3"), [])
                            .map_err(|e| StorageError::io("test replace preflight file", e))?;
                        Ok(())
                    },
                    || Ok(()),
                )
                .err()
                .ok_or("same-name empty replacement must refuse")?;
            assert!(matches!(
                failed.storage_error(),
                StorageError::AdoptionInputChanged
            ));
            assert!(failed.retains_lease());
            assert_eq!(
                fs::metadata(replaced.join("db/librairii.sqlite3"))?.len(),
                0
            );
            assert_eq!(fs::metadata(replaced.join("held-original"))?.len(), 0);

            // Deterministic lifetime regression, not an inode-reuse assay:
            // unlink/recreate while the original created objects MUST remain
            // physically held through the returned failure. No allocation loop
            // or forged identity is needed to prove that ownership interval.
            for replace_directory in [false, true] {
                let target = roots.library().join(if replace_directory {
                    "unlinked-preflight-directory"
                } else {
                    "unlinked-preflight-file"
                });
                let failed = input
                    .copy_with_checkpoints(
                        &target,
                        &WorkCancellation::default(),
                        |candidate| {
                            let held = candidate
                                .preflight_files
                                .get("db/librairii.sqlite3")
                                .ok_or(StorageError::AdoptionInputChanged)?;
                            let original = FileIdentity::from_file(held)?;
                            fs::remove_file(target.join("db/librairii.sqlite3"))
                                .map_err(|e| StorageError::io("test unlink preflight file", e))?;
                            if replace_directory {
                                fs::remove_dir(target.join("db")).map_err(|e| {
                                    StorageError::io("test unlink preflight directory", e)
                                })?;
                                fs::create_dir(target.join("db")).map_err(|e| {
                                    StorageError::io("test replace preflight directory", e)
                                })?;
                                let held_directory = candidate
                                    .preflight_directories
                                    .get("db")
                                    .ok_or(StorageError::AdoptionInputChanged)?;
                                let named = candidate.directory.child(OsStr::new("db"))?;
                                assert_ne!(held_directory.identity(), named.identity());
                                assert_eq!(
                                    held_directory.duplicate()?.identity(),
                                    held_directory.identity()
                                );
                            }
                            fs::write(target.join("db/librairii.sqlite3"), [])
                                .map_err(|e| StorageError::io("test recreate preflight file", e))?;
                            let replacement = File::open(target.join("db/librairii.sqlite3"))
                                .map_err(|e| {
                                    StorageError::io("test observe preflight replacement", e)
                                })?;
                            assert_ne!(original, FileIdentity::from_file(&replacement)?);
                            assert_eq!(
                                held.metadata()
                                    .map_err(|e| StorageError::io(
                                        "test inspect unlinked witness",
                                        e
                                    ))?
                                    .nlink(),
                                0
                            );
                            Ok(())
                        },
                        || Ok(()),
                    )
                    .err()
                    .ok_or("unlinked preflight witness was replaced with write authority")?;
                assert!(matches!(
                    failed.storage_error(),
                    StorageError::AdoptionInputChanged
                ));
                let candidate = failed
                    .retained
                    .as_ref()
                    .ok_or("failed copy lost actual owners")?;
                let held = candidate
                    .preflight_files
                    .get("db/librairii.sqlite3")
                    .ok_or("failed copy lost original File")?;
                assert_eq!(held.metadata()?.nlink(), 0);
                assert_eq!(held.metadata()?.len(), 0);
                assert_eq!(fs::metadata(target.join("db/librairii.sqlite3"))?.len(), 0);
                assert_eq!(
                    fs::metadata(target.join(".librairii-library"))?.len(),
                    0,
                    "all-name revalidation must refuse before ANY payload write"
                );
                if replace_directory {
                    let held = candidate
                        .preflight_directories
                        .get("db")
                        .ok_or("failed copy lost original Directory")?;
                    assert_ne!(
                        held.identity(),
                        candidate.directory.child(OsStr::new("db"))?.identity()
                    );
                    assert_eq!(held.duplicate()?.identity(), held.identity());
                }
                assert!(failed.retains_lease());
                let lock = fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(target.join(LIBRARY_LOCK_FILE))?;
                assert!(matches!(lock.try_lock(), Err(fs::TryLockError::WouldBlock)));
                assert_eq!(fs::read(input.root.join("db/librairii.sqlite3"))?, original);
                drop(failed);
                // Releasing fixture resources is not product cleanup.
                assert!(target.join("db/librairii.sqlite3").is_file());
            }

            let target = roots.library().join("copy-unacknowledged");
            let failed = input
                .copy_with_checkpoint(&target, &WorkCancellation::default(), || {
                    Err(StorageError::DirectoryDurabilityUnavailable)
                })
                .err()
                .ok_or("injected final flush must fail")?;
            assert!(matches!(
                failed.storage_error(),
                StorageError::DirectoryDurabilityUnavailable
            ));
            assert_eq!(failed.retained_root(), Some(target.as_path()));
            assert_eq!(failed.attempted_target(), Some(target.as_path()));
            assert!(failed.retains_lease());
            assert_eq!(fs::read(target.join("db/librairii.sqlite3"))?, original);
            assert_eq!(fs::read(input.root.join("db/librairii.sqlite3"))?, original);
            let lock = fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(target.join(LIBRARY_LOCK_FILE))?;
            assert!(matches!(lock.try_lock(), Err(fs::TryLockError::WouldBlock)));
            assert!(!input.root.join(LIBRARY_LOCK_FILE).exists());
            assert!(!target.join("settings.json").exists());
            assert!(!target.join("relocation.json").exists());
            drop(failed);
            lock.try_lock()?;
            assert_eq!(fs::read(target.join("db/librairii.sqlite3"))?, original);
            Ok(())
        }

        #[test]
        fn late_copy_cancel_and_root_substitution_never_activate_or_purge_bytes()
        -> std::result::Result<(), Box<dyn Error>> {
            let roots = TestRoots::new()?;
            let input = input(&roots)?;
            let cancellation = WorkCancellation::default();
            let cancelled = roots.library().join("late-cancel");
            let failed = input
                .copy_with_checkpoint(&cancelled, &cancellation, || {
                    cancellation.cancel();
                    Ok(())
                })
                .err()
                .ok_or("late cancellation must fail")?;
            assert!(matches!(failed.storage_error(), StorageError::Cancelled));
            assert!(failed.retains_lease());
            assert_eq!(
                fs::read(cancelled.join("db/librairii.sqlite3"))?,
                fs::read(input.root.join("db/librairii.sqlite3"))?
            );
            let target = roots.library().join("substituted");
            let original = roots.library().join("owned-retained");
            let substituted = input
                .copy_with_checkpoint(&target, &WorkCancellation::default(), || {
                    fs::rename(&target, &original)
                        .map_err(|e| StorageError::io("inject copied root rename", e))?;
                    fs::create_dir(&target)
                        .map_err(|e| StorageError::io("inject replacement root", e))?;
                    fs::write(target.join("unowned"), b"do not remove")
                        .map_err(|e| StorageError::io("inject unowned file", e))
                })
                .err()
                .ok_or("substitution must fail")?;
            assert!(matches!(
                substituted.storage_error(),
                StorageError::SnapshotChanged
            ));
            assert!(substituted.retains_lease());
            drop(substituted);
            assert_eq!(fs::read(target.join("unowned"))?, b"do not remove");
            assert_eq!(
                fs::read(original.join("db/librairii.sqlite3"))?,
                fs::read(input.root.join("db/librairii.sqlite3"))?
            );
            Ok(())
        }
    }
}
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub use closed::{ClosedLibraryInput, IndependentCopy, IndependentCopyFailure};

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

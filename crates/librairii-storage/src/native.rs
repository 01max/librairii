//! Native profile binding of an actually owned independent library copy.
//!
//! The explicit native route borrows this owner through all SQLite joins. A
//! separate native namespace and retained verified copy supply its authority,
//! never TestRoots, profile DATA, a caller boolean or an in-place legacy lock.
//! Recovery replay and the owned threaded jobs bridge remain separate.

pub(crate) const NATIVE_STATE_FILE: &str = ".librairii-native-state.json";
pub(crate) const NATIVE_STATE_TEMP: &str = ".librairii-native-state.json.tmp-";
pub(crate) fn reserved_control_name(name: &str) -> bool {
    name == NATIVE_STATE_FILE || name.starts_with(NATIVE_STATE_TEMP)
}
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub(crate) fn digest(bytes: &[u8]) -> String {
    use sha2::Digest;
    librairii_core::Checksum::from_bytes(sha2::Sha256::digest(bytes).into()).to_string()
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
mod state {
    use super::{NATIVE_STATE_FILE, NATIVE_STATE_TEMP};
    use crate::{
        Result, StorageError,
        platform::unix::{ConfigTemp, ControlAllocation, Directory, durable_file_flush},
        profile::MAX_PROFILE_BYTES,
    };
    use serde::{Deserialize, Serialize};
    use std::{
        ffi::OsStr,
        io::{Read, Write},
        path::{Component, Path},
    };

    #[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub(super) enum Phase {
        ImportedClosed,
        Active,
        Closed,
    }
    #[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub(super) struct Document {
        pub format: String,
        pub version: u64,
        pub state: Phase,
        pub epoch: u64,
        pub native_profile_root: String,
        pub native_data_root: String,
        pub library_root: String,
        pub source_root: String,
        pub library_id_raw: String,
        pub library_marker_sha256: String,
        pub profile_marker_sha256: String,
        pub source_manifest_sha256: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        pub temporaries: Vec<crate::native_effects::Temporary>,
    }
    impl Document {
        pub fn decode(bytes: &[u8]) -> Result<Self> {
            if bytes.len() > crate::profile::MAX_PROFILE_BYTES {
                return Err(StorageError::RecoveryBindingRequired);
            }
            let document: Self =
                serde_json::from_slice(bytes).map_err(|_| StorageError::RecoveryBindingRequired)?;
            document.validate()?;
            Ok(document)
        }
        pub fn validate(&self) -> Result<()> {
            if self.format != "librairii.native-library"
                || self.version != 1
                || (self.state == Phase::ImportedClosed) != (self.epoch == 0)
                || crate::identity::parse_spelling(&self.library_id_raw).is_err()
            {
                return Err(StorageError::RecoveryBindingRequired);
            }
            if self.state != Phase::Active && !self.temporaries.is_empty() {
                return Err(StorageError::RecoveryBindingRequired);
            }
            if self.temporaries.len() > 64 {
                return Err(StorageError::RecoveryBindingRequired);
            }
            let mut names = std::collections::BTreeSet::new();
            for temporary in &self.temporaries {
                temporary.validate()?;
                if !names.insert((&temporary.destination, &temporary.name)) {
                    return Err(StorageError::RecoveryBindingRequired);
                }
            }
            for path in [
                &self.native_profile_root,
                &self.native_data_root,
                &self.library_root,
                &self.source_root,
            ] {
                if !Path::new(path).is_absolute()
                    || path.contains('\0')
                    || Path::new(path)
                        .components()
                        .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
                {
                    return Err(StorageError::RecoveryBindingRequired);
                }
            }
            for hash in [
                &self.library_marker_sha256,
                &self.profile_marker_sha256,
                &self.source_manifest_sha256,
            ] {
                if hash.len() != 64
                    || !hash
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                {
                    return Err(StorageError::RecoveryBindingRequired);
                }
            }
            Ok(())
        }
        pub fn bytes(&self) -> Result<Vec<u8>> {
            self.validate()?;
            let bytes =
                serde_json::to_vec(self).map_err(|_| StorageError::RecoveryBindingRequired)?;
            if bytes.len() > crate::profile::MAX_PROFILE_BYTES {
                return Err(StorageError::RecoveryBindingRequired);
            }
            Ok(bytes)
        }
    }
    #[derive(Debug)]
    pub(super) struct CreateFailure {
        pub cause: StorageError,
        pub allocation: Option<ControlAllocation>,
    }
    impl std::fmt::Display for CreateFailure {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "native control creation failed: {}", self.cause)
        }
    }
    impl std::error::Error for CreateFailure {}
    #[derive(Debug)]
    pub(super) struct Record {
        pub current: ConfigTemp,
        pub document: Document,
        pub bytes: Vec<u8>,
        // Every old/new actual object survives any failed boundary, including
        // displaced objects that have already lost their private owned name.
        retained: Vec<ConfigTemp>,
        retained_allocations: Vec<ControlAllocation>,
        pub uncertain: bool,
    }
    fn read(file: &mut impl Read) -> Result<Vec<u8>> {
        let mut bytes = Vec::new();
        file.take((MAX_PROFILE_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|e| StorageError::io("read bounded native state", e))?;
        if bytes.len() > crate::profile::MAX_PROFILE_BYTES {
            return Err(StorageError::RecoveryBindingRequired);
        }
        Ok(bytes)
    }
    impl Record {
        pub fn create(
            root: &Directory,
            document: Document,
        ) -> std::result::Result<Self, Box<CreateFailure>> {
            let bytes = document.bytes().map_err(|cause| {
                Box::new(CreateFailure {
                    cause,
                    allocation: None,
                })
            })?;
            let current = root
                .create_control_record(OsStr::new(NATIVE_STATE_FILE))
                .map_err(|failure| {
                    let failure = *failure;
                    Box::new(CreateFailure {
                        cause: failure.cause,
                        allocation: Some(failure.allocation),
                    })
                })?;
            Ok(Self {
                current,
                document,
                bytes,
                retained: Vec::new(),
                retained_allocations: Vec::new(),
                uncertain: true,
            })
        }
        pub fn initialize(&mut self, root: &Directory) -> Result<()> {
            self.current
                .file
                .write_all(&self.bytes)
                .map_err(|e| StorageError::io("write imported native state", e))?;
            durable_file_flush(&self.current.file)?;
            root.flush()?;
            self.uncertain = false;
            let checked = self.revalidate(root);
            if checked.is_err() {
                self.uncertain = true;
            }
            checked
        }
        pub fn open(root: &Directory) -> Result<Self> {
            let mut current = root.read_config(OsStr::new(NATIVE_STATE_FILE))?;
            let bytes = read(&mut current.file)?;
            let document = Document::decode(&bytes)?;
            let record = Self {
                current,
                document,
                bytes,
                retained: Vec::new(),
                retained_allocations: Vec::new(),
                uncertain: false,
            };
            record.revalidate(root)?;
            Ok(record)
        }
        #[cfg(test)]
        pub(super) fn retained_file_for_test(&mut self) -> Result<std::fs::File> {
            self.retained
                .iter()
                .rev()
                .find(|w| w.name != OsStr::new(NATIVE_STATE_FILE))
                .ok_or(StorageError::CompatibilityInvariant)?
                .file
                .try_clone()
                .map_err(|e| StorageError::io("retain displaced record for assertion", e))
        }
        pub fn revalidate(&self, root: &Directory) -> Result<()> {
            if self.uncertain {
                return Err(StorageError::RecoveryBindingRequired);
            }
            use std::os::unix::fs::MetadataExt;
            let expected = self
                .current
                .file
                .metadata()
                .map_err(|e| StorageError::io("inspect native state witness", e))?;
            let mut actual = root.read_config(OsStr::new(NATIVE_STATE_FILE))?;
            let found = actual
                .file
                .metadata()
                .map_err(|e| StorageError::io("inspect current native state", e))?;
            if expected.nlink() != 1
                || found.nlink() != 1
                || expected.dev() != found.dev()
                || expected.ino() != found.ino()
                || read(&mut actual.file)? != self.bytes
            {
                return Err(StorageError::RecoveryBindingRequired);
            }
            Ok(())
        }
        // Used only with an actual retained export allocation under custody.
        // Accept the exact prior, registered or retired form of this one record,
        // and only a currently named control object whose FD we already own.
        pub fn reconcile_temporary(
            &mut self,
            root: &Directory,
            temporary: &crate::native_effects::Temporary,
        ) -> Result<()> {
            if !self.uncertain {
                return self.revalidate(root);
            }
            let mut actual = root.read_config(OsStr::new(NATIVE_STATE_FILE))?;
            let identity = crate::platform::unix::FileIdentity::from_file(&actual.file)?;
            let owned = std::iter::once(&self.current.file)
                .chain(self.retained.iter().map(|r| &r.file))
                .chain(
                    self.retained_allocations
                        .iter()
                        .filter_map(|r| r.file.as_ref()),
                )
                .any(|f| crate::platform::unix::FileIdentity::from_file(f).ok() == Some(identity));
            if !owned {
                return Err(StorageError::RecoveryBindingRequired);
            }
            let bytes = read(&mut actual.file)?;
            let same = |t: &crate::native_effects::Temporary| {
                t.operation == temporary.operation
                    && t.item == temporary.item
                    && t.destination == temporary.destination
                    && t.parent == temporary.parent
                    && t.name == temporary.name
            };
            let mut with = self.document.clone();
            if let Some(index) = with.temporaries.iter().position(same) {
                with.temporaries[index] = temporary.clone();
            } else {
                with.temporaries.push(temporary.clone());
            }
            let mut without = self.document.clone();
            without.temporaries.retain(|t| !same(t));
            let document = if bytes == self.bytes {
                self.document.clone()
            } else if bytes == with.bytes()? {
                with
            } else if bytes == without.bytes()? {
                without
            } else {
                return Err(StorageError::RecoveryBindingRequired);
            };
            durable_file_flush(&actual.file)?;
            root.flush()?;
            if crate::platform::unix::FileIdentity::from_file(&self.current.file)? != identity {
                let previous = std::mem::replace(&mut self.current, actual);
                self.retained.push(previous);
            }
            self.document = document;
            self.bytes = bytes;
            for retained in &self.retained {
                if retained.name != OsStr::new(NATIVE_STATE_FILE) {
                    use std::os::unix::fs::MetadataExt;
                    if retained
                        .file
                        .metadata()
                        .map_err(|e| StorageError::io("observe retained control witness", e))?
                        .nlink()
                        != 0
                    {
                        root.remove_config_temp(retained)?;
                    }
                }
            }
            for retained in &self.retained_allocations {
                root.retire_control_allocation(retained)?;
            }
            root.flush()?;
            self.uncertain = false;
            if let Err(e) = self.revalidate(root) {
                self.uncertain = true;
                return Err(e);
            }
            self.retained.clear();
            self.retained_allocations.clear();
            Ok(())
        }
        pub fn transition_with(
            &mut self,
            root: &Directory,
            next: Document,
            after_replace: impl FnOnce() -> Result<()>,
        ) -> Result<()> {
            self.revalidate(root)?;
            let expected = self
                .current
                .file
                .metadata()
                .map_err(|e| StorageError::io("retain old native state", e))?;
            let bytes = next.bytes()?;
            self.uncertain = true;
            let mut staged = match root.create_control_temp(NATIVE_STATE_TEMP) {
                Ok(staged) => staged,
                Err(failure) => {
                    let failure = *failure;
                    self.retained_allocations.push(failure.allocation);
                    return Err(failure.cause);
                }
            };
            let staged_write = staged
                .file
                .write_all(&bytes)
                .map_err(|e| StorageError::io("write next native state", e))
                .and_then(|()| durable_file_flush(&staged.file));
            if let Err(error) = staged_write {
                self.retained.push(staged);
                return Err(error);
            }
            match root.replace_owned_config(staged, OsStr::new(NATIVE_STATE_FILE), Some(&expected))
            {
                Err(failure) => {
                    let failure = *failure;
                    self.retained.push(failure.temporary);
                    return Err(failure.cause);
                }
                Ok((published, old)) => {
                    let previous = std::mem::replace(&mut self.current, published);
                    self.retained.push(previous);
                    if let Some(displaced) = old {
                        let mut checked = root.read_config(&displaced.name)?;
                        if read(&mut checked.file)? != self.bytes {
                            self.retained.push(displaced);
                            return Err(StorageError::RecoveryBindingRequired);
                        }
                        self.retained.push(displaced);
                    }
                }
            }
            self.document = next;
            self.bytes = bytes;
            after_replace()?;
            root.flush()?;
            // Compare the exact currently named bytes before retiring ONLY the
            // actual displaced control object. Never scan/purge a prefix.
            self.uncertain = false;
            let validation = self.revalidate(root);
            self.uncertain = true;
            validation?;
            for temporary in &self.retained {
                if temporary.name != OsStr::new(NATIVE_STATE_FILE) {
                    let links = temporary
                        .file
                        .metadata()
                        .map_err(|e| StorageError::io("inspect displaced native state", e))?;
                    use std::os::unix::fs::MetadataExt;
                    if links.nlink() != 0 {
                        root.remove_config_temp(temporary)?;
                    }
                }
            }
            root.flush()?;
            self.uncertain = false;
            let checked = self.revalidate(root);
            if checked.is_err() {
                self.uncertain = true;
            } else {
                // Durable cleanup and final validation acknowledge this update.
                // Only uncertain transitions need the displaced witnesses.
                self.retained.clear();
                self.retained_allocations.clear();
            }
            checked
        }
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        use librairii_testkit::TestRoots;
        use std::error::Error;
        fn document(root: &Path) -> Document {
            Document {
                format: "librairii.native-library".into(),
                version: 1,
                state: Phase::ImportedClosed,
                epoch: 0,
                native_profile_root: "/explicit/profile".into(),
                native_data_root: "/explicit/data".into(),
                library_root: root.to_str().unwrap_or("").into(),
                source_root: "/explicit/source".into(),
                library_id_raw: "A0000000-0000-4000-8000-000000000001".into(),
                library_marker_sha256: "a".repeat(64),
                profile_marker_sha256: "b".repeat(64),
                source_manifest_sha256: "c".repeat(64),
                temporaries: Vec::new(),
            }
        }
        #[test]
        fn native_state_private_codec_rejects_duplicates_unknowns_types_limits_and_epoch_mismatch()
        -> std::result::Result<(), Box<dyn Error>> {
            let bytes = document(Path::new("/explicit/data/copy")).bytes()?;
            let decoded = Document::decode(&bytes)?;
            assert_eq!(
                decoded.library_id_raw,
                "A0000000-0000-4000-8000-000000000001"
            );
            let text = std::str::from_utf8(&bytes)?;
            for bad in [
                format!("{{\"version\":1,{}", &text[1..]),
                format!("{text} null"),
                text.replace("\"version\":1", "\"version\":null"),
                text.replace("\"version\":1", "\"version\":1.0"),
                text.replace("\"version\":1", "\"version\":2"),
                text.replace("\"epoch\":0", "\"epoch\":-1"),
                text.replace("\"epoch\":0", "\"epoch\":18446744073709551616"),
                text.replace("imported_closed", "active"),
                text.replace("imported_closed", "unknown"),
                text.replace(
                    "A0000000-0000-4000-8000-000000000001",
                    " A0000000-0000-4000-8000-000000000001 ",
                ),
                text.replacen("\"format\":", "\"extra\":true,\"format\":", 1),
                text.replace(&"a".repeat(64), &"A".repeat(64)),
                " ".repeat(MAX_PROFILE_BYTES + 1),
            ] {
                assert!(
                    matches!(
                        Document::decode(bad.as_bytes()),
                        Err(StorageError::RecoveryBindingRequired)
                    ),
                    "{bad}"
                );
            }
            for raw in [
                "xA0000000-0000-4000-8000-000000000001y",
                "\0A0000000-0000-4000-8000-000000000001\0",
            ] {
                let mut original = document(Path::new("/explicit/data/copy"));
                original.library_id_raw = raw.into();
                let encoded = original.bytes()?;
                let decoded = Document::decode(&encoded)?;
                assert_eq!(decoded.library_id_raw, raw);
                assert_eq!(decoded.bytes()?, encoded);
            }
            Ok(())
        }
        #[test]
        fn native_state_atomic_response_loss_retains_old_new_records_and_never_purges_prefix()
        -> std::result::Result<(), Box<dyn Error>> {
            use std::io::{Seek, SeekFrom};
            let roots = TestRoots::new()?;
            let root = Directory::open(roots.library())?;
            let _lease =
                crate::platform::lock::LibraryLease::create_fresh_in(roots.library(), &root)?;
            let mut record = Record::create(&root, document(roots.library()))?;
            record.initialize(&root)?;
            let before = record.bytes.clone();
            let mut next = record.document.clone();
            next.state = Phase::Active;
            next.epoch = 1;
            let result = record.transition_with(&root, next, || {
                Err(StorageError::io(
                    "inject state rename acknowledgement loss",
                    std::io::Error::other("after actual exchange"),
                ))
            });
            assert!(result.is_err());
            assert!(record.uncertain);
            assert!(record.revalidate(&root).is_err());
            assert_eq!(Record::open(&root)?.document.state, Phase::Active);
            let old = record
                .retained
                .iter_mut()
                .find(|w| w.name != OsStr::new(NATIVE_STATE_FILE))
                .ok_or("displaced witness")?;
            old.file.seek(SeekFrom::Start(0))?;
            assert_eq!(read(&mut old.file)?, before);
            let path = roots.library().join(&old.name);
            assert!(path.exists());
            drop(record);
            assert!(path.exists());
            assert_eq!(Record::open(&root)?.document.state, Phase::Active);
            Ok(())
        }
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
mod owned {
    use super::{
        NATIVE_STATE_FILE, NATIVE_STATE_TEMP, digest,
        state::{Document, Phase, Record},
    };
    use crate::{
        LibraryIdentity, LibraryLayout, Result, StorageError,
        adoption::IndependentCopy,
        database::WorkCancellation,
        platform::{
            lock::LibraryLease,
            unix::{ControlAllocation, Directory, EntryName},
        },
        profile::{FrozenProfile, NativeProfileStore, ProfileBinding, ProfileLocations},
    };
    use std::{
        ffi::OsStr,
        fs::{self, File},
        io::Read,
        os::unix::fs::MetadataExt,
        path::{Path, PathBuf},
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, Ordering},
        },
    };

    #[derive(Debug)]
    enum RootOwner {
        Copy(Box<IndependentCopy>),
        Existing(Box<ExistingRoot>),
    }
    #[derive(Debug)]
    struct ExistingRoot {
        parent: Directory,
        name: EntryName,
        directories: Vec<(&'static str, Directory)>,
        // Last: this existing root lease outlives every retained parent/child.
        lease: LibraryLease,
    }
    impl ExistingRoot {
        fn revalidate(&self) -> Result<()> {
            self.lease.revalidate()?;
            let root = self.lease.root();
            self.parent.matches_metadata(
                &fs::symlink_metadata(root.parent().ok_or(StorageError::RootNotAbsolute)?)
                    .map_err(|e| StorageError::io("revalidate existing native parent", e))?,
            )?;
            self.parent.child(self.name.as_os_str())?.matches_metadata(
                &fs::symlink_metadata(root)
                    .map_err(|e| StorageError::io("revalidate existing native child", e))?,
            )?;
            for (name, directory) in &self.directories {
                directory.matches_metadata(
                    &fs::symlink_metadata(root.join(name))
                        .map_err(|e| StorageError::io("revalidate existing managed child", e))?,
                )?;
            }
            Ok(())
        }
    }
    impl RootOwner {
        fn revalidate(&self) -> Result<()> {
            match self {
                Self::Copy(copy) => copy.revalidate_root_lease(),
                Self::Existing(root) => root.revalidate(),
            }
        }
        fn initial(&self, cancellation: &WorkCancellation) -> Result<()> {
            match self {
                Self::Copy(copy) => copy.revalidate(cancellation),
                _ => self.revalidate(),
            }
        }
    }

    struct NativeDirectories {
        layout: LibraryLayout,
        root: Directory,
        database: Directory,
    }

    /// A sealed native owner retaining profile/root leases and actual record,
    /// marker and directory objects. A parsed record never constructs authority.
    ///
    /// ```compile_fail
    /// use librairii_storage::{database::Database, native::NativeLibrary};
    /// fn native_is_not_a_development_override(library: &NativeLibrary<'_>) {
    ///     let _ = Database::open_isolated(library);
    /// }
    /// ```
    #[derive(Debug)]
    pub struct NativeLibrary<'profile> {
        layout: LibraryLayout,
        root: Directory,
        database: Directory,
        marker: File,
        identity: LibraryIdentity,
        record: Mutex<Record>,
        managed: Mutex<Option<Vec<(&'static str, Directory)>>>,
        // Serialize bounded binding observations made by concurrent database
        // and file workers; a second observer must not resemble a profile update.
        revalidation: Mutex<()>,
        runtime_active: AtomicBool,
        recovery_pending: AtomicBool,
        closed_success: AtomicBool,
        pub(crate) custody_active: AtomicBool,
        fenced: Arc<AtomicBool>,
        finalized: bool,
        owner: RootOwner,
        // On failure this reservation physically retains the profile state;
        // all callbacks/publications remain excluded without holding a mutex.
        frozen: Option<FrozenProfile<'profile>>,
        binding: Option<ProfileBinding<'profile>>,
    }
    #[derive(Debug)]
    pub struct RecoveryLibrary<'profile> {
        library: NativeLibrary<'profile>,
    }
    impl<'profile> RecoveryLibrary<'profile> {
        pub fn layout(&self) -> &LibraryLayout {
            self.library.layout()
        }
        pub fn open(&self) -> Result<crate::database::Database<'_>> {
            let database = crate::database::Database::open_native(&self.library)?;
            if let Err(error) = database.recover_operation_startup() {
                self.library.fence();
                return Err(error);
            }
            Ok(database)
        }
        pub fn finish(self) -> std::result::Result<(), Box<NativeCloseFailure<'profile>>> {
            self.library.finish()
        }
    }
    impl<'profile> NativeLibrary<'profile> {
        pub fn bind_independent(
            copy: IndependentCopy,
            store: &'profile NativeProfileStore,
            locations: &ProfileLocations,
            cancellation: &WorkCancellation,
        ) -> std::result::Result<Self, Box<NativeBindingFailure<'profile>>> {
            Self::bind_with_profile(copy, store, locations, None, cancellation)
        }
        pub(crate) fn bind_rebound(
            copy: IndependentCopy,
            store: &'profile NativeProfileStore,
            locations: &ProfileLocations,
            foreign: &crate::profile::Profile,
            cancellation: &WorkCancellation,
        ) -> std::result::Result<Self, Box<NativeBindingFailure<'profile>>> {
            Self::bind_with_profile(copy, store, locations, Some(foreign), cancellation)
        }
        fn bind_with_profile(
            mut copy: IndependentCopy,
            store: &'profile NativeProfileStore,
            locations: &ProfileLocations,
            foreign: Option<&crate::profile::Profile>,
            cancellation: &WorkCancellation,
        ) -> std::result::Result<Self, Box<NativeBindingFailure<'profile>>> {
            let mut record = None;
            let mut allocation = None;
            let prepare = (|| {
                let admission = store.begin_binding()?;
                if cancellation.is_cancelled() {
                    return Err(StorageError::Cancelled);
                }
                copy.revalidate(cancellation)?;
                copy.require_no_recovery_input()?;
                // Foreign wire data can select preserved settings only AFTER
                // actual independent-copy ownership and exact raw ID proof.
                // A recovering profile/journal is not replay/cleanup authority.
                let mut proposed = foreign.unwrap_or(copy.source_profile().profile()).clone();
                if !proposed.complete || proposed.recovering {
                    return Err(StorageError::RecoveryBindingRequired);
                }
                copy.identity().require_spelling(&proposed.library_id)?;
                proposed.root = exact_path(copy.root())?;
                require_paths(copy.root(), copy.source_root(), store, locations)?;
                let layout = LibraryLayout::probe(copy.root())?;
                let root = copy.retain_root()?;
                let database = root.child(OsStr::new("db"))?;
                let marker = root.open_regular(
                    OsStr::new(crate::identity::IDENTITY_FILE),
                    rustix::fs::OFlags::RDONLY,
                )?;
                let document = Document {
                    format: "librairii.native-library".into(),
                    version: 1,
                    state: Phase::ImportedClosed,
                    epoch: 0,
                    native_profile_root: exact_path(
                        &fs::canonicalize(store.directory())
                            .map_err(|e| StorageError::io("resolve native profile root", e))?,
                    )?,
                    native_data_root: exact_path(
                        &fs::canonicalize(&locations.data)
                            .map_err(|e| StorageError::io("resolve native data root", e))?,
                    )?,
                    library_root: exact_path(copy.root())?,
                    source_root: exact_path(copy.source_root())?,
                    library_id_raw: copy.identity().spelling().into(),
                    library_marker_sha256: digest(copy.identity().original_bytes()),
                    profile_marker_sha256: store.marker_digest()?,
                    source_manifest_sha256: copy.manifest_digest(),
                    temporaries: Vec::new(),
                };
                record = Some(match Record::create(&root, document) {
                    Ok(record) => record,
                    Err(failure) => {
                        let failure = *failure;
                        allocation = failure.allocation;
                        return Err(failure.cause);
                    }
                });
                let actual = record
                    .as_mut()
                    .ok_or(StorageError::CompatibilityInvariant)?;
                actual.initialize(&root)?;
                copy.attach_native_record(
                    actual
                        .current
                        .file
                        .try_clone()
                        .map_err(|e| StorageError::io("retain imported record in copy", e))?,
                    actual.bytes.clone(),
                )?;
                if cancellation.is_cancelled() {
                    return Err(StorageError::Cancelled);
                }
                let binding = admission.publish(&proposed)?;
                Ok((layout, root, database, marker, binding))
            })();
            match prepare {
                Ok((layout, root, database, marker, binding)) => {
                    let Some(record) = record else {
                        return Err(Box::new(NativeBindingFailure {
                            cause: StorageError::CompatibilityInvariant,
                            copy,
                            store,
                            record: None,
                            allocation,
                        }));
                    };
                    let identity = copy.identity().clone();
                    Ok(Self::new(
                        NativeDirectories {
                            layout,
                            root,
                            database,
                        },
                        marker,
                        identity,
                        record,
                        RootOwner::Copy(Box::new(copy)),
                        binding,
                    ))
                }
                Err(cause) => Err(Box::new(NativeBindingFailure {
                    cause,
                    copy,
                    store,
                    record,
                    allocation,
                })),
            }
        }
        fn new(
            directories: NativeDirectories,
            marker: File,
            identity: LibraryIdentity,
            record: Record,
            owner: RootOwner,
            binding: ProfileBinding<'profile>,
        ) -> Self {
            let NativeDirectories {
                layout,
                root,
                database,
            } = directories;
            let fenced = binding.fence_signal();
            Self {
                layout,
                root,
                database,
                marker,
                identity,
                record: Mutex::new(record),
                managed: Mutex::new(None),
                revalidation: Mutex::new(()),
                runtime_active: AtomicBool::new(false),
                recovery_pending: AtomicBool::new(false),
                closed_success: AtomicBool::new(false),
                custody_active: AtomicBool::new(false),
                fenced,
                finalized: false,
                owner,
                frozen: None,
                binding: Some(binding),
            }
        }
        /// Reopen ONLY the root already named by the durable complete native
        /// profile. Imported orphan records never publish or retarget a profile.
        /// Active/corrupt/foreign control or recovery artifacts are refused.
        /// This route never rechecks historical copy DB hashes after native use.
        pub fn reopen(
            store: &'profile NativeProfileStore,
            locations: &ProfileLocations,
            cancellation: &WorkCancellation,
        ) -> Result<Self> {
            Self::reopen_mode(store, locations, cancellation, false)
        }
        pub fn recover_interrupted(
            store: &'profile NativeProfileStore,
            locations: &ProfileLocations,
            cancellation: &WorkCancellation,
        ) -> Result<RecoveryLibrary<'profile>> {
            Ok(RecoveryLibrary {
                library: Self::reopen_mode(store, locations, cancellation, true)?,
            })
        }
        fn reopen_mode(
            store: &'profile NativeProfileStore,
            locations: &ProfileLocations,
            cancellation: &WorkCancellation,
            recovery: bool,
        ) -> Result<Self> {
            let profile = store
                .load()?
                .ok_or(StorageError::RecoveryBindingRequired)?
                .profile()
                .clone();
            if !profile.complete || profile.recovering {
                return Err(StorageError::RecoveryBindingRequired);
            }
            let admission = store.begin_binding()?;
            if cancellation.is_cancelled() {
                return Err(StorageError::Cancelled);
            }
            let path = PathBuf::from(&profile.root);
            let actual_path = fs::canonicalize(&path)
                .map_err(|e| StorageError::io("resolve closed native root", e))?;
            if exact_path(&actual_path)? != profile.root {
                return Err(StorageError::RecoveryBindingRequired);
            }
            let layout = LibraryLayout::probe(&path)?;
            let root = Directory::open(&path)?;
            let lease = LibraryLease::acquire_existing_in(&path, &root)?;
            let record = Record::open(&root)?;
            if record.document.state == Phase::Active && !recovery {
                return Err(StorageError::RecoveryBindingRequired);
            }
            let d = &record.document;
            if d.library_root != profile.root
                || d.library_id_raw != profile.library_id
                || d.native_profile_root
                    != exact_path(
                        &fs::canonicalize(store.directory())
                            .map_err(|e| StorageError::io("resolve existing native profile", e))?,
                    )?
                || d.native_data_root
                    != exact_path(
                        &fs::canonicalize(&locations.data)
                            .map_err(|e| StorageError::io("resolve existing native data", e))?,
                    )?
                || d.profile_marker_sha256 != store.marker_digest()?
            {
                return Err(StorageError::RecoveryBindingRequired);
            }
            require_paths(&path, Path::new(&d.source_root), store, locations)?;
            let mut marker = root.open_regular(
                OsStr::new(crate::identity::IDENTITY_FILE),
                rustix::fs::OFlags::RDONLY,
            )?;
            let identity = LibraryIdentity::from_bytes(read_marker(&mut marker)?)?;
            identity.require_spelling(&profile.library_id)?;
            if digest(identity.original_bytes()) != d.library_marker_sha256 {
                return Err(StorageError::RecoveryBindingRequired);
            }
            require_root_entries(&root)?;
            let database = root.child(OsStr::new("db"))?;
            let mut directories = Vec::new();
            for name in ["db", "archives", "catalog", "staging", "trash", "logs"] {
                match root.child(OsStr::new(name)) {
                    Ok(directory) => directories.push((name, directory)),
                    Err(StorageError::Io { source, .. })
                        if source.kind() == std::io::ErrorKind::NotFound
                            && (record.document.state == Phase::ImportedClosed
                                || (recovery && name != "db")) => {}
                    Err(error) => return Err(error),
                }
            }
            let parent = Directory::open(path.parent().ok_or(StorageError::RootNotAbsolute)?)?;
            let name = EntryName::parse(path.file_name().ok_or(StorageError::InvalidManagedPath)?)?;
            if recovery {
                for name in [
                    "librairii.sqlite3-wal",
                    "librairii.sqlite3-shm",
                    "librairii.sqlite3-journal",
                ] {
                    database.read_regular(OsStr::new(name))?;
                }
            } else {
                require_no_sidecars(&database)?;
            }
            let scratch = tempfile::tempdir()
                .map_err(|e| StorageError::io("create existing native DB probe", e))?;
            crate::backup::probe_owned(&path.join("db/librairii.sqlite3"), scratch.path())?;
            lease.revalidate()?;
            record.revalidate(&root)?;
            let binding = admission.publish(&profile)?;
            let owner = RootOwner::Existing(Box::new(ExistingRoot {
                parent,
                name,
                directories,
                lease,
            }));
            owner.revalidate()?;
            let library = Self::new(
                NativeDirectories {
                    layout,
                    root,
                    database,
                },
                marker,
                identity,
                record,
                owner,
                binding,
            );
            library.recovery_pending.store(recovery, Ordering::Release);
            Ok(library)
        }
        pub(crate) fn registered_temporaries(
            &self,
        ) -> Result<Vec<crate::native_effects::Temporary>> {
            let record = self
                .record
                .lock()
                .map_err(|_| StorageError::RuntimeFenced)?;
            record.revalidate(&self.root)?;
            Ok(record.document.temporaries.clone())
        }
        pub(crate) fn register_temporary(
            &self,
            temporary: crate::native_effects::Temporary,
        ) -> Result<()> {
            temporary.validate()?;
            self.revalidate_database_ownership()?;
            let mut record = self
                .record
                .lock()
                .map_err(|_| StorageError::RuntimeFenced)?;
            record.revalidate(&self.root)?;
            if record.document.state != Phase::Active
                || !self.runtime_active.load(Ordering::Acquire)
            {
                return Err(StorageError::IntakeClosed);
            }
            if record.document.temporaries.contains(&temporary) {
                return Ok(());
            }
            if let Some(index) = record.document.temporaries.iter().position(|t| {
                t.operation == temporary.operation
                    && t.item == temporary.item
                    && t.destination == temporary.destination
                    && t.parent == temporary.parent
                    && t.name == temporary.name
            }) {
                let prior = &record.document.temporaries[index];
                if prior.scope_sha256 != temporary.scope_sha256
                    || prior.directory.is_some()
                    || prior.payload.is_some()
                {
                    return Err(StorageError::SnapshotChanged);
                }
                let mut next = record.document.clone();
                next.temporaries[index] = temporary;
                if next.bytes().is_err() {
                    return Err(StorageError::QueueFull);
                }
                let result = record.transition_with(&self.root, next, || Ok(()));
                if result.is_err() {
                    self.fence();
                }
                return result;
            }
            if record.document.temporaries.len() >= 64 {
                return Err(StorageError::QueueFull);
            }
            let mut next = record.document.clone();
            next.temporaries.push(temporary);
            if next.bytes().is_err() {
                return Err(StorageError::QueueFull);
            }
            let result = record.transition_with(&self.root, next, || Ok(()));
            if result.is_err() {
                self.fence();
            }
            result
        }
        #[cfg(test)]
        pub(crate) fn lose_temporary_acknowledgement_for_test(&self) -> Result<()> {
            let mut record = self
                .record
                .lock()
                .map_err(|_| StorageError::RuntimeFenced)?;
            if record.document.state != Phase::Active || record.document.temporaries.is_empty() {
                return Err(StorageError::SnapshotChanged);
            }
            let next = record.document.clone();
            let result = record.transition_with(&self.root, next, || {
                Err(StorageError::DirectoryDurabilityUnavailable)
            });
            self.fence();
            result
        }
        #[cfg(test)]
        pub(crate) fn lose_forget_acknowledgement_for_test(
            &self,
            temporary: &crate::native_effects::Temporary,
        ) -> Result<()> {
            let mut record = self
                .record
                .lock()
                .map_err(|_| StorageError::RuntimeFenced)?;
            let mut next = record.document.clone();
            next.temporaries.retain(|t| t != temporary);
            let result = record.transition_with(&self.root, next, || {
                Err(StorageError::DirectoryDurabilityUnavailable)
            });
            self.fence();
            result
        }
        pub(crate) fn reconcile_export_registration(
            &self,
            allocation: &crate::platform::unix::ExportAllocation<'_, '_>,
            temporary: &crate::native_effects::Temporary,
        ) -> Result<()> {
            if !allocation.matches_registration(temporary) {
                return Err(StorageError::SnapshotChanged);
            }
            temporary.validate()?;
            self.revalidate_database_ownership()?;
            let mut record = self
                .record
                .lock()
                .map_err(|_| StorageError::RuntimeFenced)?;
            if record.document.state != Phase::Active
                || !self.runtime_active.load(Ordering::Acquire)
            {
                return Err(StorageError::IntakeClosed);
            }
            record.reconcile_temporary(&self.root, temporary)
        }
        pub(crate) fn forget_temporary(
            &self,
            temporary: &crate::native_effects::Temporary,
        ) -> Result<()> {
            self.revalidate_database_ownership()?;
            let mut record = self
                .record
                .lock()
                .map_err(|_| StorageError::RuntimeFenced)?;
            record.revalidate(&self.root)?;
            let mut next = record.document.clone();
            next.temporaries.retain(|t| t != temporary);
            let result = record.transition_with(&self.root, next, || Ok(()));
            if result.is_err() {
                self.fence();
            }
            result
        }
        pub fn layout(&self) -> &LibraryLayout {
            &self.layout
        }
        pub fn identity(&self) -> &LibraryIdentity {
            &self.identity
        }
        fn binding(&self) -> Result<&ProfileBinding<'profile>> {
            self.binding.as_ref().ok_or(StorageError::RuntimeFenced)
        }
        pub fn revalidate(&self, cancellation: &WorkCancellation) -> Result<()> {
            let _observation = self
                .revalidation
                .lock()
                .map_err(|_| StorageError::RuntimeFenced)?;
            self.binding()?.revalidate()?;
            let result = (|| {
                self.revalidate_database_ownership()?;
                self.record
                    .lock()
                    .map_err(|_| StorageError::RuntimeFenced)?
                    .revalidate(&self.root)?;
                let managed = self
                    .managed
                    .lock()
                    .map_err(|_| StorageError::RuntimeFenced)?;
                match managed.as_ref() {
                    None => self.owner.initial(cancellation),
                    Some(directories) => {
                        for (name, directory) in directories {
                            if cancellation.is_cancelled() {
                                return Err(StorageError::Cancelled);
                            }
                            directory.matches_metadata(
                                &fs::symlink_metadata(self.layout.root().join(name)).map_err(
                                    |e| StorageError::io("revalidate original native directory", e),
                                )?,
                            )?;
                        }
                        Ok(())
                    }
                }
            })();
            match result {
                Ok(()) => Ok(()),
                Err(StorageError::Cancelled) => Err(StorageError::Cancelled),
                Err(error) => {
                    self.fence();
                    Err(error)
                }
            }
        }
        pub(crate) fn effect_fence(&self) -> Arc<AtomicBool> {
            self.fenced.clone()
        }
        pub(crate) fn fence(&self) {
            self.fenced.store(true, Ordering::Release);
        }
        pub(crate) fn retain_active_files_root(&self) -> Result<Directory> {
            if !self.runtime_active.load(Ordering::Acquire) {
                return Err(StorageError::IntakeClosed);
            }
            self.revalidate(&WorkCancellation::default())?;
            self.root.duplicate()
        }
        pub(crate) fn revalidate_database_ownership(&self) -> Result<()> {
            self.owner.revalidate()?;
            self.root.matches_metadata(
                &fs::symlink_metadata(self.layout.root())
                    .map_err(|e| StorageError::io("inspect retained native root", e))?,
            )?;
            let mut named = self.root.open_regular(
                OsStr::new(crate::identity::IDENTITY_FILE),
                rustix::fs::OFlags::RDONLY,
            )?;
            let prior = self
                .marker
                .metadata()
                .map_err(|e| StorageError::io("inspect retained native marker", e))?;
            let now = named
                .metadata()
                .map_err(|e| StorageError::io("inspect current native marker", e))?;
            if prior.nlink() != 1
                || now.nlink() != 1
                || prior.dev() != now.dev()
                || prior.ino() != now.ino()
                || read_marker(&mut named)? != self.identity.original_bytes()
            {
                return Err(StorageError::IdentityMismatch);
            }
            self.database.matches_metadata(
                &fs::symlink_metadata(self.layout.root().join("db"))
                    .map_err(|e| StorageError::io("revalidate native DB directory", e))?,
            )
        }
        pub(crate) fn claim_runtime(&self) -> Result<()> {
            self.claim_runtime_with(|| Ok(()))
        }
        fn claim_runtime_with(
            &self,
            after_active_replace: impl FnOnce() -> Result<()>,
        ) -> Result<()> {
            self.runtime_active
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .map_err(|_| StorageError::RuntimeInUse)?;
            let prior_closed = self.closed_success.swap(false, Ordering::AcqRel);
            let result = (|| {
                self.binding()?.revalidate()?;
                self.revalidate_database_ownership()?;
                require_root_entries(&self.root)?;
                let recovery = self.recovery_pending.swap(false, Ordering::AcqRel);
                if !recovery {
                    require_no_sidecars(&self.database)?;
                }
                let mut managed = self
                    .managed
                    .lock()
                    .map_err(|_| StorageError::RuntimeFenced)?;
                // Every new runtime claim must freshly validate the retained
                // state, including reuse after a successful DB close. Managed
                // directory witnesses do not prove durable Active bytes.
                let mut record = self
                    .record
                    .lock()
                    .map_err(|_| StorageError::RuntimeFenced)?;
                record.revalidate(&self.root)?;
                if let Some(directories) = managed.as_ref() {
                    if record.document.state != Phase::Active {
                        return Err(StorageError::RecoveryBindingRequired);
                    }
                    for (name, directory) in directories {
                        directory.matches_metadata(
                            &fs::symlink_metadata(self.layout.root().join(name)).map_err(|e| {
                                StorageError::io("revalidate native runtime directory", e)
                            })?,
                        )?;
                    }
                    return Ok(());
                }
                self.owner.initial(&WorkCancellation::default())?;
                let mut next = record.document.clone();
                if next.state == Phase::Active && !recovery {
                    return Err(StorageError::RecoveryBindingRequired);
                }
                next.epoch = next
                    .epoch
                    .checked_add(1)
                    .ok_or(StorageError::RecoveryBindingRequired)?;
                next.state = Phase::Active;
                // This durable ACK is BEFORE managed mkdir and SQLite open,
                // including schema migration/initialization and sidecar writes.
                record.transition_with(&self.root, next, after_active_replace)?;
                require_root_entries(&self.root)?;
                *managed = Some(Vec::new());
                let directories = managed.as_mut().ok_or(StorageError::RuntimeFenced)?;
                for name in ["db", "archives", "catalog", "staging", "trash", "logs"] {
                    let directory = match self.root.child(OsStr::new(name)) {
                        Ok(d) => d,
                        Err(StorageError::Io { source, .. })
                            if source.kind() == std::io::ErrorKind::NotFound =>
                        {
                            self.root.create_child(OsStr::new(name))?
                        }
                        Err(e) => return Err(e),
                    };
                    directories.push((name, directory));
                }
                for (_, directory) in directories {
                    directory.flush()?;
                }
                self.root.flush()?;
                self.revalidate_database_ownership()
            })();
            if matches!(result, Err(StorageError::ProfileBusy)) {
                // The sole Busy boundary precedes record/mkdir/SQLite effects.
                self.closed_success.store(prior_closed, Ordering::Release);
                self.runtime_active.store(false, Ordering::Release);
            } else if result.is_err() {
                self.fence();
            }
            result
        }
        pub(crate) fn release_runtime(&self, fully_opened: bool) {
            // Called ONLY by Database after every actual reader/writer join,
            // checkpoint and owned Connection close has succeeded.
            self.closed_success.store(fully_opened, Ordering::Release);
            if !fully_opened {
                self.fence();
            }
            self.runtime_active.store(false, Ordering::Release);
        }
        /// Consume only after all actual DB/file borrows have drained. Failure
        /// returns the library and real frozen namespace/control witnesses.
        /// Busy from an already-admitted profile operation returns an unfenced
        /// owner for explicit retry after that operation actually settles.
        ///
        /// ```compile_fail
        /// use librairii_storage::{database::Database, native::NativeLibrary};
        /// fn close_before_join(library: NativeLibrary<'_>) {
        ///     let db = Database::open_native(&library);
        ///     let _ = library.finish();
        ///     drop(db);
        /// }
        /// ```
        pub fn finish(self) -> std::result::Result<(), Box<NativeCloseFailure<'profile>>> {
            self.finish_with(|| Ok(()))
        }
        fn finish_with(
            mut self,
            after_closed_replace: impl FnOnce() -> Result<()>,
        ) -> std::result::Result<(), Box<NativeCloseFailure<'profile>>> {
            if self.frozen.is_none() {
                let freeze = self.binding().and_then(ProfileBinding::freeze);
                match freeze {
                    Ok(frozen) => self.frozen = Some(frozen),
                    Err(StorageError::ProfileBusy) => {
                        return Err(Box::new(NativeCloseFailure {
                            cause: StorageError::ProfileBusy,
                            library: self,
                        }));
                    }
                    Err(cause) => {
                        self.fence();
                        return Err(Box::new(NativeCloseFailure {
                            cause,
                            library: self,
                        }));
                    }
                }
            }
            let result = self.finish_closed(after_closed_replace);
            if let Err(cause) = result {
                self.fence();
                return Err(Box::new(NativeCloseFailure {
                    cause,
                    library: self,
                }));
            }
            let frozen = self.frozen.take();
            let binding = self.binding.take();
            self.finalized = true;
            drop(self); // All actual library/root/record resources + OS lease.
            if let (Some(frozen), Some(binding)) = (frozen, binding) {
                frozen.release(binding);
            }
            Ok(())
        }
        fn finish_closed(
            &mut self,
            after_closed_replace: impl FnOnce() -> Result<()>,
        ) -> Result<()> {
            if self.runtime_active.load(Ordering::Acquire)
                || !self.closed_success.load(Ordering::Acquire)
                || self.custody_active.load(Ordering::Acquire)
                || self.fenced.load(Ordering::Acquire)
            {
                return Err(StorageError::RuntimeFenced);
            }
            self.frozen
                .as_ref()
                .ok_or(StorageError::RuntimeFenced)?
                .revalidate()?;
            self.revalidate_database_ownership()?;
            let managed = self
                .managed
                .lock()
                .map_err(|_| StorageError::RuntimeFenced)?;
            let directories = managed.as_ref().ok_or(StorageError::RuntimeFenced)?;
            for (name, directory) in directories {
                directory.matches_metadata(
                    &fs::symlink_metadata(self.layout.root().join(name)).map_err(|e| {
                        StorageError::io("verify closed native directory ownership", e)
                    })?,
                )?;
            }
            drop(managed);
            require_root_entries(&self.root)?;
            require_no_sidecars(&self.database)?;
            // Durable removals retain product data in their bounded namespace.
            // Other trash owners remain unsettled and cannot certify closure.
            let trash = self.root.child(OsStr::new("trash"))?;
            for entry in trash.entries(1, &WorkCancellation::default())? {
                if entry != OsStr::new("removals") {
                    return Err(StorageError::CustodyRecoveryRequired);
                }
                trash.child(&entry)?;
            }
            // Trash is retained product data after a committed removal.
            if !self
                .root
                .child(OsStr::new("staging"))?
                .entries(0, &WorkCancellation::default())?
                .is_empty()
            {
                return Err(StorageError::CustodyRecoveryRequired);
            }
            let mut record = self
                .record
                .lock()
                .map_err(|_| StorageError::RuntimeFenced)?;
            record.revalidate(&self.root)?;
            if record.document.state != Phase::Active || !record.document.temporaries.is_empty() {
                return Err(StorageError::RecoveryBindingRequired);
            }
            // EVERY non-control settlement predicate is already true before
            // Closed can reach disk. A late ACK failure preserves both truthful
            // possible states and never releases this process's exclusion.
            let mut next = record.document.clone();
            next.state = Phase::Closed;
            record.transition_with(&self.root, next, after_closed_replace)?;
            require_root_entries(&self.root)?;
            self.revalidate_database_ownership()?;
            self.frozen
                .as_ref()
                .ok_or(StorageError::RuntimeFenced)?
                .revalidate()
        }
    }
    impl Drop for NativeLibrary<'_> {
        fn drop(&mut self) {
            if !self.finalized {
                self.fence();
            }
        }
    }
    #[derive(Debug)]
    pub struct NativeCloseFailure<'profile> {
        cause: StorageError,
        library: NativeLibrary<'profile>,
    }
    impl<'profile> NativeCloseFailure<'profile> {
        pub fn storage_error(&self) -> &StorageError {
            &self.cause
        }
        pub fn retained_library(&self) -> &NativeLibrary<'profile> {
            &self.library
        }
        pub fn into_library(self) -> NativeLibrary<'profile> {
            self.library
        }
    }
    impl std::fmt::Display for NativeCloseFailure<'_> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "native closure failed: {}", self.cause)
        }
    }
    impl std::error::Error for NativeCloseFailure<'_> {}
    #[derive(Debug)]
    pub struct NativeBindingFailure<'profile> {
        cause: StorageError,
        copy: IndependentCopy,
        store: &'profile NativeProfileStore,
        record: Option<Record>,
        allocation: Option<ControlAllocation>,
    }
    impl NativeBindingFailure<'_> {
        pub fn storage_error(&self) -> &StorageError {
            &self.cause
        }
        pub fn retained_copy(&self) -> &IndependentCopy {
            &self.copy
        }
        pub fn retained_profile(&self) -> &NativeProfileStore {
            self.store
        }
        pub fn retains_native_record(&self) -> bool {
            self.record.is_some() || self.allocation.as_ref().is_some_and(|a| a.file.is_some())
        }
    }
    impl std::fmt::Display for NativeBindingFailure<'_> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "native profile binding failed: {}", self.cause)
        }
    }
    impl std::error::Error for NativeBindingFailure<'_> {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            Some(&self.cause)
        }
    }
    fn exact_path(path: &Path) -> Result<String> {
        if !path.is_absolute() {
            return Err(StorageError::RootNotAbsolute);
        }
        path.to_str()
            .map(str::to_owned)
            .ok_or(StorageError::RecoveryBindingRequired)
    }
    // Preserve legacy marker bytes without inventing a JSON-sized identity
    // limit. The retained file's observed size bounds this one read + sentinel;
    // growth/short EOF/change is refusal, never a truncated identity/hash.
    fn read_marker(file: &mut File) -> Result<Vec<u8>> {
        let before = file
            .metadata()
            .map_err(|e| StorageError::io("inspect native marker size", e))?;
        let limit = before
            .len()
            .checked_add(1)
            .ok_or(StorageError::HashSizeLimit)?;
        let mut bytes = Vec::new();
        file.take(limit)
            .read_to_end(&mut bytes)
            .map_err(|e| StorageError::io("read exact native marker", e))?;
        let after = file
            .metadata()
            .map_err(|e| StorageError::io("reinspect native marker size", e))?;
        if u64::try_from(bytes.len()).ok() != Some(before.len())
            || before.len() != after.len()
            || before.nlink() != 1
            || after.nlink() != 1
            || before.modified().ok() != after.modified().ok()
        {
            return Err(StorageError::IdentityMismatch);
        }
        Ok(bytes)
    }
    #[cfg(test)]
    fn read_bounded(reader: &mut impl Read) -> Result<Vec<u8>> {
        let mut bytes = Vec::new();
        reader
            .take((crate::profile::MAX_PROFILE_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|e| StorageError::io("read native marker", e))?;
        if bytes.len() > crate::profile::MAX_PROFILE_BYTES {
            return Err(StorageError::RecoveryBindingRequired);
        }
        Ok(bytes)
    }
    fn require_no_sidecars(database: &Directory) -> Result<()> {
        for name in [
            "librairii.sqlite3-wal",
            "librairii.sqlite3-shm",
            "librairii.sqlite3-journal",
        ] {
            if database.read_regular(OsStr::new(name))?.is_some() {
                return Err(StorageError::ClosedSnapshotRequired);
            }
        }
        Ok(())
    }
    fn require_root_entries(root: &Directory) -> Result<()> {
        for name in root.entries(14, &WorkCancellation::default())? {
            let known = [
                crate::identity::IDENTITY_FILE,
                crate::platform::lock::LIBRARY_LOCK_FILE,
                NATIVE_STATE_FILE,
                "db",
                "archives",
                "catalog",
                "staging",
                "trash",
                "logs",
                "state.json",
                "history.json",
            ];
            if name
                .as_encoded_bytes()
                .starts_with(NATIVE_STATE_TEMP.as_bytes())
                || !known.iter().any(|allowed| name == OsStr::new(allowed))
            {
                return Err(StorageError::RecoveryBindingRequired);
            }
        }
        Ok(())
    }
    fn require_paths(
        root: &Path,
        source: &Path,
        store: &NativeProfileStore,
        locations: &ProfileLocations,
    ) -> Result<()> {
        // ProfileLocations is DATA and its public fields are not authority.
        // Enforce the selected native names independently; a caller cannot
        // label an empty Go-discoverable config directory as "native".
        #[cfg(target_os = "macos")]
        let (native_name, data_name, legacy_name) = (
            "io.github.librairii.native",
            "LibrairiiNative",
            "io.github.librairii.app",
        );
        #[cfg(target_os = "linux")]
        let (native_name, data_name, legacy_name) =
            ("librairii-native", "librairii-native", "librairii");
        if locations.native.file_name() != Some(OsStr::new(native_name))
            || locations.data.file_name() != Some(OsStr::new(data_name))
            || locations.legacy.as_ref().and_then(|path| path.file_name())
                != Some(OsStr::new(legacy_name))
        {
            return Err(StorageError::PathEscape);
        }
        let resolve = |path: &Path| {
            if !path.is_absolute() {
                return Err(StorageError::RootNotAbsolute);
            }
            fs::canonicalize(path)
                .map_err(|error| StorageError::io("resolve explicit native namespace", error))
        };
        let source_actual = resolve(source)?;
        if source_actual != source {
            return Err(StorageError::PathEscape);
        }
        let native = resolve(&locations.native)?;
        let store_root = resolve(store.directory())?;
        let data = resolve(&locations.data)?;
        if native != store_root
            || native.file_name() != Some(OsStr::new(native_name))
            || data.file_name() != Some(OsStr::new(data_name))
            || overlaps(&native, &data)
            || !root.starts_with(&data)
            || root == data
            || overlaps(&native, source)
            || overlaps(&data, source)
        {
            return Err(StorageError::PathEscape);
        }
        if let Some(legacy) = &locations.legacy {
            if !legacy.is_absolute() {
                return Err(StorageError::RootNotAbsolute);
            }
            // Resolve a materialized legacy namespace too; do not let an alias
            // make the explicit native profile/data location legacy-discoverable.
            let legacy = match fs::canonicalize(legacy) {
                Ok(path) => path,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => legacy.clone(),
                Err(error) => {
                    return Err(StorageError::io("inspect explicit legacy namespace", error));
                }
            };
            if overlaps(&legacy, &native) || overlaps(&legacy, &data) {
                return Err(StorageError::PathEscape);
            }
            match fs::symlink_metadata(legacy.join("relocation.json")) {
                Ok(_) => return Err(StorageError::RecoveryBindingRequired),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
                Err(error) => {
                    return Err(StorageError::io(
                        "inspect explicit source recovery namespace",
                        error,
                    ));
                }
            }
        }
        Ok(())
    }
    fn overlaps(a: &Path, b: &Path) -> bool {
        a.starts_with(b) || b.starts_with(a)
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::{
            adoption::{ClosedLibraryInput, ClosedManifest},
            database::Database,
            profile::{Profile, ProfileDocument},
        };
        use librairii_testkit::TestRoots;
        use std::{
            error::Error,
            io::{Seek, SeekFrom},
        };
        type TestResult<T = ()> = std::result::Result<T, Box<dyn Error>>;
        fn copy(roots: &TestRoots) -> TestResult<(IndependentCopy, ProfileLocations)> {
            let source = roots.library().join("source");
            fs::create_dir(&source)?;
            let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../preservation/corpus");
            let bytes = fs::read(base.join("profiles/library-manifest.json"))?;
            let entries: serde_json::Value = serde_json::from_slice(&bytes)?;
            for entry in entries.as_array().ok_or("manifest")? {
                let relative = entry["path"].as_str().ok_or("relative")?;
                if entry["directory"] == true {
                    fs::create_dir(source.join(relative))?;
                } else {
                    fs::copy(
                        base.join("libraries/schema018").join(relative),
                        source.join(relative),
                    )?;
                }
            }
            let profile = Profile {
                root: exact_path(&source)?,
                library_id: "a0000000-0000-4000-8000-000000000001".into(),
                complete: true,
                ..Profile::setup()
            };
            let input = ClosedLibraryInput::verify(
                &source,
                ProfileDocument::decode(&serde_json::to_vec(&profile)?)?,
                ClosedManifest::decode(&bytes)?,
                &WorkCancellation::default(),
            )?;
            let locations = ProfileLocations::for_host(
                &roots.profile().join("home"),
                Some(&roots.profile().join("config")),
                Some(&roots.library().join("data")),
            )?;
            fs::create_dir_all(&locations.data)?;
            fs::create_dir_all(locations.native.parent().ok_or("native parent")?)?;
            fs::create_dir_all(locations.legacy.as_ref().ok_or("legacy namespace")?)?;
            Ok((
                input
                    .copy_to_new_root(&locations.data.join("copy"), &WorkCancellation::default())?,
                locations,
            ))
        }
        #[test]
        fn concurrent_native_observers_do_not_refuse_each_other_as_profile_updates() -> TestResult {
            let roots = TestRoots::new()?;
            let (copy, locations) = copy(&roots)?;
            let store = NativeProfileStore::create_fresh(&locations.native)?;
            let native = NativeLibrary::bind_independent(
                copy,
                &store,
                &locations,
                &WorkCancellation::default(),
            )
            .map_err(|f| f.to_string())?;
            let mut db = Database::open_native(&native)?;
            let barrier = std::sync::Barrier::new(4);
            std::thread::scope(|scope| -> TestResult {
                let workers: Vec<_> = (0..4)
                    .map(|_| {
                        scope.spawn(|| {
                            barrier.wait();
                            for _ in 0..100 {
                                native.revalidate(&WorkCancellation::default())?;
                            }
                            Ok::<_, StorageError>(())
                        })
                    })
                    .collect();
                for worker in workers {
                    worker.join().map_err(|_| "observer panicked")??;
                }
                Ok(())
            })?;
            db.close()?;
            drop(db);
            native.finish().map_err(|f| f.to_string())?;
            Ok(())
        }
        #[test]
        fn settled_import_stages_recycle_registration_slots_in_one_session() -> TestResult {
            use crate::{
                database::TransactionOutcome,
                platform::unix::{EntryName, FileArea, LibraryFiles, Publication},
            };
            use librairii_app::ports::creation::{NewItem, NewOperation};
            use librairii_core::OperationKind;
            use std::io::Write;
            for publish in [false, true] {
                let roots = TestRoots::new()?;
                let (copy, locations) = copy(&roots)?;
                let store = NativeProfileStore::create_fresh(&locations.native)?;
                let native = NativeLibrary::bind_independent(
                    copy,
                    &store,
                    &locations,
                    &WorkCancellation::default(),
                )
                .map_err(|f| f.to_string())?;
                let mut db = Database::open_native(&native)?;
                let TransactionOutcome::Committed(operation) = db
                    .create_operation(
                        NewOperation {
                            id: "a0000000-0000-4000-8000-000000000093".into(),
                            kind: OperationKind::Import,
                            source: None,
                            destination: None,
                            items: (0..66)
                                .map(|n| NewItem {
                                    source_name: format!("import-{n}"),
                                    ..Default::default()
                                })
                                .collect(),
                        },
                        "2026-10-22T00:00:00Z".into(),
                        Default::default(),
                    )?
                    .wait()?
                else {
                    return Err("create operation was not acknowledged".into());
                };
                for item in &operation.items {
                    assert!(matches!(
                        db.mark_operation_item_running(
                            operation.operation.identity.id.clone(),
                            item.identity.id,
                            "2026-10-22T00:00:01Z".into(),
                            Default::default(),
                        )?
                        .wait()?,
                        TransactionOutcome::Committed(_)
                    ));
                }
                {
                    let files = LibraryFiles::open_native(&db)?;
                    let guard = files.try_custody()?;
                    for (index, item) in operation.items.iter().enumerate() {
                        let mut stage = files
                            .create_stage(&guard)
                            .map_err(|f| f.cause().to_string())?;
                        stage.register_operation(
                            &guard,
                            operation.operation.identity.id.clone(),
                            item.identity.id,
                        )?;
                        stage.write_all(b"registered import")?;
                        if publish {
                            let digest = stage.measure(17, &WorkCancellation::default())?;
                            let publication = stage
                                .publish_no_clobber(
                                    &guard,
                                    FileArea::Archives,
                                    EntryName::parse("registered.pack")?,
                                    digest,
                                    &WorkCancellation::default(),
                                )
                                .map_err(|f| f.cause.to_string())?;
                            assert_eq!(matches!(publication, Publication::NewOwned(_)), index == 0);
                            publication
                                .finish_metadata_stage(&guard)
                                .map_err(|f| f.cause.to_string())?;
                        } else {
                            let retired = stage
                                .retire_owned_temp(&guard)
                                .map_err(|f| f.cause.to_string())?;
                            retired.revalidate(&guard)?;
                        }
                    }
                    assert!(native.registered_temporaries()?.is_empty());
                    assert_eq!(fs::read_dir(native.layout().staging())?.count(), 0);
                    if publish {
                        assert_eq!(
                            fs::read(native.layout().archives().join("registered.pack"))?,
                            b"registered import"
                        );
                    } else {
                        // Retirement preserves recoverable trash; it only frees
                        // the registration for the former staging workspace.
                        assert_eq!(fs::read_dir(native.layout().trash())?.count(), 66);
                        let mut stage = files
                            .create_stage(&guard)
                            .map_err(|f| f.cause().to_string())?;
                        stage.register_operation(
                            &guard,
                            operation.operation.identity.id.clone(),
                            operation.items[0].identity.id,
                        )?;
                        stage.write_all(b"preserved")?;
                        assert!(
                            stage
                                .register_operation(
                                    &guard,
                                    operation.operation.identity.id.clone(),
                                    operation.items[1].identity.id
                                )
                                .is_err()
                        );
                        let registration = native
                            .registered_temporaries()?
                            .pop()
                            .ok_or("registration")?;
                        let original = native.layout().staging().join(&registration.name);
                        let held = native.layout().staging().join("held-original");
                        fs::rename(&original, &held)?;
                        fs::create_dir(&original)?;
                        let failure = stage
                            .retire_owned_temp(&guard)
                            .err()
                            .ok_or("substitution accepted")?;
                        assert_eq!(native.registered_temporaries()?, [registration]);
                        assert_eq!(fs::read(held.join("payload"))?, b"preserved");
                        drop(failure);
                    }
                }
                db.close()?;
                drop(db);
                if publish {
                    native.finish().map_err(|f| f.to_string())?;
                }
            }
            Ok(())
        }
        fn fault() -> Result<()> {
            Err(StorageError::io(
                "injected native record post-replace flush boundary",
                std::io::Error::other("durability acknowledgement lost"),
            ))
        }
        #[test]
        fn native_exact_paths_refuse_non_utf8_without_lossy_state_or_profile_strings() {
            use std::{ffi::OsString, os::unix::ffi::OsStringExt};
            let raw = OsString::from_vec(b"/explicit/invalid-utf8-\xff".to_vec());
            assert!(matches!(
                exact_path(Path::new(&raw)),
                Err(StorageError::RecoveryBindingRequired)
            ));
            assert_eq!(Path::new(&raw).as_os_str(), raw.as_os_str());
        }
        #[test]
        fn native_activation_uncertainty_keeps_real_owner_and_admits_no_sqlite_or_managed_mutation()
        -> TestResult {
            let roots = TestRoots::new()?;
            let (copy, locations) = copy(&roots)?;
            let root = copy.root().to_owned();
            let before = fs::read(root.join("db/librairii.sqlite3"))?;
            let store = NativeProfileStore::create_fresh(&locations.native)?;
            let native = NativeLibrary::bind_independent(
                copy,
                &store,
                &locations,
                &WorkCancellation::default(),
            )
            .map_err(|f| f.to_string())?;
            assert!(native.claim_runtime_with(fault).is_err());
            assert!(!root.join("staging").exists());
            assert!(!root.join("catalog").exists());
            assert!(!root.join("db/librairii.sqlite3-wal").exists());
            assert_eq!(fs::read(root.join("db/librairii.sqlite3"))?, before);
            assert!(Database::open_native(&native).is_err());
            assert!(native.record.lock().map_err(|_| "record mutex")?.uncertain);
            assert!(LibraryLease::acquire_existing_in(&root, &native.root).is_err());
            drop(native);
            drop(store);
            let store = NativeProfileStore::open_native(&locations.native)?;
            assert!(matches!(
                NativeLibrary::reopen(&store, &locations, &WorkCancellation::default()),
                Err(StorageError::RecoveryBindingRequired)
            ));
            Ok(())
        }
        #[test]
        fn native_closed_response_loss_occurs_only_after_settlement_and_retains_frozen_old_new_owners()
        -> TestResult {
            let roots = TestRoots::new()?;
            let (copy, locations) = copy(&roots)?;
            let root = copy.root().to_owned();
            let store = NativeProfileStore::create_fresh(&locations.native)?;
            let native = NativeLibrary::bind_independent(
                copy,
                &store,
                &locations,
                &WorkCancellation::default(),
            )
            .map_err(|f| f.to_string())?;
            let mut db = Database::open_native(&native)?;
            db.close()?;
            drop(db);
            let failure = native
                .finish_with(|| {
                    assert_eq!(
                        fs::read_dir(root.join("staging"))
                            .map_err(|e| StorageError::io("observe settled staging", e))?
                            .count(),
                        0
                    );
                    assert_eq!(
                        fs::read_dir(root.join("trash"))
                            .map_err(|e| StorageError::io("observe settled trash", e))?
                            .count(),
                        0
                    );
                    assert!(!root.join("db/librairii.sqlite3-wal").exists());
                    fault()
                })
                .err()
                .ok_or("lost close acknowledgement was success")?;
            assert!(matches!(
                store.update(|_| Ok(())),
                Err(StorageError::ProfileBusy)
            ));
            let native = failure.retained_library();
            assert!(!native.runtime_active.load(Ordering::Acquire));
            assert!(native.closed_success.load(Ordering::Acquire));
            assert!(native.frozen.is_some());
            assert!(native.fenced.load(Ordering::Acquire));
            assert!(LibraryLease::acquire_existing_in(&root, &native.root).is_err());
            let mut record = native.record.lock().map_err(|_| "record mutex")?;
            assert!(record.uncertain);
            assert_eq!(record.document.state, Phase::Closed);
            let mut old = record.retained_file_for_test()?;
            old.seek(SeekFrom::Start(0))?;
            let old: serde_json::Value = serde_json::from_slice(&read_bounded(&mut old)?)?;
            assert_eq!(old["state"], "active");
            drop(record);
            drop(failure);
            drop(store);
            let store = NativeProfileStore::open_native(&locations.native)?;
            assert!(matches!(
                NativeLibrary::reopen(&store, &locations, &WorkCancellation::default()),
                Err(StorageError::RecoveryBindingRequired)
            ));
            Ok(())
        }
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
pub use owned::{NativeBindingFailure, NativeCloseFailure, NativeLibrary, RecoveryLibrary};

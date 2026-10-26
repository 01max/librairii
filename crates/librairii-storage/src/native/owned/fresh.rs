//! Explicit new-install confirmation; no existing Go root is initialized here.
use super::*;
use crate::{
    platform::unix::{ConfigTemp, durable_file_flush},
    profile::Profile,
    settings::TargetPreview,
};
use librairii_app::localization::Language;
use librairii_core::LibraryId;
use std::io::{Seek, Write};

#[derive(Debug)]
struct NewInstallAttempt<'profile> {
    target: TargetPreview,
    store: &'profile NativeProfileStore,
    locations: ProfileLocations,
    id: LibraryId,
    language: Language,
    protected: bool,
    source: Option<PathBuf>,
    policy: Option<RootPolicy>,
    cursor: Option<(PathBuf, Directory)>,
    parents: Vec<(PathBuf, Directory)>,
    next_parent: usize,
    root: Option<Directory>,
    name: Option<EntryName>,
    database: Option<Directory>,
    directories: Vec<(&'static str, Directory)>,
    marker: Option<ConfigTemp>,
    identity: Option<LibraryIdentity>,
    database_file: Option<File>,
    initialized: bool,
    record: Option<Record>,
    allocation: Option<ControlAllocation>,
    // Last: retained even after any partial initialization/publication failure.
    lease: Option<LibraryLease>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::Database;
    type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;
    const ID: &str = "b0000000-0000-4000-8000-000000000013";

    #[test]
    fn confirmed_setup_uses_exact_empty_or_missing_root_and_reopens_without_legacy_source()
    -> TestResult {
        for existing in [false, true] {
            let (roots, _copy, locations) = crate::test_support::native_copy()?;
            let store = NativeProfileStore::create_fresh(&locations.native)?;
            let chosen = roots.library().join("user choice").join("exact root");
            if existing {
                fs::create_dir_all(&chosen)?;
            }
            let preview = TargetPreview::inspect(&chosen, &[], &Default::default())?;
            assert!(!chosen.join("db").exists());
            let native = NativeLibrary::initialize_confirmed(
                preview,
                &store,
                &locations,
                ID.parse()?,
                Language::French,
                &Default::default(),
            )
            .map_err(|f| f.to_string())?;
            assert_eq!(native.layout().root(), chosen);
            assert_eq!(
                store.load()?.ok_or("profile")?.profile().root,
                chosen.to_str().ok_or("root")?
            );
            let mut db = Database::open_native(&native)?;
            assert_eq!(
                db.read(|c| c
                    .query_row("SELECT COUNT(*) FROM stories", [], |r| r.get::<_, i64>(0))
                    .map_err(StorageError::from))?
                    .wait()?,
                0
            );
            db.close()?;
            drop(db);
            native.finish().map_err(|f| f.to_string())?;
            let reopened = NativeLibrary::reopen(&store, &locations, &Default::default())?;
            let mut db = Database::open_native(&reopened)?;
            db.close()?;
            drop(db);
            reopened.finish().map_err(|f| f.to_string())?;
            assert!(!legacy_default_root(&locations)?.exists());
        }
        Ok(())
    }

    #[test]
    fn setup_cancellation_invalid_profile_and_existing_default_never_initialize_or_replace()
    -> TestResult {
        let (roots, _copy, locations) = crate::test_support::native_copy()?;
        let store = NativeProfileStore::create_fresh(&locations.native)?;
        let chosen = roots.library().join("not created");
        let preview = TargetPreview::inspect(&chosen, &[], &Default::default())?;
        let cancel = WorkCancellation::default();
        cancel.cancel();
        let failure = NativeLibrary::initialize_confirmed(
            preview,
            &store,
            &locations,
            ID.parse()?,
            Language::English,
            &cancel,
        )
        .err()
        .ok_or("cancel")?;
        assert!(matches!(failure.storage_error(), StorageError::Cancelled));
        assert!(!chosen.exists());
        drop(failure);
        let legacy = legacy_default_root(&locations)?;
        fs::create_dir(&legacy)?;
        fs::write(legacy.join("keep"), b"original")?;
        let preview = TargetPreview::inspect(&chosen, &[], &Default::default())?;
        assert!(
            NativeLibrary::initialize_confirmed(
                preview,
                &store,
                &locations,
                ID.parse()?,
                Language::English,
                &Default::default()
            )
            .is_err()
        );
        assert!(!chosen.exists());
        assert_eq!(fs::read(legacy.join("keep"))?, b"original");
        fs::write(locations.native.join("settings.json"), b"invalid")?;
        assert!(crate::profile::read_legacy(&locations.native).is_err());
        assert_eq!(
            fs::read(locations.native.join("settings.json"))?,
            b"invalid"
        );
        assert!(!chosen.exists());
        Ok(())
    }

    #[test]
    fn setup_retries_busy_profile_preserving_newer_language() -> TestResult {
        let (roots, _copy, locations) = crate::test_support::native_copy()?;
        let store = NativeProfileStore::create_fresh(&locations.native)?;
        let chosen = roots.library().join("partial");
        let preview = TargetPreview::inspect(&chosen, &[], &Default::default())?;
        let mut retained = None;
        store.update(|p| {
            retained = NativeLibrary::initialize_confirmed(
                preview,
                &store,
                &locations,
                ID.parse().map_err(|_| StorageError::InvalidProfile)?,
                Language::English,
                &Default::default(),
            )
            .err();
            p.language = "fr".into();
            Ok(())
        })?;
        let failure = retained.ok_or("busy")?;
        assert!(matches!(failure.storage_error(), StorageError::ProfileBusy));
        let native = failure
            .retry(&Default::default())
            .map_err(|f| f.to_string())?;
        assert_eq!(store.load()?.ok_or("profile")?.profile().language, "fr");
        let mut db = Database::open_native(&native)?;
        db.close()?;
        drop(db);
        native.finish().map_err(|f| f.to_string())?;
        Ok(())
    }
}
#[derive(Debug)]
pub struct NewInstallFailure<'profile> {
    cause: StorageError,
    attempt: NewInstallAttempt<'profile>,
}
impl<'profile> NewInstallFailure<'profile> {
    pub fn storage_error(&self) -> &StorageError {
        &self.cause
    }
    pub fn selected_root(&self) -> &Path {
        self.attempt.target.requested()
    }
    pub fn retry(
        self: Box<Self>,
        cancellation: &WorkCancellation,
    ) -> std::result::Result<NativeLibrary<'profile>, Box<Self>> {
        self.attempt.complete(cancellation)
    }
}
impl std::fmt::Display for NewInstallFailure<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "setup initialization failed: {}", self.cause)
    }
}
impl std::error::Error for NewInstallFailure<'_> {}

impl<'profile> NativeLibrary<'profile> {
    /// A UI confirmation calls this consuming operation. Startup inspection
    /// never calls it. The exact selected directory becomes the library root.
    pub fn initialize_confirmed(
        target: TargetPreview,
        store: &'profile NativeProfileStore,
        locations: &ProfileLocations,
        id: LibraryId,
        language: Language,
        cancellation: &WorkCancellation,
    ) -> std::result::Result<Self, Box<NewInstallFailure<'profile>>> {
        NewInstallAttempt {
            target,
            store,
            locations: locations.clone(),
            id,
            language,
            protected: false,
            source: None,
            policy: None,
            cursor: None,
            parents: Vec::new(),
            next_parent: 0,
            root: None,
            name: None,
            database: None,
            directories: Vec::new(),
            marker: None,
            identity: None,
            database_file: None,
            initialized: false,
            record: None,
            allocation: None,
            lease: None,
        }
        .complete(cancellation)
    }
}
impl<'profile> NewInstallAttempt<'profile> {
    fn complete(
        mut self,
        cancellation: &WorkCancellation,
    ) -> std::result::Result<NativeLibrary<'profile>, Box<NewInstallFailure<'profile>>> {
        match self.run(cancellation) {
            Ok(library) => Ok(library),
            Err(cause) => Err(Box::new(NewInstallFailure {
                cause,
                attempt: self,
            })),
        }
    }
    fn run(&mut self, cancellation: &WorkCancellation) -> Result<NativeLibrary<'profile>> {
        if cancellation.is_cancelled() {
            return Err(StorageError::Cancelled);
        }
        if self.store.load()?.is_some_and(|document| {
            let p = document.profile();
            p.complete || p.recovering || !p.root.is_empty() || !p.library_id.is_empty()
        }) {
            return Err(StorageError::RecoveryBindingRequired);
        }
        self.store.require_unrecovered_namespace()?;
        namespace_roots(self.store, &self.locations)?;
        if !self.protected {
            let source = legacy_default_root(&self.locations)?;
            if let Some(legacy) = &self.locations.legacy
                && crate::profile::read_legacy(legacy)?.is_some_and(|document| {
                    let p = document.profile();
                    p.complete || p.recovering || !p.root.is_empty() || !p.library_id.is_empty()
                }) {
                    return Err(StorageError::RecoveryBindingRequired);
                }
            match fs::read_dir(&source) {
                Ok(mut entries) => {
                    if entries.next().is_some() {
                        return Err(StorageError::RecoveryBindingRequired);
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
                Err(error) => {
                    return Err(StorageError::io(
                        "inspect old default before new setup",
                        error,
                    ));
                }
            }
            let mut excluded = vec![self.locations.native.as_path(), source.as_path()];
            if let Some(legacy) = &self.locations.legacy {
                excluded.push(legacy);
            }
            self.target.protect(&excluded, cancellation)?;
            self.target.check_writable(cancellation)?;
            self.policy = Some(RootPolicy::for_source(&source, &self.locations)?);
            self.source = Some(source);
            self.protected = true;
        }
        self.acquire_root(cancellation)?;
        self.revalidate_root()?;
        let root = self
            .root
            .as_ref()
            .ok_or(StorageError::CompatibilityInvariant)?;
        let path = self.target.resolved();
        if self.lease.is_none() {
            root.entries(0, cancellation)?;
            self.lease = Some(LibraryLease::create_fresh_in(path, root)?);
        }
        self.lease
            .as_ref()
            .ok_or(StorageError::CompatibilityInvariant)?
            .revalidate()?;
        if self.marker.is_none() {
            self.marker = Some(
                root.create_control_record(OsStr::new(crate::identity::IDENTITY_FILE))
                    .map_err(|failure| {
                        let failure = *failure;
                        self.allocation = Some(failure.allocation);
                        failure.cause
                    })?,
            );
        }
        let marker_bytes = format!("{}\n", self.id).into_bytes();
        let marker = self
            .marker
            .as_mut()
            .ok_or(StorageError::CompatibilityInvariant)?;
        let named = root.open_regular(
            OsStr::new(crate::identity::IDENTITY_FILE),
            rustix::fs::OFlags::RDONLY,
        )?;
        if !crate::settings::manifest::same_file(&marker.file, &named)? {
            return Err(StorageError::SnapshotChanged);
        }
        marker
            .file
            .rewind()
            .map_err(|e| StorageError::io("rewind setup marker", e))?;
        let mut current = Vec::new();
        (&mut marker.file)
            .take((marker_bytes.len() + 1) as u64)
            .read_to_end(&mut current)
            .map_err(|e| StorageError::io("observe own setup marker", e))?;
        if !marker_bytes.starts_with(&current) {
            return Err(StorageError::IdentityMismatch);
        }
        marker
            .file
            .rewind()
            .map_err(|e| StorageError::io("rewind own setup identity", e))?;
        marker
            .file
            .write_all(&marker_bytes)
            .map_err(|e| StorageError::io("write new setup identity", e))?;
        durable_file_flush(&marker.file)?;
        self.identity = Some(LibraryIdentity::from_bytes(marker_bytes)?);
        for name in ["db", "archives", "catalog", "staging", "trash", "logs"] {
            if !self.directories.iter().any(|(owned, _)| *owned == name) {
                let directory = root.create_child(OsStr::new(name))?;
                self.directories.push((name, directory));
            }
        }
        if self.database.is_none() {
            self.database = Some(root.child(OsStr::new("db"))?);
        }
        let database = self
            .database
            .as_ref()
            .ok_or(StorageError::CompatibilityInvariant)?;
        if !self.initialized {
            if self.database_file.is_some() {
                return Err(StorageError::RecoveryBindingRequired);
            }
            self.database_file = Some(database.open_regular(
                OsStr::new("librairii.sqlite3"),
                rustix::fs::OFlags::CREATE | rustix::fs::OFlags::EXCL | rustix::fs::OFlags::RDWR,
            )?);
            self.revalidate_root()?;
            let connection = crate::database::initialize_new(
                &path.join("db/librairii.sqlite3"),
                &crate::migrations::MIGRATIONS,
            )?;
            connection.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")?;
            connection
                .close()
                .map_err(|(_, error)| StorageError::Sqlite(error))?;
            let file = self
                .database_file
                .as_ref()
                .ok_or(StorageError::CompatibilityInvariant)?;
            if !crate::settings::manifest::same_file(
                file,
                &database
                    .open_regular(OsStr::new("librairii.sqlite3"), rustix::fs::OFlags::RDONLY)?,
            )? {
                return Err(StorageError::SnapshotChanged);
            }
            durable_file_flush(file)?;
            database.flush()?;
            self.initialized = true;
        }
        require_no_sidecars(database)?;
        for (_, directory) in &self.directories {
            directory.flush()?;
        }
        root.flush()?;
        self.revalidate_root()?;
        if self.record.is_none() {
            let manifest = crate::settings::RelocationManifest::capture(root, cancellation)?;
            let identity = self
                .identity
                .as_ref()
                .ok_or(StorageError::CompatibilityInvariant)?;
            let document = Document {
                format: "librairii.native-library".into(),
                version: 2,
                state: Phase::ImportedClosed,
                epoch: 0,
                native_profile_root: exact_path(
                    &fs::canonicalize(self.store.directory())
                        .map_err(|e| StorageError::io("resolve setup profile namespace", e))?,
                )?,
                native_data_root: exact_path(
                    &fs::canonicalize(&self.locations.data)
                        .map_err(|e| StorageError::io("resolve setup data namespace", e))?,
                )?,
                library_root: exact_path(path)?,
                source_root: exact_path(
                    self.source
                        .as_ref()
                        .ok_or(StorageError::CompatibilityInvariant)?,
                )?,
                library_id_raw: identity.spelling().into(),
                library_marker_sha256: digest(identity.original_bytes()),
                profile_marker_sha256: self.store.marker_digest()?,
                source_manifest_sha256: digest(&manifest.bytes()?),
                root_policy: self.policy.clone(),
                temporaries: Vec::new(),
            };
            self.record = Some(Record::create(root, document).map_err(|failure| {
                let failure = *failure;
                self.allocation = failure.allocation;
                failure.cause
            })?);
        }
        let record = self
            .record
            .as_mut()
            .ok_or(StorageError::CompatibilityInvariant)?;
        if record.uncertain {
            let named = root.read_config(OsStr::new(NATIVE_STATE_FILE))?;
            if !crate::settings::manifest::same_file(&named.file, &record.current.file)? {
                return Err(StorageError::SnapshotChanged);
            }
            record
                .current
                .file
                .rewind()
                .map_err(|e| StorageError::io("rewind own setup record", e))?;
            let mut current = Vec::new();
            (&mut record.current.file)
                .take((crate::profile::MAX_PROFILE_BYTES + 1) as u64)
                .read_to_end(&mut current)
                .map_err(|e| StorageError::io("read own incomplete setup record", e))?;
            if !record.bytes.starts_with(&current) {
                return Err(StorageError::RecoveryBindingRequired);
            }
            record
                .current
                .file
                .rewind()
                .map_err(|e| StorageError::io("rewind own incomplete setup record", e))?;
            record.initialize(root)?;
        } else {
            record.revalidate(root)?;
        }
        require_root_entries(root)?;
        crate::settings::RelocationManifest::capture(root, cancellation)?;
        self.revalidate_root()?;
        if cancellation.is_cancelled() {
            return Err(StorageError::Cancelled);
        }
        // No filesystem work or old settings snapshot runs inside this short
        // publication reservation; a newer independent language is retained.
        let proposed = Profile {
            version: 1,
            language: self.language.code().into(),
            root: exact_path(path)?,
            library_id: self.id.to_string(),
            complete: true,
            recovering: false,
        };
        let marker = self
            .marker
            .as_ref()
            .ok_or(StorageError::CompatibilityInvariant)?
            .file
            .try_clone()
            .map_err(|e| StorageError::io("retain new library marker", e))?;
        let parent = self
            .cursor
            .as_ref()
            .ok_or(StorageError::CompatibilityInvariant)?
            .1
            .duplicate()?;
        let layout = LibraryLayout::probe(path)?;
        let binding = self.store.begin_binding()?.publish(&proposed)?;
        let identity = self
            .identity
            .take()
            .ok_or(StorageError::CompatibilityInvariant)?;
        let owner = RootOwner::Existing(Box::new(ExistingRoot {
            parent,
            name: self
                .name
                .take()
                .ok_or(StorageError::CompatibilityInvariant)?,
            directories: std::mem::take(&mut self.directories),
            lease: self
                .lease
                .take()
                .ok_or(StorageError::CompatibilityInvariant)?,
        }));
        Ok(NativeLibrary::new(
            NativeDirectories {
                layout,
                root: self
                    .root
                    .take()
                    .ok_or(StorageError::CompatibilityInvariant)?,
                database: self
                    .database
                    .take()
                    .ok_or(StorageError::CompatibilityInvariant)?,
            },
            marker,
            identity,
            self.record
                .take()
                .ok_or(StorageError::CompatibilityInvariant)?,
            owner,
            binding,
        ))
    }
    fn acquire_root(&mut self, cancellation: &WorkCancellation) -> Result<()> {
        if self.root.is_some() {
            return Ok(());
        }
        if self.cursor.is_none() {
            self.target.revalidate(cancellation)?;
            self.cursor = Some((
                self.target.anchor_path.clone(),
                self.target.anchor.duplicate()?,
            ));
        }
        while self.next_parent + 1 < self.target.missing.len() {
            if cancellation.is_cancelled() {
                return Err(StorageError::Cancelled);
            }
            let (path, directory) = self
                .cursor
                .as_ref()
                .ok_or(StorageError::CompatibilityInvariant)?;
            directory.matches_metadata(
                &fs::symlink_metadata(path)
                    .map_err(|e| StorageError::io("revalidate setup ancestor", e))?,
            )?;
            let name = &self.target.missing[self.next_parent];
            let child = directory.create_child(name)?;
            let child_path = path.join(name);
            self.parents.push((child_path.clone(), child.duplicate()?));
            directory.flush()?;
            self.cursor = Some((child_path, child));
            self.next_parent += 1;
        }
        let name = EntryName::parse(
            self.target
                .missing
                .last()
                .ok_or(StorageError::SettingsDestination)?,
        )?;
        self.name = Some(name.clone());
        let directory = &self
            .cursor
            .as_ref()
            .ok_or(StorageError::CompatibilityInvariant)?
            .1;
        self.root = Some(match &self.target.existing {
            Some(existing) => existing.duplicate()?,
            None => directory.create_child(name.as_os_str())?,
        });
        directory.flush()?;
        Ok(())
    }
    fn revalidate_root(&self) -> Result<()> {
        let root = self
            .root
            .as_ref()
            .ok_or(StorageError::CompatibilityInvariant)?;
        root.matches_metadata(
            &fs::symlink_metadata(self.target.resolved())
                .map_err(|e| StorageError::io("revalidate exact setup root", e))?,
        )?;
        for (path, directory) in &self.parents {
            directory.matches_metadata(
                &fs::symlink_metadata(path)
                    .map_err(|e| StorageError::io("revalidate created setup parent", e))?,
            )?;
        }
        let (parent_path, parent) = self
            .cursor
            .as_ref()
            .ok_or(StorageError::CompatibilityInvariant)?;
        parent.matches_metadata(
            &fs::symlink_metadata(parent_path)
                .map_err(|e| StorageError::io("revalidate setup publication parent", e))?,
        )?;
        require_explicit_paths(
            self.target.resolved(),
            self.source
                .as_ref()
                .ok_or(StorageError::CompatibilityInvariant)?,
            self.policy
                .as_ref()
                .ok_or(StorageError::CompatibilityInvariant)?,
            self.store,
            &self.locations,
        )
    }
}

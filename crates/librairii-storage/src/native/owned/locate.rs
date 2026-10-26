//! Explicit same-UUID recovery after an external move. No folder initialization
//! or source cleanup occurs here, and an unresolved relocation stays gated.
use super::*;
#[derive(Debug)]
pub struct LocatedLibrary<'profile> {
    pub library: NativeLibrary<'profile>,
    pub retained_move_locations: Vec<PathBuf>,
}
impl<'profile> NativeLibrary<'profile> {
    pub fn locate_same_uuid(
        store: &'profile NativeProfileStore,
        locations: &ProfileLocations,
        selected: &Path,
        cancel: &WorkCancellation,
    ) -> Result<Self> {
        Ok(Self::locate_same_uuid_with_report(store, locations, selected, cancel)?.library)
    }
    pub fn locate_same_uuid_with_report(
        store: &'profile NativeProfileStore,
        locations: &ProfileLocations,
        selected: &Path,
        cancel: &WorkCancellation,
    ) -> Result<LocatedLibrary<'profile>> {
        if cancel.is_cancelled() {
            return Err(StorageError::Cancelled);
        }
        let journal = store.read_move_journal()?;
        if journal.is_none() {
            store.require_unrecovered_namespace()?;
        }
        let previous = store
            .load()?
            .ok_or(StorageError::RecoveryBindingRequired)?
            .profile()
            .clone();
        if !previous.complete || previous.recovering {
            return Err(StorageError::RecoveryBindingRequired);
        }
        match fs::symlink_metadata(&previous.root) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => return Err(StorageError::io("inspect missing configured root", e)),
            Ok(_) => return Err(StorageError::RecoveryBindingRequired),
        }
        let path = fs::canonicalize(selected)
            .map_err(|e| StorageError::io("resolve explicit recovery selection", e))?;
        if fs::symlink_metadata(selected)
            .map_err(|e| StorageError::io("inspect selected recovery root", e))?
            .file_type()
            .is_symlink()
        {
            return Err(StorageError::PathEscape);
        }
        let root = Directory::open(&path)?;
        let lease = LibraryLease::acquire_existing_in(&path, &root)?;
        let mut record = Record::open(&root)?;
        let d = &record.document;
        if d.state != Phase::Closed
            || (d.library_root != previous.root && d.library_root != exact_path(&path)?)
            || d.library_id_raw != previous.library_id
            || d.profile_marker_sha256 != store.marker_digest()?
            || d.native_profile_root
                != exact_path(
                    &fs::canonicalize(store.directory())
                        .map_err(|e| StorageError::io("resolve recovery profile", e))?,
                )?
            || d.native_data_root
                != exact_path(
                    &fs::canonicalize(&locations.data)
                        .map_err(|e| StorageError::io("resolve recovery data namespace", e))?,
                )?
        {
            return Err(StorageError::RecoveryBindingRequired);
        }
        if let Some(bytes) = &journal {
            relocation::validate_locate_role(store, &previous, d, bytes)?;
        }
        // A matching UUID/record is not authority to replay or overlook a
        // foreign journal. Match ordinary reopen before SQLite can write.
        require_root_entries(&root, d.version == 2)?;
        let policy = d.root_policy.clone().unwrap_or(RootPolicy::for_source(
            Path::new(&d.source_root),
            locations,
        )?);
        require_explicit_paths(&path, Path::new(&d.source_root), &policy, store, locations)?;
        let mut marker = root.open_regular(
            OsStr::new(crate::identity::IDENTITY_FILE),
            rustix::fs::OFlags::RDONLY,
        )?;
        let identity = LibraryIdentity::from_bytes(read_marker(&mut marker)?)?;
        identity.require_spelling(&previous.library_id)?;
        if digest(identity.original_bytes()) != d.library_marker_sha256 {
            return Err(StorageError::IdentityMismatch);
        }
        let database = root.child(OsStr::new("db"))?;
        require_no_sidecars(&database)?;
        let scratch =
            tempfile::tempdir().map_err(|e| StorageError::io("create located DB probe", e))?;
        crate::backup::probe_owned(&path.join("db/librairii.sqlite3"), scratch.path())?;
        crate::database::ready_closed_move(&path.join("db/librairii.sqlite3"))?;
        require_no_sidecars(&database)?;
        let layout = LibraryLayout::probe(&path)?;
        let parent = Directory::open(path.parent().ok_or(StorageError::RootNotAbsolute)?)?;
        let name = EntryName::parse(path.file_name().ok_or(StorageError::RootNotAbsolute)?)?;
        let mut directories = Vec::new();
        for name in ["db", "archives", "catalog", "staging", "trash", "logs"] {
            directories.push((name, root.child(OsStr::new(name))?));
        }
        // Explicitly locating the saved native root abandons automatic old
        // move cleanup. Verify its role first (a stale pre-commit source cannot
        // be selected as the committed destination), preserve every copy, and
        // retire only the native namespace journal before rebinding authority.
        let retained_move_locations = if let Some(bytes) = journal {
            relocation::abandon_for_locate(store, &previous, d, &bytes)?
        } else {
            Vec::new()
        };
        let mut next = d.clone();
        next.version = 2;
        next.library_root = exact_path(&path)?;
        next.root_policy = Some(policy);
        record.transition_with(&root, next, || Ok(()))?;
        lease.revalidate()?;
        if cancel.is_cancelled() {
            return Err(StorageError::Cancelled);
        }
        let verified = relocation::VerifiedMoveTarget {
            root: exact_path(&path)?,
            raw_id: identity.spelling().into(),
        };
        let binding = store.commit_located(&verified, &previous)?;
        Ok(LocatedLibrary {
            retained_move_locations,
            library: Self::new(
                NativeDirectories {
                    layout,
                    root,
                    database,
                },
                marker,
                identity,
                record,
                RootOwner::Existing(Box::new(ExistingRoot {
                    parent,
                    name,
                    directories,
                    lease,
                })),
                binding,
            ),
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::Database;
    type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;
    #[test]
    fn locate_refuses_foreign_root_controls_before_sqlite_or_profile_changes() -> TestResult {
        let roots = librairii_testkit::TestRoots::new()?;
        let locations = ProfileLocations::for_host(&roots.profile().join("account"), None, None)?;
        let store = crate::settings::prepare_native_profile(&locations, &Default::default())?;
        let old = locations.data.join("library");
        let native = NativeLibrary::initialize_confirmed(
            crate::settings::TargetPreview::inspect(&old, &[], &Default::default())?,
            &store,
            &locations,
            "b0000000-0000-4000-8000-000000000013".parse()?,
            librairii_app::localization::Language::English,
            &Default::default(),
        )
        .map_err(|e| e.to_string())?;
        let mut db = Database::open_native(&native)?;
        db.close()?;
        drop(db);
        fs::write(
            old.join("ordinary user notes.txt"),
            b"retained additional file",
        )?;
        native.finish().map_err(|e| e.to_string())?;
        let selected = roots.library().join("moved native v2");
        fs::rename(&old, &selected)?;
        let before_profile = store.load()?.ok_or("profile")?.original_bytes().to_vec();
        let before_db = fs::read(selected.join("db/librairii.sqlite3"))?;
        for name in [
            "relocation.json",
            ".librairii-foreign-control",
            ".librairii-native-state.json.tmp-unknown",
        ] {
            fs::write(selected.join(name), b"unverified control bytes")?;
            assert!(
                NativeLibrary::locate_same_uuid(&store, &locations, &selected, &Default::default())
                    .is_err()
            );
            assert_eq!(
                store.load()?.ok_or("profile")?.original_bytes(),
                before_profile
            );
            assert_eq!(fs::read(selected.join("db/librairii.sqlite3"))?, before_db);
            assert_eq!(fs::read(selected.join(name))?, b"unverified control bytes");
            assert!(!selected.join("db/librairii.sqlite3-wal").exists());
            fs::remove_file(selected.join(name))?;
        }
        let native =
            NativeLibrary::locate_same_uuid(&store, &locations, &selected, &Default::default())?;
        let mut db = Database::open_native(&native)?;
        db.close()?;
        drop(db);
        native.finish().map_err(|e| e.to_string())?;
        assert_eq!(
            fs::read(selected.join("ordinary user notes.txt"))?,
            b"retained additional file"
        );
        Ok(())
    }
    #[test]
    fn locating_same_uuid_preserves_language_and_refuses_wrong_or_missing_database() -> TestResult {
        let (roots, copy, locations) = crate::test_support::native_copy()?;
        let store = NativeProfileStore::create_fresh(&locations.native)?;
        let native = NativeLibrary::bind_independent(copy, &store, &locations, &Default::default())
            .map_err(|f| f.to_string())?;
        let old = native.layout().root().to_path_buf();
        let id = native.identity().original_bytes().to_vec();
        let mut db = Database::open_native(&native)?;
        db.close()?;
        drop(db);
        native.finish().map_err(|f| f.to_string())?;
        let selected = roots.library().join("externally moved");
        fs::rename(&old, &selected)?;
        fs::write(
            selected.join(crate::identity::IDENTITY_FILE),
            b"d0000000-0000-4000-8000-000000000013\n",
        )?;
        assert!(
            NativeLibrary::locate_same_uuid(&store, &locations, &selected, &Default::default())
                .is_err()
        );
        assert_eq!(
            store.load()?.ok_or("profile")?.profile().root,
            old.to_str().ok_or("root")?
        );
        fs::write(selected.join(crate::identity::IDENTITY_FILE), id)?;
        let database = selected.join("db/librairii.sqlite3");
        let preserved = selected.join("db/held-original.sqlite3");
        fs::rename(&database, &preserved)?;
        let before = store.load()?.ok_or("profile")?.original_bytes().to_vec();
        assert!(
            NativeLibrary::locate_same_uuid(&store, &locations, &selected, &Default::default())
                .is_err()
        );
        assert!(!database.exists());
        assert_eq!(store.load()?.ok_or("profile")?.original_bytes(), before);
        fs::rename(&preserved, &database)?;
        let native =
            NativeLibrary::locate_same_uuid(&store, &locations, &selected, &Default::default())?;
        assert_eq!(store.load()?.ok_or("profile")?.profile().language, "fr");
        let mut db = Database::open_native(&native)?;
        db.close()?;
        drop(db);
        native.finish().map_err(|f| f.to_string())?;
        assert!(!old.exists());
        Ok(())
    }
    #[test]
    fn independent_native_restart_does_not_require_original_legacy_drive() -> TestResult {
        let (_roots, copy, locations) = crate::test_support::native_copy()?;
        let original = copy.source_root().to_path_buf();
        let store = NativeProfileStore::create_fresh(&locations.native)?;
        let native = NativeLibrary::bind_independent(copy, &store, &locations, &Default::default())
            .map_err(|f| f.to_string())?;
        let mut db = Database::open_native(&native)?;
        db.close()?;
        drop(db);
        native.finish().map_err(|f| f.to_string())?;
        fs::rename(&original, original.with_file_name("disconnected-original"))?;
        let native = NativeLibrary::reopen(&store, &locations, &Default::default())?;
        let mut db = Database::open_native(&native)?;
        db.close()?;
        drop(db);
        native.finish().map_err(|f| f.to_string())?;
        assert!(!original.exists());
        Ok(())
    }
}

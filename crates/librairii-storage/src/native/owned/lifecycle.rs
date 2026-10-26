use super::*;

/// Actual root/profile ownership after every DB/file/logger borrow has ended
/// and durable Closed has been acknowledged. This is not caller-supplied idle
/// data. The root lease remains held while a relocation manifest/copy is made.
///
/// ```compile_fail
/// use librairii_storage::{database::Database, native::NativeLibrary};
/// fn cannot_move_open_readers(library: NativeLibrary<'_>) {
///     let db = Database::open_native(&library);
///     let _ = library.close_for_relocation();
///     drop(db);
/// }
/// ```
#[derive(Debug)]
pub struct ClosedNativeLibrary<'profile> {
    pub(super) library: NativeLibrary<'profile>,
}
impl<'profile> NativeLibrary<'profile> {
    pub fn preview_relocation(
        &self,
        selected: &Path,
        locations: &ProfileLocations,
        cancel: &WorkCancellation,
    ) -> Result<crate::settings::RelocationPreview> {
        use crate::settings::{RelocationPreview, TargetPreview};
        self.revalidate(cancel)?;
        match fs::canonicalize(selected) {
            Ok(path) => {
                if Directory::open(&path)?.identity() == self.root.identity() {
                    self.revalidate(cancel)?;
                    return Ok(RelocationPreview::Unchanged);
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => return Err(StorageError::io("resolve relocation selection", e)),
        }
        let record = self
            .record
            .lock()
            .map_err(|_| StorageError::RuntimeFenced)?;
        let document = &record.document;
        let mut excluded = vec![
            self.layout.root(),
            Path::new(&document.native_profile_root),
            Path::new(&document.source_root),
        ];
        let default = legacy_default_root(locations)?;
        excluded.push(&default);
        if let Some(legacy) = &locations.legacy {
            excluded.push(legacy);
        }
        if let Some(policy) = &document.root_policy {
            excluded.extend(policy.legacy_roots.iter().map(Path::new));
        }
        Ok(RelocationPreview::Destination(Box::new(
            TargetPreview::inspect(selected, &excluded, cancel)?,
        )))
    }
    pub fn close_for_relocation(
        mut self,
    ) -> std::result::Result<ClosedNativeLibrary<'profile>, Box<NativeCloseFailure<'profile>>> {
        if let Err(cause) = self.seal_closed(|| Ok(())) {
            if !matches!(&cause, StorageError::ProfileBusy) {
                self.fence();
            }
            return Err(Box::new(NativeCloseFailure {
                cause,
                library: self,
            }));
        }
        let Some(frozen) = self.frozen.take() else {
            self.fence();
            return Err(Box::new(NativeCloseFailure {
                cause: StorageError::RuntimeFenced,
                library: self,
            }));
        };
        frozen.thaw();
        Ok(ClosedNativeLibrary { library: self })
    }
}
impl<'profile> ClosedNativeLibrary<'profile> {
    pub fn relocation_manifest(
        &self,
        cancellation: &WorkCancellation,
    ) -> Result<crate::settings::RelocationManifest> {
        self.revalidate()?;
        let manifest =
            crate::settings::RelocationManifest::capture(&self.library.root, cancellation)?;
        self.revalidate()?;
        Ok(manifest)
    }
    pub fn layout(&self) -> &LibraryLayout {
        self.library.layout()
    }
    pub fn identity(&self) -> &LibraryIdentity {
        self.library.identity()
    }
    /// No root has been committed by this handoff. Reopening uses the real
    /// original owner and a fresh durable Active epoch, never a parsed path cast.
    pub fn resume(self) -> NativeLibrary<'profile> {
        self.library
    }
    pub fn revalidate(&self) -> Result<()> {
        self.library.revalidate(&WorkCancellation::default())?;
        let record = self
            .library
            .record
            .lock()
            .map_err(|_| StorageError::RuntimeFenced)?;
        if record.document.state != Phase::Closed || !record.document.temporaries.is_empty() {
            return Err(StorageError::RecoveryBindingRequired);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{database::Database, profile::NativeProfileStore};
    type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn current_root_selection_through_alias_is_unchanged_without_profile_or_lifecycle_effects()
    -> TestResult {
        use crate::settings::RelocationPreview;
        let (roots, copy, locations) = crate::test_support::native_copy()?;
        let store = NativeProfileStore::create_fresh(&locations.native)?;
        let native = NativeLibrary::bind_independent(copy, &store, &locations, &Default::default())
            .map_err(|f| f.to_string())?;
        let mut db = Database::open_native(&native)?;
        let before = store.load()?.ok_or("profile")?.original_bytes().to_vec();
        let state = fs::read(native.layout().root().join(NATIVE_STATE_FILE))?;
        let alias = roots.library().join("alias to current library");
        std::os::unix::fs::symlink(native.layout().root(), &alias)?;
        for selected in [
            native.layout().root(),
            alias.as_path(),
            native.layout().root().join(".").as_path(),
        ] {
            assert!(matches!(
                native.preview_relocation(selected, &locations, &Default::default())?,
                RelocationPreview::Unchanged
            ));
        }
        assert_eq!(store.load()?.ok_or("profile")?.original_bytes(), before);
        assert_eq!(
            fs::read(native.layout().root().join(NATIVE_STATE_FILE))?,
            state
        );
        assert!(matches!(
            native.preview_relocation(
                &roots.library().join("a new target"),
                &locations,
                &Default::default()
            )?,
            RelocationPreview::Destination(_)
        ));
        assert!(
            native
                .preview_relocation(&locations.native, &locations, &Default::default())
                .is_err()
        );
        db.read(|c| {
            c.query_row("SELECT 1", [], |r| r.get::<_, i64>(0))
                .map_err(StorageError::from)
        })?
        .wait()?;
        db.close()?;
        drop(db);
        native.finish().map_err(|f| f.to_string())?;
        Ok(())
    }

    #[test]
    fn closed_relocation_owner_retains_real_exclusion_allows_language_and_reopens_new_epoch()
    -> TestResult {
        let (_roots, copy, locations) = crate::test_support::native_copy()?;
        let store = NativeProfileStore::create_fresh(&locations.native)?;
        let native = NativeLibrary::bind_independent(copy, &store, &locations, &Default::default())
            .map_err(|failure| failure.to_string())?;
        let mut db = Database::open_native(&native)?;
        let before = db
            .read(|c| {
                c.query_row("SELECT COUNT(*) FROM stories", [], |r| r.get::<_, i64>(0))
                    .map_err(StorageError::from)
            })?
            .wait()?;
        db.close()?;
        drop(db);
        let closed = native
            .close_for_relocation()
            .map_err(|failure| failure.to_string())?;
        closed.revalidate()?;
        let manifest = closed.relocation_manifest(&WorkCancellation::default())?;
        assert_eq!(
            manifest
                .entries
                .iter()
                .filter(|entry| !entry.directory
                    && entry.path.first().is_some_and(|part| part == b"archives"))
                .count(),
            5
        );
        assert!(manifest.summary()?.bytes > 0);
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&fs::read(
                closed.layout().root().join(NATIVE_STATE_FILE)
            )?)?["state"],
            "closed"
        );
        assert!(
            LibraryLease::acquire_existing_in(
                closed.layout().root(),
                &Directory::open(closed.layout().root())?
            )
            .is_err()
        );
        store.update(|profile| {
            profile.language = "en".into();
            Ok(())
        })?;
        closed.revalidate()?;
        let native = closed.resume();
        let mut db = Database::open_native(&native)?;
        assert_eq!(
            db.read(|c| c
                .query_row("SELECT COUNT(*) FROM stories", [], |r| r.get::<_, i64>(0))
                .map_err(StorageError::from))?
                .wait()?,
            before
        );
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&fs::read(
                native.layout().root().join(NATIVE_STATE_FILE)
            )?)?["epoch"],
            2
        );
        db.close()?;
        drop(db);
        native.finish().map_err(|failure| failure.to_string())?;
        Ok(())
    }

    #[test]
    fn relocation_manifest_captures_all_managed_areas_and_refuses_shared_or_changed_inputs()
    -> TestResult {
        let (roots, copy, locations) = crate::test_support::native_copy()?;
        let store = NativeProfileStore::create_fresh(&locations.native)?;
        let native = NativeLibrary::bind_independent(copy, &store, &locations, &Default::default())
            .map_err(|f| f.to_string())?;
        let mut db = Database::open_native(&native)?;
        db.close()?;
        drop(db);
        fs::create_dir(native.layout().catalog().join("nested"))?;
        fs::write(
            native.layout().catalog().join("nested/retained-cache"),
            b"cache",
        )?;
        fs::write(native.layout().logs().join("retained-log"), b"log")?;
        let closed = native.close_for_relocation().map_err(|f| f.to_string())?;
        let manifest = closed.relocation_manifest(&Default::default())?;
        assert!(manifest.entries.iter().any(|entry| entry.path
            == [
                b"catalog".to_vec(),
                b"nested".to_vec(),
                b"retained-cache".to_vec()
            ]));
        assert!(
            manifest
                .entries
                .iter()
                .any(|entry| entry.path == [b"logs".to_vec(), b"retained-log".to_vec()])
        );
        assert!(
            manifest
                .entries
                .iter()
                .any(|entry| entry.path == [b"db".to_vec(), b"librairii.sqlite3".to_vec()])
        );
        let decoded: crate::settings::RelocationManifest =
            serde_json::from_slice(&manifest.bytes()?)?;
        assert_eq!(decoded, manifest);
        fs::hard_link(
            closed.layout().logs().join("retained-log"),
            roots.profile().join("shared-log"),
        )?;
        assert!(closed.relocation_manifest(&Default::default()).is_err());
        assert_eq!(fs::read(roots.profile().join("shared-log"))?, b"log");
        let cancel = WorkCancellation::default();
        cancel.cancel();
        assert!(matches!(
            closed.relocation_manifest(&cancel),
            Err(StorageError::Cancelled)
        ));
        Ok(())
    }

    #[test]
    fn unopened_library_cannot_create_relocation_or_release_profile_authority() -> TestResult {
        let (_roots, copy, locations) = crate::test_support::native_copy()?;
        let store = NativeProfileStore::create_fresh(&locations.native)?;
        let native = NativeLibrary::bind_independent(copy, &store, &locations, &Default::default())
            .map_err(|failure| failure.to_string())?;
        let Err(failure) = native.close_for_relocation() else {
            return Err("missing runtime close proof must refuse".into());
        };
        assert!(matches!(
            failure.storage_error(),
            StorageError::RuntimeFenced | StorageError::ProfileRestartRequired
        ));
        assert!(
            LibraryLease::acquire_existing_in(
                failure.retained_library().layout().root(),
                &Directory::open(failure.retained_library().layout().root())?
            )
            .is_err()
        );
        assert!(store.update(|_| Ok(())).is_err());
        Ok(())
    }
}

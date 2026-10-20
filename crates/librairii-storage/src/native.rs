//! Native profile binding of an actually owned independent library copy.
//!
//! The explicit native route borrows this owner through all SQLite joins. A
//! separate native namespace and retained verified copy supply its authority,
//! never TestRoots, profile DATA, a caller boolean or an in-place legacy lock.
//! Recovery replay and the owned threaded jobs bridge remain separate.

#[cfg(any(target_os = "macos", target_os = "linux"))]
mod owned {
    use crate::{
        LibraryIdentity, LibraryLayout, Result, StorageError,
        adoption::IndependentCopy,
        database::WorkCancellation,
        platform::unix::Directory,
        profile::{NativeProfileStore, ProfileBinding, ProfileLocations},
    };
    use std::{
        ffi::OsStr,
        fs,
        path::Path,
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, Ordering},
        },
    };

    /// A profile-bound independent copy. Construction consumes storage's actual
    /// retained root/parent handles and OS lease; paths, profile DATA, typed IDs
    /// or development authorization cannot substitute for that owned object.
    /// The borrowed store retains its own native namespace/marker OS lease.
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
        managed: Mutex<Option<Vec<(&'static str, Directory)>>>,
        runtime_active: AtomicBool,
        // Root objects/lease close before the profile-binding exclusion ends.
        copy: IndependentCopy,
        binding: ProfileBinding<'profile>,
    }

    impl<'profile> NativeLibrary<'profile> {
        /// Bind only the separately marked explicit native namespace and a
        /// verified NEW copy beneath the explicit native data location. Source
        /// bytes/legacy settings remain untouched. Any known copied recovery
        /// input or extra native recovery/config artifact is refused: this
        /// method does not infer cleanup authority from journal phase/path.
        ///
        /// Every failure returns the actual copy and keeps the profile store
        /// borrowed. Publication uncertainty remains uncertainty, with the
        /// store's real witnesses and poisoned cache retained until restart.
        pub fn bind_independent(
            copy: IndependentCopy,
            store: &'profile NativeProfileStore,
            locations: &ProfileLocations,
            cancellation: &WorkCancellation,
        ) -> std::result::Result<Self, Box<NativeBindingFailure<'profile>>> {
            let prepare = || -> Result<(LibraryLayout, Directory, Directory)> {
                if cancellation.is_cancelled() {
                    return Err(StorageError::Cancelled);
                }
                copy.revalidate(cancellation)?;
                copy.require_no_recovery_input()?;
                require_locations(&copy, store, locations)?;
                let layout = LibraryLayout::probe(copy.root())?;
                let root = copy.retain_root()?;
                let database = root.child(OsStr::new("db"))?;
                if cancellation.is_cancelled() {
                    return Err(StorageError::Cancelled);
                }
                Ok((layout, root, database))
            };
            let prepared = prepare().and_then(|(layout, root, database)| {
                let mut proposed = copy.source_profile().profile().clone();
                proposed.root = copy
                    .root()
                    .to_str()
                    .ok_or(StorageError::InvalidProfile)?
                    .to_owned();
                copy.identity().require_spelling(&proposed.library_id)?;
                // The sole durable root publication is the existing atomic
                // profile operation. No SQL/copy callback holds its mutex.
                let binding = store.bind_verified_copy(&proposed)?;
                Ok((layout, root, database, binding))
            });
            match prepared {
                Ok((layout, root, database, binding)) => Ok(Self {
                    layout,
                    root,
                    database,
                    managed: Mutex::new(None),
                    runtime_active: AtomicBool::new(false),
                    copy,
                    binding,
                }),
                Err(cause) => Err(Box::new(NativeBindingFailure { cause, copy, store })),
            }
        }
        pub fn layout(&self) -> &LibraryLayout {
            &self.layout
        }
        pub fn identity(&self) -> &LibraryIdentity {
            self.copy.identity()
        }
        /// Before first runtime admission verify the full closed copy. After
        /// SQLite starts, its DB bytes legitimately change; original root/marker,
        /// managed directory objects and the profile generation must still match.
        pub fn revalidate(&self, cancellation: &WorkCancellation) -> Result<()> {
            self.binding.revalidate()?;
            let result = (|| {
                let managed = self
                    .managed
                    .lock()
                    .map_err(|_| StorageError::RuntimeFenced)?;
                self.revalidate_database_ownership()?;
                match managed.as_ref() {
                    None => self.copy.revalidate(cancellation),
                    Some(directories) => {
                        for (name, directory) in directories {
                            if cancellation.is_cancelled() {
                                return Err(StorageError::Cancelled);
                            }
                            directory.matches_metadata(
                                &fs::symlink_metadata(self.layout.root().join(name)).map_err(
                                    |error| {
                                        StorageError::io(
                                            "revalidate original native directory",
                                            error,
                                        )
                                    },
                                )?,
                            )?;
                        }
                        Ok(())
                    }
                }
            })();
            match result {
                Ok(()) => Ok(()),
                // Work cancellation alone is not loss of ownership. A changed
                // copy is: retain exclusion instead of silently reactivating.
                Err(StorageError::Cancelled) => Err(StorageError::Cancelled),
                Err(error) => {
                    self.binding.fence();
                    Err(error)
                }
            }
        }
        pub(crate) fn effect_fence(&self) -> Arc<AtomicBool> {
            self.binding.fence_signal()
        }
        pub(crate) fn fence(&self) {
            self.binding.fence();
        }
        /// Required same-generation DB-only repair does not need a healthy
        /// settings cache or archive directory. Actual root/marker/DB-directory
        /// ownership still must hold; this cannot activate replacement names.
        pub(crate) fn revalidate_database_ownership(&self) -> Result<()> {
            self.copy.revalidate_root_lease()?;
            if LibraryIdentity::probe(self.layout.root())?.as_ref() != Some(self.identity()) {
                return Err(StorageError::IdentityMismatch);
            }
            self.database.matches_metadata(
                &fs::symlink_metadata(self.layout.root().join("db")).map_err(|error| {
                    StorageError::io("revalidate original native database directory", error)
                })?,
            )
        }
        pub(crate) fn claim_runtime(&self) -> Result<()> {
            // Claim before initialization; a competing open must not mistake
            // our own newly created directories for changed closed-copy input.
            self.runtime_active
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .map_err(|_| StorageError::RuntimeInUse)?;
            let result = (|| {
                self.binding.revalidate()?;
                self.revalidate_database_ownership()?;
                let mut managed = self
                    .managed
                    .lock()
                    .map_err(|_| StorageError::RuntimeFenced)?;
                if let Some(directories) = managed.as_ref() {
                    for (name, directory) in directories {
                        directory.matches_metadata(
                            &fs::symlink_metadata(self.layout.root().join(name)).map_err(
                                |error| {
                                    StorageError::io("revalidate native runtime directory", error)
                                },
                            )?,
                        )?;
                    }
                    return Ok(());
                }
                self.copy.revalidate(&WorkCancellation::default())?;
                // Keep every opened/created object if initialization later
                // fails; partial creation is not a clean failure or retry grant.
                *managed = Some(Vec::new());
                let directories = managed.as_mut().ok_or(StorageError::RuntimeFenced)?;
                for name in ["db", "archives", "catalog", "staging", "trash", "logs"] {
                    let directory = match self.root.child(OsStr::new(name)) {
                        Ok(directory) => directory,
                        Err(StorageError::Io { source, .. })
                            if source.kind() == std::io::ErrorKind::NotFound =>
                        {
                            self.root.create_child(OsStr::new(name))?
                        }
                        Err(error) => return Err(error),
                    };
                    directories.push((name, directory));
                }
                for (_, directory) in directories.iter() {
                    directory.flush()?;
                }
                self.root.flush()?;
                self.revalidate_database_ownership()
            })();
            if result.is_err() {
                self.fence();
            }
            result
        }
        pub(crate) fn release_runtime(&self) {
            self.runtime_active.store(false, Ordering::Release);
        }
    }

    impl Drop for NativeLibrary<'_> {
        fn drop(&mut self) {
            // A failed claim/close is not a settled replacement generation.
            // Safe borrowing means every Database has already joined before
            // this destructor can run; the profile exclusion still stays fenced.
            if self.runtime_active.load(Ordering::Acquire) {
                self.fence();
            }
        }
    }

    fn require_locations(
        copy: &IndependentCopy,
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
        let native = resolve(&locations.native)?;
        let store_root = resolve(store.directory())?;
        let data = resolve(&locations.data)?;
        let root = copy.root();
        if native != store_root
            || native.file_name() != Some(OsStr::new(native_name))
            || data.file_name() != Some(OsStr::new(data_name))
            || overlaps(&native, &data)
            || !root.starts_with(&data)
            || root == data
            || overlaps(&native, copy.source_root())
            || overlaps(&data, copy.source_root())
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

    /// Preserved copy and profile lifetime on binding failure. Neither the
    /// error nor a path observation authorizes deletion or blind retry.
    #[derive(Debug)]
    pub struct NativeBindingFailure<'profile> {
        cause: StorageError,
        copy: IndependentCopy,
        store: &'profile NativeProfileStore,
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
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
pub use owned::{NativeBindingFailure, NativeLibrary};

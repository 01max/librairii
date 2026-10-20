//! Native profile binding of an actually owned independent library copy.
//!
//! This phase retains the copy and both namespace/OS-lease lifetimes. It does
//! not open SQLite, replay a journal or implement the owned threaded jobs
//! bridge. Database::open_isolated cannot accept this authority. Native worker
//! activation/drain is a separate integration step, never a TestRoots cast.

#[cfg(any(target_os = "macos", target_os = "linux"))]
mod owned {
    use crate::{
        LibraryIdentity, LibraryLayout, Result, StorageError,
        adoption::IndependentCopy,
        database::WorkCancellation,
        profile::{NativeProfileStore, ProfileBinding, ProfileLocations},
    };
    use std::{fs, path::Path};

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
            let prepare = || -> Result<LibraryLayout> {
                if cancellation.is_cancelled() {
                    return Err(StorageError::Cancelled);
                }
                copy.revalidate(cancellation)?;
                copy.require_no_recovery_input()?;
                require_locations(&copy, store, locations)?;
                let layout = LibraryLayout::probe(copy.root())?;
                if cancellation.is_cancelled() {
                    return Err(StorageError::Cancelled);
                }
                Ok(layout)
            };
            let prepared = prepare().and_then(|layout| {
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
                Ok((layout, binding))
            });
            match prepared {
                Ok((layout, binding)) => Ok(Self {
                    layout,
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
        /// While this phase has no SQLite mutation API, verify the entire
        /// closed copy AND the active raw root/ID/recovery binding afresh.
        pub fn revalidate(&self, cancellation: &WorkCancellation) -> Result<()> {
            self.binding.revalidate()?;
            match self.copy.revalidate(cancellation) {
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
    }

    fn require_locations(
        copy: &IndependentCopy,
        store: &NativeProfileStore,
        locations: &ProfileLocations,
    ) -> Result<()> {
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

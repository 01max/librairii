//! Read-only startup decisions. No failed input falls back to initialization.
use crate::{
    LibraryIdentity, Result, StorageError,
    database::WorkCancellation,
    platform::unix::Directory,
    profile::{self, Profile, ProfileDocument, ProfileLocations},
};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub enum Startup {
    ConfirmFolder {
        suggested: PathBuf,
    },
    Native(ProfileDocument),
    Recovery {
        configured: Option<Profile>,
        cause: StorageError,
    },
}

/// Called after an explicit setup or language choice, never by startup
/// inspection or picker cancellation. Creates only native configuration/data
/// containers; a library identity and database still require confirmation.
pub fn prepare_native_profile(
    locations: &ProfileLocations,
    cancel: &WorkCancellation,
) -> Result<profile::NativeProfileStore> {
    use super::target::{physical_overlap, resolve};
    if cancel.is_cancelled() {
        return Err(StorageError::Cancelled);
    }
    locations.validate_native_names()?;
    validate_native_namespace(locations, cancel)?;
    let saved = profile::read_legacy(&locations.native)?;
    if let Some(document) = &saved {
        let p = document.profile();
        if p.recovering || (!p.complete && (!p.root.is_empty() || !p.library_id.is_empty())) {
            return Err(StorageError::RecoveryBindingRequired);
        }
        if p.complete {
            // Reopening preferences must not replace a missing data volume.
            return profile::NativeProfileStore::open_native(&locations.native);
        }
    }
    let native = resolve(&locations.native, true)?.0;
    let data = resolve(&locations.data, true)?.0;
    let (native_name, data_name, _) = ProfileLocations::native_names();
    if native.file_name() != Some(std::ffi::OsStr::new(native_name))
        || data.file_name() != Some(std::ffi::OsStr::new(data_name))
        || physical_overlap(&native, &data)?
    {
        return Err(StorageError::PathEscape);
    }
    // Old profiles and journals are not inputs to a fresh Rust installation.
    // Reserve their fixed namespaces without decoding their contents.
    let mut excluded = vec![default_legacy_root(locations)?];
    excluded.extend(locations.legacy.iter().cloned());
    for root in excluded {
        let root = resolve(&root, false)?.0;
        if physical_overlap(&root, &native)? || physical_overlap(&root, &data)? {
            return Err(StorageError::PathEscape);
        }
    }
    let (data_directory, _) = prepare_directory(&locations.data, cancel)?;
    let (namespace, parent) = prepare_directory(&locations.native, cancel)?;
    data_directory.matches_metadata(
        &fs::symlink_metadata(&locations.data)
            .map_err(|e| StorageError::io("revalidate prepared data container", e))?,
    )?;
    let store = if namespace.entries(4, cancel)?.is_empty() {
        profile::NativeProfileStore::create_prepared(&native, namespace, &parent, cancel)?
    } else {
        profile::NativeProfileStore::open_native(&native)?
    };
    store.load()?;
    Ok(store)
}

fn prepare_directory(path: &Path, cancel: &WorkCancellation) -> Result<(Directory, Directory)> {
    let (_, anchor, missing) = super::target::resolve(path, true)?;
    let mut parents = vec![(anchor.clone(), Directory::open(&anchor)?)];
    for name in missing {
        if cancel.is_cancelled() {
            return Err(StorageError::Cancelled);
        }
        let (path, parent) = parents.last().ok_or(StorageError::PathEscape)?;
        parent.matches_metadata(
            &fs::symlink_metadata(path)
                .map_err(|e| StorageError::io("revalidate setup parent", e))?,
        )?;
        let child = parent.create_child(&name)?;
        child.flush()?;
        parent.flush()?;
        parents.push((path.join(name), child));
    }
    for (path, directory) in &parents {
        directory.matches_metadata(
            &fs::symlink_metadata(path)
                .map_err(|e| StorageError::io("revalidate prepared setup directory", e))?,
        )?;
        if fs::canonicalize(path)
            .map_err(|e| StorageError::io("resolve prepared setup directory", e))?
            != *path
        {
            return Err(StorageError::SnapshotChanged);
        }
    }
    let (path, directory) = parents.pop().ok_or(StorageError::PathEscape)?;
    let parent = if let Some((_, parent)) = parents.pop() {
        parent
    } else {
        Directory::open(path.parent().ok_or(StorageError::PathEscape)?)?
    };
    Ok((directory, parent))
}

pub fn inspect_startup(locations: &ProfileLocations, cancellation: &WorkCancellation) -> Startup {
    let mut configured = None;
    let result = (|| -> Result<Startup> {
        if cancellation.is_cancelled() {
            return Err(StorageError::Cancelled);
        }
        if let Some(document) = profile::read_legacy(&locations.native)? {
            configured = Some(document.profile().clone());
            let p = document.profile();
            validate_native_namespace(locations, cancellation)?;
            if p.complete && !p.recovering {
                let root = Directory::open(Path::new(&p.root))?;
                let mut marker = root.open_regular(
                    std::ffi::OsStr::new(crate::identity::IDENTITY_FILE),
                    rustix::fs::OFlags::RDONLY,
                )?;
                let mut bytes = Vec::new();
                (&mut marker)
                    .take(1025)
                    .read_to_end(&mut bytes)
                    .map_err(|e| StorageError::io("read configured identity", e))?;
                LibraryIdentity::from_bytes(bytes)?.require_spelling(&p.library_id)?;
                root.child(std::ffi::OsStr::new("db"))?.open_regular(
                    std::ffi::OsStr::new("librairii.sqlite3"),
                    rustix::fs::OFlags::RDONLY,
                )?;
                return Ok(Startup::Native(document));
            }
            if p.recovering || !p.root.is_empty() || !p.library_id.is_empty() {
                return Err(StorageError::RecoveryBindingRequired);
            }
        }
        validate_native_namespace(locations, cancellation)?;
        Ok(Startup::ConfirmFolder {
            suggested: locations.data.join("library"),
        })
    })();
    result.unwrap_or_else(|cause| Startup::Recovery { configured, cause })
}
fn validate_native_namespace(
    locations: &ProfileLocations,
    cancel: &WorkCancellation,
) -> Result<()> {
    let root = match Directory::open(&locations.native) {
        Ok(root) => root,
        Err(StorageError::Io { source, .. }) if source.kind() == std::io::ErrorKind::NotFound => {
            return Ok(());
        }
        Err(e) => return Err(e),
    };
    let entries = root.entries(6, cancel)?;
    if entries.is_empty() {
        return Ok(());
    }
    if entries.iter().any(|name| {
        name != profile::MARKER
            && name != "settings.json"
            && name != "navigation.json"
            && name != "shelf-warnings.json"
            && name != "interface.json"
            && name != "handover.json"
    }) {
        return Err(StorageError::RecoveryBindingRequired);
    }
    root.regular_entry(std::ffi::OsStr::new("interface.json"))?;
    root.regular_entry(std::ffi::OsStr::new("handover.json"))?;
    let mut marker = root.open_regular(
        std::ffi::OsStr::new(profile::MARKER),
        rustix::fs::OFlags::RDONLY,
    )?;
    let mut bytes = Vec::new();
    (&mut marker)
        .take((profile::MARKER_BYTES.len() + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| StorageError::io("read bounded native namespace marker", e))?;
    if bytes != profile::MARKER_BYTES {
        return Err(StorageError::RecoveryBindingRequired);
    }
    Ok(())
}
pub fn default_legacy_root(locations: &ProfileLocations) -> Result<PathBuf> {
    let name = if cfg!(target_os = "macos") {
        "Librairii"
    } else {
        "librairii"
    };
    Ok(locations
        .data
        .parent()
        .ok_or(StorageError::InvalidProfile)?
        .join(name))
}

#[cfg(test)]
mod tests {
    use super::*;
    type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;
    #[test]
    fn cold_start_confirms_suggested_native_root_without_precreated_parents() -> TestResult {
        let roots = librairii_testkit::TestRoots::new()?;
        let locations =
            ProfileLocations::for_host(&roots.profile().join("new account"), None, None)?;
        let cancel = WorkCancellation::default();
        let Startup::ConfirmFolder { suggested } = inspect_startup(&locations, &cancel) else {
            return Err("fresh folder confirmation".into());
        };
        assert_eq!(suggested, locations.data.join("library"));
        assert_ne!(suggested, default_legacy_root(&locations)?);
        assert!(!locations.native.exists());
        assert!(!locations.data.exists());
        cancel.cancel();
        assert!(matches!(
            prepare_native_profile(&locations, &cancel),
            Err(StorageError::Cancelled)
        ));
        assert!(!locations.native.exists());
        assert!(!locations.data.exists());
        let cancel = WorkCancellation::default();
        let store = prepare_native_profile(&locations, &cancel)?;
        assert!(!suggested.exists());
        assert!(store.load()?.is_none());
        let target = super::super::TargetPreview::inspect(&suggested, &[], &cancel)?;
        let native = crate::native::NativeLibrary::initialize_confirmed(
            target,
            &store,
            &locations,
            "b0000000-0000-4000-8000-000000000013".parse()?,
            librairii_app::localization::Language::French,
            &cancel,
        )
        .map_err(|f| f.to_string())?;
        assert_eq!(native.layout().root(), suggested);
        let mut db = crate::database::Database::open_native(&native)?;
        db.close()?;
        drop(db);
        native.finish().map_err(|f| f.to_string())?;
        drop(store);
        let store = prepare_native_profile(&locations, &cancel)?;
        let id = "b0000000-0000-4000-8000-000000000013".parse()?;
        let mut preferences = profile::InterfacePreferences::new(id);
        preferences.sync_panel_hidden = true;
        preferences
            .dismissed_operations
            .insert("notice-identity-with-original-CASE".into());
        preferences
            .dismissed_operations
            .insert("another-notice".into());
        store.save_interface(&preferences)?;
        assert!(matches!(
            inspect_startup(&locations, &cancel),
            Startup::Native(_)
        ));
        assert_eq!(store.load_interface(id)?, Some(preferences));
        let saved = fs::read(locations.native.join("interface.json"))?;
        let mut too_many = store.load_interface(id)?.ok_or("interface preferences")?;
        too_many
            .dismissed_operations
            .extend((0..4097).map(|i| format!("notice-{i}")));
        assert!(store.save_interface(&too_many).is_err());
        assert_eq!(fs::read(locations.native.join("interface.json"))?, saved);
        let native = crate::native::NativeLibrary::reopen(&store, &locations, &cancel)?;
        let mut db = crate::database::Database::open_native(&native)?;
        db.close()?;
        drop(db);
        native.finish().map_err(|f| f.to_string())?;
        assert_eq!(store.load()?.ok_or("saved")?.profile().language, "fr");
        assert!(!default_legacy_root(&locations)?.exists());
        Ok(())
    }
    #[test]
    fn invalid_native_profile_is_preserved_before_setup() -> TestResult {
        let roots = librairii_testkit::TestRoots::new()?;
        let locations = ProfileLocations::for_host(&roots.profile().join("account"), None, None)?;
        fs::create_dir_all(&locations.native)?;
        fs::write(
            locations.native.join("settings.json"),
            b"keep invalid native",
        )?;
        assert!(prepare_native_profile(&locations, &Default::default()).is_err());
        assert_eq!(
            fs::read(locations.native.join("settings.json"))?,
            b"keep invalid native"
        );
        assert!(!locations.data.exists());
        Ok(())
    }
    #[test]
    fn startup_is_readonly_and_invalid_or_missing_profiles_never_silently_replace() -> TestResult {
        let (roots, _copy, locations) = crate::test_support::native_copy()?;
        assert!(matches!(
            inspect_startup(&locations, &Default::default()),
            Startup::ConfirmFolder { .. }
        ));
        assert!(!locations.native.exists());
        fs::create_dir(&locations.native)?;
        fs::write(locations.native.join("settings.json"), b"bad")?;
        assert!(matches!(
            inspect_startup(&locations, &Default::default()),
            Startup::Recovery { .. }
        ));
        assert_eq!(fs::read(locations.native.join("settings.json"))?, b"bad");
        let profile = Profile {
            version: 1,
            language: "fr".into(),
            root: roots
                .library()
                .join("disconnected")
                .to_str()
                .ok_or("root")?
                .into(),
            library_id: "a0000000-0000-4000-8000-000000000001".into(),
            complete: true,
            recovering: false,
        };
        fs::write(
            locations.native.join("settings.json"),
            serde_json::to_vec(&profile)?,
        )?;
        assert!(matches!(
            inspect_startup(&locations, &Default::default()),
            Startup::Recovery {
                configured: Some(_),
                ..
            }
        ));
        assert!(!Path::new(&profile.root).exists());
        Ok(())
    }
    #[test]
    fn fresh_setup_ignores_old_profiles_journals_and_database_and_reopens_its_own_state()
    -> TestResult {
        let roots = librairii_testkit::TestRoots::new()?;
        let locations = ProfileLocations::for_host(&roots.profile().join("account"), None, None)?;
        let legacy = locations.legacy.as_ref().ok_or("legacy namespace")?;
        let old_root = default_legacy_root(&locations)?;
        fs::create_dir_all(legacy)?;
        fs::create_dir_all(old_root.join("db"))?;
        let originals = [
            (
                legacy.join("settings.json"),
                b"invalid old settings".as_slice(),
            ),
            (
                legacy.join("relocation.json"),
                b"unfinished old move".as_slice(),
            ),
            (
                old_root.join("db/librairii.sqlite3"),
                b"old database".as_slice(),
            ),
        ];
        for (path, bytes) in &originals {
            fs::write(path, bytes)?;
        }
        let cancel = WorkCancellation::default();
        let Startup::ConfirmFolder { suggested } = inspect_startup(&locations, &cancel) else {
            return Err("old state blocked fresh setup".into());
        };
        assert_eq!(suggested, locations.data.join("library"));
        assert!(!locations.native.exists());
        let store = prepare_native_profile(&locations, &cancel)?;
        let target = super::super::TargetPreview::inspect(&suggested, &[], &cancel)?;
        let native = crate::native::NativeLibrary::initialize_confirmed(
            target,
            &store,
            &locations,
            "b0000000-0000-4000-8000-000000000013".parse()?,
            librairii_app::localization::Language::English,
            &cancel,
        )
        .map_err(|f| f.to_string())?;
        let mut db = crate::database::Database::open_native(&native)?;
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
        assert!(matches!(
            inspect_startup(&locations, &cancel),
            Startup::Native(_)
        ));
        let reopened = crate::native::NativeLibrary::reopen(&store, &locations, &cancel)?;
        reopened.finish().map_err(|f| f.to_string())?;
        for (path, bytes) in originals {
            assert_eq!(fs::read(path)?, bytes);
        }
        Ok(())
    }
}

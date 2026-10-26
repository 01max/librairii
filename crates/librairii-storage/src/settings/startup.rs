//! Read-only startup decisions. No failed input falls back to initialization.
use crate::{
    LibraryIdentity, Result, StorageError,
    adoption::{ClosedLibraryInput, ClosedManifest},
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
    AdoptIndependent(ClosedLibraryInput),
    Recovery {
        configured: Option<Profile>,
        cause: StorageError,
    },
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
        let default = default_legacy_root(locations)?;
        let legacy = locations
            .legacy
            .as_ref()
            .map(|path| profile::read_legacy(path))
            .transpose()?
            .flatten();
        let (root, language, document) = if let Some(document) = legacy {
            configured = Some(document.profile().clone());
            let p = document.profile();
            if p.recovering || (!p.complete && (!p.root.is_empty() || !p.library_id.is_empty())) {
                return Err(StorageError::RecoveryBindingRequired);
            }
            if p.complete {
                (PathBuf::from(&p.root), p.language.clone(), Some(document))
            } else {
                (default.clone(), p.language.clone(), None)
            }
        } else {
            (default.clone(), "en".into(), None)
        };
        match fs::read_dir(&root) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && document.is_none() => {
                return Ok(Startup::ConfirmFolder { suggested: default });
            }
            Err(e) => return Err(StorageError::io("inspect selected startup root", e)),
            Ok(mut entries) => {
                if entries.next().is_none() && document.is_none() {
                    return Ok(Startup::ConfirmFolder { suggested: default });
                }
            }
        }
        let directory = Directory::open(&root)?;
        let manifest = super::RelocationManifest::capture(&directory, cancellation)?;
        let mut entries = Vec::new();
        for entry in manifest.entries {
            let path = entry
                .path
                .iter()
                .map(|p| std::str::from_utf8(p).map_err(|_| StorageError::AdoptionInputInvalid))
                .collect::<Result<Vec<_>>>()?
                .join("/");
            entries.push(serde_json::json!({"path":path,"size":entry.bytes,"sha256":entry.sha256,"directory":entry.directory}));
        }
        let manifest = ClosedManifest::decode(
            &serde_json::to_vec(&entries).map_err(|_| StorageError::AdoptionInputInvalid)?,
        )?;
        let document = if let Some(document) = document {
            document
        } else {
            let mut marker = directory.open_regular(
                std::ffi::OsStr::new(crate::identity::IDENTITY_FILE),
                rustix::fs::OFlags::RDONLY,
            )?;
            let mut bytes = Vec::new();
            (&mut marker)
                .take(1025)
                .read_to_end(&mut bytes)
                .map_err(|e| StorageError::io("read bounded default identity", e))?;
            let identity = LibraryIdentity::from_bytes(bytes)?;
            ProfileDocument::decode(
                &serde_json::to_vec(&Profile {
                    version: 1,
                    language,
                    root: root.to_str().ok_or(StorageError::InvalidProfile)?.into(),
                    library_id: identity.spelling().into(),
                    complete: true,
                    recovering: false,
                })
                .map_err(|_| StorageError::InvalidProfile)?,
            )?
        };
        Ok(Startup::AdoptIndependent(ClosedLibraryInput::verify(
            &root,
            document,
            manifest,
            cancellation,
        )?))
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
    let entries = root.entries(4, cancel)?;
    if entries.is_empty() {
        return Ok(());
    }
    if entries
        .iter()
        .any(|name| name != profile::MARKER && name != "settings.json" && name != "navigation.json")
    {
        return Err(StorageError::RecoveryBindingRequired);
    }
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

#[derive(Debug)]
pub enum AdoptionSetupFailure<'profile> {
    Preparation(StorageError),
    Copy(Box<crate::adoption::IndependentCopyFailure>),
    Binding(Box<crate::native::NativeBindingFailure<'profile>>),
}
impl std::fmt::Display for AdoptionSetupFailure<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Preparation(e) => e.fmt(f),
            Self::Copy(e) => e.fmt(f),
            Self::Binding(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for AdoptionSetupFailure<'_> {}
/// Explicit clone confirmation only. The legacy input stays read-only; all
/// candidate writes use physically independent files and the native namespace.
pub fn confirm_adoption<'profile>(
    input: ClosedLibraryInput,
    mut target: super::TargetPreview,
    store: &'profile profile::NativeProfileStore,
    locations: &ProfileLocations,
    cancel: &WorkCancellation,
) -> std::result::Result<crate::native::NativeLibrary<'profile>, AdoptionSetupFailure<'profile>> {
    let prepared = (|| -> Result<()> {
        let default = default_legacy_root(locations)?;
        let mut excluded = vec![input.root(), locations.native.as_path(), default.as_path()];
        if let Some(legacy) = &locations.legacy {
            excluded.push(legacy);
        }
        target.protect(&excluded, cancel)?;
        target.check_writable(cancel)?;
        let mut parent = target.anchor.duplicate()?;
        let mut path = target.anchor_path.clone();
        for name in target
            .missing
            .iter()
            .take(target.missing.len().saturating_sub(1))
        {
            parent = parent.create_child(name)?;
            parent.flush()?;
            path.push(name);
        }
        parent.matches_metadata(
            &fs::symlink_metadata(&path)
                .map_err(|e| StorageError::io("revalidate adoption parent", e))?,
        )?;
        if let Some(existing) = &target.existing {
            parent.remove_empty_child(
                target
                    .missing
                    .last()
                    .ok_or(StorageError::SettingsDestination)?,
                existing,
            )?;
        }
        Ok(())
    })();
    prepared.map_err(AdoptionSetupFailure::Preparation)?;
    let copy = input
        .copy_to_new_root(target.resolved(), cancel)
        .map_err(AdoptionSetupFailure::Copy)?;
    crate::native::NativeLibrary::bind_selected_copy(copy, store, locations, cancel)
        .map_err(AdoptionSetupFailure::Binding)
}

#[cfg(test)]
mod tests {
    use super::*;
    type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;
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
    fn default_legacy_library_requires_confirmed_independent_clone_at_exact_choice() -> TestResult {
        let (roots, _copy, locations) = crate::test_support::native_copy()?;
        let default = default_legacy_root(&locations)?;
        fs::rename(roots.library().join("closed-source"), &default)?;
        let before = fs::read(default.join("db/librairii.sqlite3"))?;
        let Startup::AdoptIndependent(input) = inspect_startup(&locations, &Default::default())
        else {
            return Err("adoption offer".into());
        };
        assert_eq!(fs::read(default.join("db/librairii.sqlite3"))?, before);
        let store = profile::NativeProfileStore::create_fresh(&locations.native)?;
        let selected = roots.library().join("my selected clone");
        fs::create_dir(&selected)?;
        let preview = super::super::TargetPreview::inspect(&selected, &[], &Default::default())?;
        let native = confirm_adoption(input, preview, &store, &locations, &Default::default())
            .map_err(|f| f.to_string())?;
        assert_eq!(native.layout().root(), selected);
        let mut db = crate::database::Database::open_native(&native)?;
        db.close()?;
        drop(db);
        native.finish().map_err(|f| f.to_string())?;
        assert_eq!(fs::read(default.join("db/librairii.sqlite3"))?, before);
        let reopened =
            crate::native::NativeLibrary::reopen(&store, &locations, &Default::default())?;
        let mut db = crate::database::Database::open_native(&reopened)?;
        db.close()?;
        drop(db);
        reopened.finish().map_err(|f| f.to_string())?;
        Ok(())
    }
}

use librairii_core::LibraryId;
use librairii_storage::{
    IsolatedLibrary, LibraryIdentity, LibraryLayout, ManagedPath, StorageError,
};
use librairii_testkit::TestRoots;
use std::{error::Error, ffi::OsStr, fs, path::Path};

type TestResult = Result<(), Box<dyn Error>>;

fn identity() -> Result<LibraryId, librairii_core::BoundaryError> {
    "a0000000-0000-4000-8000-000000000001".parse()
}

#[test]
fn explicit_override_creates_exact_private_layout_without_profile_access() -> TestResult {
    let roots = TestRoots::new()?;
    let authority = roots.authorize(Some(roots.library()), Some(roots.profile()))?;
    fs::write(
        authority.profile().join("keep-profile"),
        b"untouched synthetic profile",
    )?;
    let root = authority.library().join("isolated");
    let isolated = IsolatedLibrary::initialize_from_override(Some(root.as_os_str()), identity()?)?;
    let layout = isolated.layout();
    assert_eq!(layout.root(), root);
    assert_eq!(layout.database(), root.join("db/librairii.sqlite3"));
    assert!(!layout.database().exists());
    assert!(!isolated.relocation_allowed());
    assert_eq!(layout.directories().len(), 7);
    for directory in layout.directories() {
        assert!(directory.is_dir());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(fs::metadata(directory)?.permissions().mode() & 0o077, 0);
        }
    }
    assert_eq!(
        fs::read(root.join(".librairii-library"))?,
        b"a0000000-0000-4000-8000-000000000001\n"
    );
    assert_eq!(
        fs::read(authority.profile().join("keep-profile"))?,
        b"untouched synthetic profile"
    );
    assert_eq!(fs::read_dir(authority.profile())?.count(), 1);
    Ok(())
}

#[test]
fn missing_relative_and_nonfresh_overrides_fail_without_mutation() -> TestResult {
    let roots = TestRoots::new()?;
    let authority = roots.authorize(Some(roots.library()), Some(roots.profile()))?;
    assert!(matches!(
        IsolatedLibrary::initialize_from_override(None, identity()?),
        Err(StorageError::MissingOverride)
    ));
    assert!(matches!(
        IsolatedLibrary::initialize_from_override(Some(OsStr::new("relative/data")), identity()?),
        Err(StorageError::RootNotAbsolute)
    ));
    let source = authority.library().join("existing");
    fs::create_dir(&source)?;
    fs::write(source.join("archive.zip"), b"existing immutable bytes")?;
    assert!(matches!(
        IsolatedLibrary::initialize_from_override(Some(source.as_os_str()), identity()?),
        Err(StorageError::RootNotFresh)
    ));
    assert_eq!(
        fs::read(source.join("archive.zip"))?,
        b"existing immutable bytes"
    );
    assert!(!source.join(".librairii-library").exists());
    let not_a_directory = authority.library().join("file");
    fs::write(&not_a_directory, b"keep")?;
    assert!(matches!(
        IsolatedLibrary::initialize_from_override(Some(not_a_directory.as_os_str()), identity()?),
        Err(StorageError::RootNotFresh)
    ));
    assert_eq!(fs::read(not_a_directory)?, b"keep");
    Ok(())
}

#[test]
fn identity_probe_preserves_raw_uuid_spelling_and_file_bytes() -> TestResult {
    let roots = TestRoots::new()?;
    let authority = roots.authorize(Some(roots.library()), Some(roots.profile()))?;
    let original = b"\t A0000000-0000-4000-8000-000000000001 \r\n";
    fs::write(authority.library().join(".librairii-library"), original)?;
    let observed = LibraryIdentity::probe(authority.library())?.ok_or("identity absent")?;
    assert_eq!(observed.original_bytes(), original);
    assert_eq!(observed.spelling(), "A0000000-0000-4000-8000-000000000001");
    assert_eq!(observed.typed(), identity()?);
    assert_eq!(
        observed.typed().to_string(),
        "a0000000-0000-4000-8000-000000000001"
    );
    observed.require_spelling("A0000000-0000-4000-8000-000000000001")?;
    assert!(matches!(
        observed.require_spelling("a0000000-0000-4000-8000-000000000001"),
        Err(StorageError::IdentityMismatch)
    ));
    assert_eq!(
        fs::read(authority.library().join(".librairii-library"))?,
        original
    );
    Ok(())
}

#[test]
fn absent_or_invalid_identity_is_never_silently_replaced() -> TestResult {
    let roots = TestRoots::new()?;
    let authority = roots.authorize(Some(roots.library()), Some(roots.profile()))?;
    assert!(LibraryIdentity::probe(authority.library())?.is_none());
    for bytes in [
        b"invalid UUID".as_slice(),
        b"\xff".as_slice(),
        b" \n".as_slice(),
    ] {
        fs::write(authority.library().join(".librairii-library"), bytes)?;
        assert!(matches!(
            LibraryIdentity::probe(authority.library()),
            Err(StorageError::InvalidIdentity)
        ));
        assert_eq!(
            fs::read(authority.library().join(".librairii-library"))?,
            bytes
        );
        assert!(matches!(
            IsolatedLibrary::initialize_from_override(
                Some(authority.library().as_os_str()),
                identity()?
            ),
            Err(StorageError::RootNotFresh)
        ));
        assert_eq!(
            fs::read(authority.library().join(".librairii-library"))?,
            bytes
        );
    }
    Ok(())
}

#[test]
fn managed_paths_retain_go_text_and_resolve_only_contained_components() -> TestResult {
    let roots = TestRoots::new()?;
    let authority = roots.authorize(Some(roots.library()), Some(roots.profile()))?;
    let isolated = IsolatedLibrary::initialize_from_override(
        Some(authority.library().as_os_str()),
        identity()?,
    )?;
    let raw = "archives/old/../Été 日本語.zip";
    let managed = ManagedPath::parse(raw)?;
    assert_eq!(managed.as_stored(), raw);
    assert_eq!(
        isolated.layout().resolve(&managed)?,
        authority.library().join("archives/Été 日本語.zip")
    );
    for bad in [
        "",
        ".",
        "..",
        "../outside",
        "a/../../outside",
        "/absolute",
        "archives\\file.zip",
        "archives/nul\0.zip",
    ] {
        assert!(
            matches!(
                ManagedPath::parse(bad),
                Err(StorageError::InvalidManagedPath)
            ),
            "{bad:?}"
        );
    }
    let composed = ManagedPath::parse("archives/Été.zip")?;
    let decomposed = ManagedPath::parse("archives/E\u{301}te\u{301}.zip")?;
    assert_ne!(
        composed.as_stored().as_bytes(),
        decomposed.as_stored().as_bytes()
    );
    assert_ne!(
        isolated.layout().resolve(&composed)?,
        isolated.layout().resolve(&decomposed)?
    );
    Ok(())
}

#[test]
fn failed_or_repeated_initialization_keeps_existing_identity_and_bytes() -> TestResult {
    let roots = TestRoots::new()?;
    let authority = roots.authorize(Some(roots.library()), Some(roots.profile()))?;
    let root = authority.library().join("isolated");
    let isolated = IsolatedLibrary::initialize_from_override(Some(root.as_os_str()), identity()?)?;
    fs::write(isolated.layout().archives().join("keep.zip"), b"unchanged")?;
    let before = fs::read(root.join(".librairii-library"))?;
    let other: LibraryId = "b0000000-0000-4000-8000-000000000001".parse()?;
    assert!(matches!(
        IsolatedLibrary::initialize_from_override(Some(root.as_os_str()), other),
        Err(StorageError::RootNotFresh)
    ));
    assert_eq!(fs::read(root.join(".librairii-library"))?, before);
    assert_eq!(
        fs::read(isolated.layout().archives().join("keep.zip"))?,
        b"unchanged"
    );
    let nested = authority.library().join("absent/child");
    let nested_library =
        IsolatedLibrary::initialize_from_override(Some(nested.as_os_str()), identity()?)?;
    assert_eq!(nested_library.layout().root(), nested);
    let blocker = authority.library().join("blocked");
    fs::write(&blocker, b"keep parent bytes")?;
    let missing_parent = blocker.join("child");
    assert!(
        IsolatedLibrary::initialize_from_override(Some(missing_parent.as_os_str()), identity()?)
            .is_err()
    );
    assert!(!missing_parent.exists());
    assert_eq!(fs::read(blocker)?, b"keep parent bytes");
    Ok(())
}

#[cfg(unix)]
#[test]
fn symlinked_root_identity_and_managed_ancestors_are_refused() -> TestResult {
    use std::os::unix::fs::symlink;
    let roots = TestRoots::new()?;
    let authority = roots.authorize(Some(roots.library()), Some(roots.profile()))?;
    fs::write(authority.profile().join("keep"), b"outside sentinel")?;
    let alias = authority.library().join("alias");
    symlink(authority.profile(), &alias)?;
    assert!(LibraryLayout::probe(&alias).is_err());
    assert!(
        IsolatedLibrary::initialize_from_override(Some(alias.as_os_str()), identity()?).is_err()
    );
    let isolated_root = authority.library().join("isolated");
    let isolated =
        IsolatedLibrary::initialize_from_override(Some(isolated_root.as_os_str()), identity()?)?;
    symlink(
        authority.profile(),
        isolated.layout().archives().join("outside"),
    )?;
    assert!(matches!(
        isolated
            .layout()
            .resolve(&ManagedPath::parse("archives/outside/keep")?),
        Err(StorageError::PathEscape)
    ));
    let identity_root = authority.library().join("identity-link");
    fs::create_dir(&identity_root)?;
    symlink(
        authority.profile().join("keep"),
        identity_root.join(".librairii-library"),
    )?;
    assert!(matches!(
        LibraryIdentity::probe(&identity_root),
        Err(StorageError::InvalidIdentity)
    ));
    assert_eq!(
        fs::read(authority.profile().join("keep"))?,
        b"outside sentinel"
    );
    Ok(())
}

#[cfg(target_os = "linux")]
#[test]
fn isolated_root_keeps_non_utf8_native_path_bytes() -> TestResult {
    use std::os::unix::ffi::{OsStrExt, OsStringExt};
    let roots = TestRoots::new()?;
    let authority = roots.authorize(Some(roots.library()), Some(roots.profile()))?;
    let root = authority
        .library()
        .join(std::ffi::OsString::from_vec(b"native-\xff".to_vec()));
    let isolated = IsolatedLibrary::initialize_from_override(Some(root.as_os_str()), identity()?)?;
    assert_eq!(
        isolated.layout().root().as_os_str().as_bytes(),
        root.as_os_str().as_bytes()
    );
    assert_eq!(
        LibraryIdentity::probe(&root)?
            .ok_or("identity absent")?
            .typed(),
        identity()?
    );
    Ok(())
}

#[cfg(target_os = "macos")]
#[test]
fn unrepresentable_native_root_bytes_fail_without_lossy_replacement() -> TestResult {
    use std::os::unix::ffi::{OsStrExt, OsStringExt};
    let roots = TestRoots::new()?;
    let authority = roots.authorize(Some(roots.library()), Some(roots.profile()))?;
    let root = authority
        .library()
        .join(std::ffi::OsString::from_vec(b"native-\xff".to_vec()));
    let original = root.as_os_str().as_bytes().to_vec();
    assert!(matches!(
        IsolatedLibrary::initialize_from_override(Some(root.as_os_str()), identity()?),
        Err(StorageError::Io { .. })
    ));
    assert_eq!(root.as_os_str().as_bytes(), original);
    assert_eq!(fs::read_dir(authority.library())?.count(), 0);
    assert_eq!(fs::read_dir(authority.profile())?.count(), 0);
    Ok(())
}

#[cfg(unix)]
#[test]
fn a_root_replaced_by_a_symlink_does_not_redirect_old_layout_paths() -> TestResult {
    use std::os::unix::fs::symlink;
    let roots = TestRoots::new()?;
    let authority = roots.authorize(Some(roots.library()), Some(roots.profile()))?;
    let root = authority.library().join("isolated");
    let isolated = IsolatedLibrary::initialize_from_override(Some(root.as_os_str()), identity()?)?;
    fs::rename(&root, authority.library().join("original-preserved"))?;
    symlink(authority.profile(), &root)?;
    assert!(matches!(
        isolated.layout().resolve(&ManagedPath::parse("keep")?),
        Err(StorageError::PathEscape)
    ));
    assert_eq!(fs::read_dir(authority.profile())?.count(), 0);
    Ok(())
}

#[test]
fn probing_never_initializes_an_absent_directory() -> TestResult {
    let roots = TestRoots::new()?;
    let authority = roots.authorize(Some(roots.library()), Some(roots.profile()))?;
    let missing = authority.library().join("absent");
    assert!(LibraryLayout::probe(&missing).is_err());
    assert!(LibraryIdentity::probe(&missing)?.is_none());
    assert!(!missing.exists());
    assert!(LibraryLayout::probe(Path::new("relative")).is_err());
    Ok(())
}

#[test]
fn concurrent_initializers_never_replace_the_winning_identity() -> TestResult {
    let roots = TestRoots::new()?;
    let authority = roots.authorize(Some(roots.library()), Some(roots.profile()))?;
    let root = authority.library().join("raced-initialization");
    let gate = std::sync::Barrier::new(8);
    let id = identity()?;
    let outcomes = std::thread::scope(|scope| -> Result<Vec<bool>, std::io::Error> {
        let mut handles = Vec::new();
        for _ in 0..8 {
            handles.push(scope.spawn(|| {
                gate.wait();
                IsolatedLibrary::initialize_from_override(Some(root.as_os_str()), id).is_ok()
            }));
        }
        handles
            .into_iter()
            .map(|handle| {
                handle
                    .join()
                    .map_err(|_| std::io::Error::other("initializer thread failed"))
            })
            .collect()
    })?;
    assert_eq!(outcomes.iter().filter(|success| **success).count(), 1);
    assert_eq!(
        LibraryIdentity::probe(&root)?
            .ok_or("identity absent")?
            .typed(),
        id
    );
    for directory in LibraryLayout::probe(&root)?.directories() {
        assert!(directory.is_dir());
    }
    Ok(())
}

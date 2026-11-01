use librairii_storage::{
    StorageError,
    portability::{ForeignProfile, ProfilePathSyntax},
};
use std::error::Error;
type TestResult = Result<(), Box<dyn Error>>;

fn declared_names(names: &[(&str, bool)]) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut entries: Vec<serde_json::Value> = serde_json::from_slice(include_bytes!(
        "../../../preservation/corpus/profiles/library-manifest.json"
    ))?;
    for (path, directory) in names {
        entries.push(if *directory {
            serde_json::json!({"path": path, "directory": true})
        } else {
            serde_json::json!({"path": path, "size": 0, "sha256": "0".repeat(64)})
        });
    }
    Ok(serde_json::to_vec(&entries)?)
}

#[test]
fn windows_name_diagnostics_retain_device_control_and_directory_spelling() -> TestResult {
    use librairii_storage::{
        adoption::ClosedManifest,
        portability::{WindowsNameProblems, windows_name_issues},
    };
    let names = [
        ("archives/CON.zip", false),
        ("archives/cOm¹.tar.gz", false),
        ("archives/LpT²", false),
        ("archives/COM³.txt ", false),
        ("archives/NUL", true),
        ("archives/NUL/unchanged.zip", false),
        ("archives/contains\\slash:*.zip", false),
        ("archives/control\u{1f}.zip.", false),
    ];
    let bytes = declared_names(&names)?;
    let manifest = ClosedManifest::decode(&bytes)?;
    let issues: Vec<_> = windows_name_issues(&manifest).collect();
    assert_eq!(issues.len(), names.len());
    for (issue, (path, _)) in issues.iter().zip(names) {
        assert_eq!(issue.raw_relative_path, path);
        assert_eq!(issue.component_index, 1);
        assert_eq!(
            issue.raw_component,
            path.split('/').nth(1).ok_or("component")?
        );
    }
    assert!(issues[..6].iter().all(|issue| issue.problems.device_name));
    assert!(issues[3].problems.trailing_space_or_dot);
    assert_eq!(
        issues[6].problems,
        WindowsNameProblems {
            reserved_character: true,
            ..WindowsNameProblems::default()
        }
    );
    assert_eq!(
        issues[7].problems,
        WindowsNameProblems {
            control_character: true,
            trailing_space_or_dot: true,
            ..WindowsNameProblems::default()
        }
    );
    assert_eq!(manifest.original_bytes(), bytes);
    // Declared data only: no source file or Windows constructor was needed,
    // and the repeated stream retains the same borrowed literal components.
    assert_eq!(windows_name_issues(&manifest).collect::<Vec<_>>(), issues);
    Ok(())
}

#[test]
fn filename_diagnostics_never_normalize_or_claim_filesystem_equivalence() -> TestResult {
    use librairii_storage::{adoption::ClosedManifest, portability::windows_name_issues};
    let bytes = declared_names(&[
        ("archives/Éte.zip", false),
        ("archives/E\u{301}te.zip", false),
        ("archives/Story.zip", false),
        ("archives/story.zip", false),
        ("archives/# %2F .zip", false),
        ("archives/COM0.zip", false),
        ("archives/COM10.zip", false),
        ("archives/COM¹¹.zip", false),
        ("archives/NULish.zip", false),
        ("archives/.hidden", false),
        ("archives/界COM1.zip", false),
        ("archives/🦀.zip", false),
    ])?;
    let manifest = ClosedManifest::decode(&bytes)?;
    // These names have no issue in this documented lexical subset. In
    // particular the case and decomposition pairs remain distinct DATA, not
    // a declaration that both files can coexist on a target filesystem.
    assert!(windows_name_issues(&manifest).next().is_none());
    assert_eq!(manifest.original_bytes(), bytes);
    let unchanged: Vec<serde_json::Value> = serde_json::from_slice(manifest.original_bytes())?;
    assert!(unchanged.iter().any(|e| e["path"] == "archives/Éte.zip"));
    assert!(
        unchanged
            .iter()
            .any(|e| e["path"] == "archives/E\u{301}te.zip")
    );
    assert!(unchanged.iter().any(|e| e["path"] == "archives/Story.zip"));
    assert!(unchanged.iter().any(|e| e["path"] == "archives/story.zip"));
    Ok(())
}

#[test]
fn foreign_profile_syntax_is_explicit_strict_and_retained_without_path_io() -> TestResult {
    let bytes = r#" {"version":1,"language":"fr","root":"Z:\\copied library\\Éte","libraryId":"A0000000-0000-4000-8000-000000000001","complete":true} "#.as_bytes();
    let profile = ForeignProfile::decode(bytes, ProfilePathSyntax::Windows)?;
    assert_eq!(profile.original_bytes(), bytes);
    assert_eq!(profile.profile().root, "Z:\\copied library\\Éte");
    assert_eq!(
        profile.profile().library_id,
        "A0000000-0000-4000-8000-000000000001"
    );
    assert_eq!(profile.syntax(), ProfilePathSyntax::Windows);
    assert!(matches!(
        ForeignProfile::decode(bytes, ProfilePathSyntax::Posix),
        Err(StorageError::InvalidProfile)
    ));
    let base: serde_json::Value = serde_json::from_slice(bytes)?;
    for (root, syntax, accepted) in [
        ("/foreign/not-materialized", ProfilePathSyntax::Posix, true),
        (
            "/foreign/not-materialized",
            ProfilePathSyntax::Windows,
            false,
        ),
        (
            "C:/foreign/not-materialized",
            ProfilePathSyntax::Windows,
            true,
        ),
        ("C:relative", ProfilePathSyntax::Windows, false),
        (
            "\\\\server\\share\\unknown",
            ProfilePathSyntax::Windows,
            true,
        ),
        ("\\\\server", ProfilePathSyntax::Windows, false),
        (
            "\\\\?\\C:\\device-namespace",
            ProfilePathSyntax::Windows,
            false,
        ),
        (
            "\\\\.\\pipe\\not-a-library",
            ProfilePathSyntax::Windows,
            false,
        ),
        ("relative", ProfilePathSyntax::Posix, false),
        ("/contains\0nul", ProfilePathSyntax::Posix, false),
    ] {
        let mut edited = base.clone();
        edited["root"] = root.into();
        assert_eq!(
            ForeignProfile::decode(&serde_json::to_vec(&edited)?, syntax).is_ok(),
            accepted,
            "{root}"
        );
    }
    for field in ["unknown", "version", "language"] {
        let mut edited = base.clone();
        match field {
            "unknown" => edited[field] = "must preserve without interpreting".into(),
            "version" => edited[field] = 2.into(),
            _ => edited[field] = "de".into(),
        }
        assert!(
            ForeignProfile::decode(&serde_json::to_vec(&edited)?, ProfilePathSyntax::Windows)
                .is_err()
        );
    }
    assert!(ForeignProfile::decode(&[bytes, b" {}"].concat(), ProfilePathSyntax::Windows).is_err());
    assert!(ForeignProfile::decode(&vec![b' '; 65537], ProfilePathSyntax::Windows).is_err());
    Ok(())
}

#[test]
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn profile_rebinding_borrows_verified_copy_keeps_raw_authority_and_never_replays_paths()
-> TestResult {
    use librairii_storage::{
        adoption::{ClosedLibraryInput, ClosedManifest},
        database::WorkCancellation,
        profile::{Profile, ProfileDocument},
    };
    use librairii_testkit::TestRoots;
    use std::{fs, path::Path};
    let roots = TestRoots::new()?;
    let source = roots.library().join("source");
    fs::create_dir(&source)?;
    let corpus =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../preservation/corpus/libraries/schema018");
    let bytes = include_bytes!("../../../preservation/corpus/profiles/library-manifest.json");
    let manifest: serde_json::Value = serde_json::from_slice(bytes)?;
    for entry in manifest.as_array().ok_or("manifest array")? {
        let path = entry["path"].as_str().ok_or("manifest path")?;
        if entry["directory"].as_bool() == Some(true) {
            fs::create_dir(source.join(path))?;
        } else {
            fs::copy(corpus.join(path), source.join(path))?;
        }
    }
    let local_input = Profile {
        version: 1,
        language: "fr".into(),
        root: source.to_str().ok_or("source UTF8")?.into(),
        library_id: "a0000000-0000-4000-8000-000000000001".into(),
        complete: true,
        recovering: false,
    };
    let input_profile = serde_json::to_vec(&local_input)?;
    let input = ClosedLibraryInput::verify(
        &source,
        ProfileDocument::decode(&input_profile)?,
        ClosedManifest::decode(bytes)?,
        &WorkCancellation::default(),
    )?;
    use librairii_storage::portability::{PathReferenceKind, relocation_path_references};
    let source_db = fs::read(source.join("db/librairii.sqlite3"))?;
    let mut references = Vec::new();
    input.visit_portability_paths(&WorkCancellation::default(), |reference| {
        references.push(reference);
        Ok(())
    })?;
    assert_eq!(references.len(), 2);
    assert_eq!(references[0].kind, PathReferenceKind::ProfileRoot);
    assert_eq!(
        references[0].raw_path,
        source.as_os_str().as_encoded_bytes()
    );
    assert_eq!(references[1].kind, PathReferenceKind::ExportDestination);
    assert_eq!(references[1].raw_path, b"/fixture/export");
    assert_eq!(
        references[1].owner_id,
        b"90000000-0000-4000-8000-000000000003"
    );
    assert_eq!(references[1].syntax(), Some(ProfilePathSyntax::Posix));
    let cancelled = WorkCancellation::default();
    cancelled.cancel();
    assert!(matches!(
        input.visit_portability_paths(&cancelled, |_| {
            Err(StorageError::CompatibilityInvariant)
        }),
        Err(StorageError::Cancelled)
    ));
    assert!(matches!(
        input.visit_portability_paths(&WorkCancellation::default(), |_| {
            Err(StorageError::CompatibilityInvariant)
        }),
        Err(StorageError::CompatibilityInvariant)
    ));
    assert_eq!(fs::read(source.join("db/librairii.sqlite3"))?, source_db);
    assert!(!source.join("db/librairii.sqlite3-wal").exists());
    assert!(!source.join("db/librairii.sqlite3-shm").exists());
    let journal_bytes = br#"{"id":"foreign-move","source":"Z:\\old-system\\source","destination":"/foreign/destination","entries":[{"path":"never replay"}]}"#;
    let journal = relocation_path_references(journal_bytes)?;
    assert_eq!(journal[0].kind, PathReferenceKind::RelocationSource);
    assert_eq!(journal[0].raw_path, br"Z:\old-system\source");
    assert_eq!(journal[0].syntax(), Some(ProfilePathSyntax::Windows));
    assert_eq!(journal[1].kind, PathReferenceKind::RelocationDestination);
    assert_eq!(journal[1].raw_path, b"/foreign/destination");
    assert_eq!(journal[1].syntax(), Some(ProfilePathSyntax::Posix));
    assert!(
        relocation_path_references(br#"{"id":"x","source":"/a","source":"/b","destination":"/c"}"#)
            .is_err()
    );
    assert!(relocation_path_references(&[journal_bytes.as_slice(), b" {}"].concat()).is_err());
    let target = roots.library().join("copied Éte # %2F");
    let copy = input.copy_to_new_root(&target, &WorkCancellation::default())?;
    let foreign_bytes = br#"{"version":1,"language":"fr","root":"Z:\\old-system\\library","libraryId":"a0000000-0000-4000-8000-000000000001","complete":true,"recovering":true}"#;
    let foreign = ForeignProfile::decode(foreign_bytes, ProfilePathSyntax::Windows)?;
    assert_eq!(
        foreign.root_path_reference().raw_path,
        br"Z:\old-system\library"
    );
    assert_eq!(
        foreign.root_path_reference().syntax(),
        Some(ProfilePathSyntax::Windows)
    );
    let rebinding = foreign.prepare_rebinding(&copy, &WorkCancellation::default())?;
    assert_eq!(rebinding.original().original_bytes(), foreign_bytes);
    let next = rebinding.proposed_document().profile();
    assert_eq!(next.root, copy.root().to_str().ok_or("copy UTF8")?);
    assert_eq!(next.library_id, foreign.profile().library_id);
    assert_eq!(
        (
            next.version,
            next.language.as_str(),
            next.complete,
            next.recovering
        ),
        (1, "fr", true, true)
    );
    assert_eq!(copy.source_profile().original_bytes(), input_profile);
    rebinding.revalidate(&WorkCancellation::default())?;
    assert_eq!(
        fs::read(source.join("history.json"))?,
        fs::read(target.join("history.json"))?
    );
    assert_eq!(
        fs::read(source.join("state.json"))?,
        fs::read(target.join("state.json"))?
    );
    for root in [&source, &target] {
        assert!(!root.join("settings.json").exists());
        assert!(!root.join("relocation.json").exists());
        assert!(!root.join("recovery").exists());
    }
    let mut wrong_id: serde_json::Value = serde_json::from_slice(foreign_bytes)?;
    wrong_id["libraryId"] = foreign.profile().library_id.to_ascii_uppercase().into();
    let wrong =
        ForeignProfile::decode(&serde_json::to_vec(&wrong_id)?, ProfilePathSyntax::Windows)?;
    assert!(matches!(
        wrong.prepare_rebinding(&copy, &WorkCancellation::default()),
        Err(StorageError::IdentityMismatch)
    ));
    let cancelled = WorkCancellation::default();
    cancelled.cancel();
    assert!(matches!(
        foreign.prepare_rebinding(&copy, &cancelled),
        Err(StorageError::Cancelled)
    ));

    // Applying a foreign profile requires an actual newly owned verified copy,
    // not the proposed DATA above. Recovery bytes/flags still grant no effects.
    use librairii_storage::{
        database::Database,
        native::NativeLibrary,
        profile::{NativeProfileStore, ProfileLocations},
    };
    let locations = ProfileLocations::for_host(
        &roots.profile().join("home"),
        Some(&roots.profile().join("config")),
        Some(&roots.library().join("data")),
    )?;
    fs::create_dir_all(&locations.data)?;
    fs::create_dir_all(locations.native.parent().ok_or("native parent")?)?;
    let store = NativeProfileStore::create_fresh(&locations.native)?;
    let cancel = WorkCancellation::default();
    let rejected_path = locations.data.join("refused-recovery");
    let rejected = input.copy_to_new_root(&rejected_path, &cancel)?;
    let failure = foreign
        .apply_rebinding(rejected, &store, &locations, &cancel)
        .err()
        .ok_or("recovering foreign profile activated")?;
    assert!(matches!(
        failure.storage_error(),
        StorageError::RecoveryBindingRequired
    ));
    assert_eq!(failure.retained_copy().root(), rejected_path);
    assert!(!failure.retains_native_record());
    assert!(store.load()?.is_none());
    assert!(!rejected_path.join(".librairii-native-state.json").exists());
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(rejected_path.join(".librairii-library.lock"))?;
    assert!(matches!(lock.try_lock(), Err(fs::TryLockError::WouldBlock)));
    drop(failure);

    let mut ready: serde_json::Value = serde_json::from_slice(foreign_bytes)?;
    ready["recovering"] = false.into();
    ready["language"] = "en".into();
    let ready_bytes = serde_json::to_vec(&ready)?;
    let ready = ForeignProfile::decode(&ready_bytes, ProfilePathSyntax::Windows)?;
    let mut wrong_ready: serde_json::Value = serde_json::from_slice(&ready_bytes)?;
    wrong_ready["libraryId"] = ready.profile().library_id.to_ascii_uppercase().into();
    let wrong_ready = ForeignProfile::decode(
        &serde_json::to_vec(&wrong_ready)?,
        ProfilePathSyntax::Windows,
    )?;
    let wrong_copy = input.copy_to_new_root(&locations.data.join("refused-raw-id"), &cancel)?;
    let failure = wrong_ready
        .apply_rebinding(wrong_copy, &store, &locations, &cancel)
        .err()
        .ok_or("different raw ID activated")?;
    assert!(matches!(
        failure.storage_error(),
        StorageError::IdentityMismatch
    ));
    assert!(!failure.retains_native_record());
    assert!(store.load()?.is_none());
    drop(failure);
    let relocated = locations.data.join("relocated Éte # %2F");
    let owned = input.copy_to_new_root(&relocated, &cancel)?;
    assert_eq!(owned.prefix().version(), 18);
    let original_db = fs::read(source.join("db/librairii.sqlite3"))?;
    let native = ready
        .apply_rebinding(owned, &store, &locations, &cancel)
        .map_err(|f| f.to_string())?;
    let published = store.load()?.ok_or("missing applied profile")?;
    assert_eq!(
        published.profile().root,
        relocated.to_str().ok_or("relocated root")?
    );
    assert_eq!(published.profile().library_id, ready.profile().library_id);
    assert_eq!(published.profile().language, ready.profile().language);
    assert!(!published.profile().recovering);
    assert_eq!(ready.original_bytes(), ready_bytes);
    let mut db = Database::open_native(&native)?;
    db.close()?;
    drop(db);
    native.finish().map_err(|f| f.to_string())?;
    drop(store);

    // Successful checked restart uses persisted native binding+Closed record,
    // fresh actual leases/IDs and the same worker close/finalizer protocol.
    let store = NativeProfileStore::open_native(&locations.native)?;
    let native = NativeLibrary::reopen(&store, &locations, &cancel)?;
    assert_eq!(native.layout().root(), relocated);
    assert_eq!(native.identity().spelling(), ready.profile().library_id);
    let mut db = Database::open_native(&native)?;
    db.close()?;
    drop(db);
    native.finish().map_err(|f| f.to_string())?;
    let closed: serde_json::Value =
        serde_json::from_slice(&fs::read(relocated.join(".librairii-native-state.json"))?)?;
    assert_eq!(closed["state"], "closed");
    assert_eq!(closed["epoch"], 2);
    let mut archive_count = 0;
    for entry in manifest.as_array().ok_or("manifest array")? {
        let path = entry["path"].as_str().ok_or("manifest path")?;
        if path.starts_with("archives/") && entry["directory"].as_bool() != Some(true) {
            archive_count += 1;
            let expected: librairii_core::Checksum =
                entry["sha256"].as_str().ok_or("archive hash")?.parse()?;
            let mut file = fs::File::open(relocated.join(path))?;
            let digest = librairii_storage::platform::hash_stream(
                &mut file,
                entry["size"].as_u64().ok_or("archive bytes")?,
                &cancel,
            )?;
            assert_eq!(
                digest.checksum(),
                expected,
                "schema018 managed archive changed: {path}"
            );
        }
    }
    assert!(
        archive_count > 0,
        "schema018 archive acceptance must not be vacuous"
    );
    for path in ["history.json", "state.json", ".librairii-library"] {
        assert_eq!(
            fs::read(source.join(path))?,
            fs::read(relocated.join(path))?
        );
    }
    assert_eq!(fs::read(source.join("db/librairii.sqlite3"))?, original_db);
    assert_eq!(input.profile().original_bytes(), input_profile);
    assert!(!source.join(".librairii-library.lock").exists());
    assert!(!source.join(".librairii-native-state.json").exists());

    fs::write(
        target.join("unowned-foreign-journal"),
        b"never follow/remove this",
    )?;
    assert!(rebinding.revalidate(&WorkCancellation::default()).is_err());
    assert!(
        foreign
            .prepare_rebinding(&copy, &WorkCancellation::default())
            .is_err()
    );
    assert_eq!(
        fs::read(target.join("unowned-foreign-journal"))?,
        b"never follow/remove this"
    );
    assert_eq!(foreign.original_bytes(), foreign_bytes);
    Ok(())
}

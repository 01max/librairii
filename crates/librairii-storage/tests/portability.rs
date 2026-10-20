use librairii_storage::{
    StorageError,
    portability::{ForeignProfile, ProfilePathSyntax},
};
use std::error::Error;
type TestResult = Result<(), Box<dyn Error>>;

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
    let target = roots.library().join("copied Éte # %2F");
    let copy = input.copy_to_new_root(&target, &WorkCancellation::default())?;
    let foreign_bytes = br#"{"version":1,"language":"fr","root":"Z:\\old-system\\library","libraryId":"a0000000-0000-4000-8000-000000000001","complete":true,"recovering":true}"#;
    let foreign = ForeignProfile::decode(foreign_bytes, ProfilePathSyntax::Windows)?;
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

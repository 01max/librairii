#![cfg(any(target_os = "macos", target_os = "linux"))]
use crate::{
    IsolatedLibrary, StorageError,
    database::WorkCancellation,
    platform::{
        hash_stream,
        unix::{EntryName, LibraryFiles, PickedSource, Publication},
    },
};
use librairii_testkit::TestRoots;
use std::{
    error::Error,
    ffi::OsString,
    fs,
    os::unix::{ffi::OsStringExt, fs::symlink},
};
type TestResult = Result<(), Box<dyn Error>>;
#[test]
fn selected_source_streams_into_content_addressed_original_name_without_mutation() -> TestResult {
    let names = std::iter::once(OsString::from("Résumé original.PK")).chain(
        cfg!(target_os = "linux").then(|| OsString::from_vec(b"raw-\xff-original.pk".to_vec())),
    );
    for name in names {
        let roots = TestRoots::new()?;
        let library = IsolatedLibrary::initialize_from_override(
            Some(roots.library().join("data").as_os_str()),
            "a0000000-0000-4000-8000-000000000051".parse()?,
        )?;
        let input = roots.profile().join(&name);
        let payload: Vec<_> = (0..200_003).map(|n| (n % 251) as u8).collect();
        fs::write(&input, &payload)?;
        let mut source = PickedSource::open(&input)?;
        let files = LibraryFiles::open(&library)?;
        let guard = files.try_custody()?;
        let mut stage = files
            .create_stage(&guard)
            .map_err(|_| "fixture stage creation failed")?;
        let cancel = WorkCancellation::default();
        let measured = source.copy_into(&mut stage, &cancel)?;
        assert_eq!(
            measured,
            hash_stream(&mut payload.as_slice(), payload.len() as u64, &cancel)?
        );
        assert_eq!(source.name().as_os_str(), name);
        let publication = stage
            .publish_archive(&guard, EntryName::parse(name.clone())?, measured, &cancel)
            .map_err(|f| f.cause)?;
        assert!(matches!(publication, Publication::NewOwned(_)));
        let managed = library
            .layout()
            .archives()
            .join(measured.checksum().to_string())
            .join(&name);
        assert_eq!(fs::read(&managed)?, payload);
        assert_eq!(fs::read(&input)?, payload);
        source.revalidate()?;
        publication
            .finish_metadata_stage(&guard)
            .map_err(|f| f.cause)?;
        assert_eq!(fs::read_dir(library.layout().staging())?.count(), 0);
        // Crash-left exact bytes can be reused without replacement on retry.
        let mut stage = files
            .create_stage(&guard)
            .map_err(|_| "fixture stage creation failed")?;
        let retry = source.copy_into(&mut stage, &cancel)?;
        let publication = stage
            .publish_archive(&guard, EntryName::parse(name)?, retry, &cancel)
            .map_err(|f| f.cause)?;
        assert!(matches!(publication, Publication::ReusedExisting { .. }));
        publication
            .finish_metadata_stage(&guard)
            .map_err(|f| f.cause)?;
        assert_eq!(fs::read(managed)?, payload);
    }
    Ok(())
}
#[test]
fn selected_aliases_are_read_only_and_source_substitution_is_refused() -> TestResult {
    let roots = TestRoots::new()?;
    let source = roots.profile().join("original.zip");
    let alias = roots.profile().join("selected.zip");
    fs::write(&source, b"original bytes")?;
    symlink(&source, &alias)?;
    let picked = PickedSource::open(&alias)?;
    assert_eq!(picked.name().as_os_str(), "selected.zip");
    picked.revalidate()?;
    fs::remove_file(&alias)?;
    fs::write(&alias, b"substituted")?;
    assert!(matches!(
        picked.revalidate(),
        Err(StorageError::SnapshotChanged)
    ));
    assert_eq!(fs::read(source)?, b"original bytes");
    assert!(PickedSource::open(roots.profile()).is_err());
    Ok(())
}
#[test]
fn cancelled_copy_retains_owned_stage_and_never_publishes_or_changes_source() -> TestResult {
    let roots = TestRoots::new()?;
    let library = IsolatedLibrary::initialize_from_override(
        Some(roots.library().join("data").as_os_str()),
        "a0000000-0000-4000-8000-000000000052".parse()?,
    )?;
    let path = roots.profile().join("source.zip");
    fs::write(&path, b"source bytes")?;
    let mut source = PickedSource::open(&path)?;
    let files = LibraryFiles::open(&library)?;
    let guard = files.try_custody()?;
    let mut stage = files
        .create_stage(&guard)
        .map_err(|_| "fixture stage creation failed")?;
    let cancel = WorkCancellation::default();
    cancel.cancel();
    assert!(matches!(
        source.copy_into(&mut stage, &cancel),
        Err(StorageError::Cancelled)
    ));
    assert_eq!(fs::read_dir(library.layout().archives())?.count(), 0);
    assert_eq!(fs::read(&path)?, b"source bytes");
    stage.retire_owned_temp(&guard).map_err(|f| f.cause)?;
    assert_eq!(fs::read_dir(library.layout().staging())?.count(), 0);
    Ok(())
}

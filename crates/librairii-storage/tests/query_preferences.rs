#![cfg(unix)]
use librairii_core::{
    collection_codec::url::{Location, Scope},
    navigation::{Navigation, Presentation},
    query::QueryRequest,
};
use librairii_storage::{StorageError, profile::NativeProfileStore};
use librairii_testkit::TestRoots;
use std::{error::Error, fs};
type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn navigation_preferences_reopen_outside_the_library_and_preserve_unknown_versions() -> TestResult {
    let roots = TestRoots::new()?;
    let id = "10000000-0000-4000-8000-000000000001".parse()?;
    let store = NativeProfileStore::create_fresh(roots.profile())?;
    let mut nav = Navigation::new(
        id,
        1,
        Location {
            scope: Scope::Library,
            request: QueryRequest::default(),
        },
    )?;
    nav.select(vec![7, 4])?;
    store.save_navigation(&nav)?;
    nav.set_presentation(Presentation::SmallCovers);
    store.save_navigation(&nav)?;
    assert!(!roots.library().join("navigation.json").exists());
    drop(store);
    let store = NativeProfileStore::open_native(roots.profile())?;
    let restored = store.load_navigation(id, 2)?.ok_or("missing preferences")?;
    assert_eq!(restored.current(), nav.current());
    assert_eq!(restored.presentation(), Presentation::SmallCovers);
    assert!(matches!(
        store.load_navigation("20000000-0000-4000-8000-000000000001".parse()?, 2),
        Err(StorageError::InvalidNavigation)
    ));
    drop(store);
    let path = roots.profile().join("navigation.json");
    let mut wire: serde_json::Value = serde_json::from_slice(&fs::read(&path)?)?;
    wire["version"] = 99.into();
    let unknown = serde_json::to_vec(&wire)?;
    fs::write(&path, &unknown)?;
    let store = NativeProfileStore::open_native(roots.profile())?;
    assert!(store.load_navigation(id, 3).is_err());
    assert!(store.save_navigation(&nav).is_err());
    assert_eq!(fs::read(path)?, unknown);
    Ok(())
}

#[test]
fn navigation_preferences_refuse_symlink_substitution_and_preserve_external_bytes() -> TestResult {
    use std::os::unix::fs::symlink;
    let roots = TestRoots::new()?;
    let store = NativeProfileStore::create_fresh(roots.profile())?;
    let external = roots.library().join("external");
    fs::write(&external, b"keep")?;
    symlink(&external, roots.profile().join("navigation.json"))?;
    let id = "10000000-0000-4000-8000-000000000001".parse()?;
    let nav = Navigation::new(
        id,
        1,
        Location {
            scope: Scope::Library,
            request: QueryRequest::default(),
        },
    )?;
    assert!(store.load_navigation(id, 1).is_err());
    assert!(store.save_navigation(&nav).is_err());
    assert_eq!(fs::read(external)?, b"keep");
    Ok(())
}

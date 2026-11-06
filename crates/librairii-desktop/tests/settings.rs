#![cfg(any(target_os = "macos", target_os = "linux"))]
use librairii_app::localization::Language;
use librairii_desktop::settings::{change_language, restore_language};
use librairii_storage::{StorageError, profile::NativeProfileStore};
use librairii_testkit::TestRoots;
use std::{error::Error, fs};
type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn language_persistence_updates_existing_projection_and_pending_native_menus() -> TestResult {
    let roots = TestRoots::new()?;
    let path = roots.profile().join("settings");
    let store = NativeProfileStore::create_fresh(&path)?;
    let mut view = restore_language(&store, Language::French)?;
    assert_eq!(view.current().language, Language::French);
    view.request_open();
    view.request_open();
    assert!(!view.take_open_request());
    assert!(view.take_menu_change().is_none());
    change_language(&store, &mut view, "en")?;
    change_language(&store, &mut view, "fr")?;
    assert_eq!(
        view.text("Welcome to Librairii", &[])?,
        "Bienvenue dans Librairii"
    );
    view.native_ready();
    let menu = view.take_menu_change().ok_or("menu revision")?;
    assert_eq!(menu.revision, 2);
    assert_eq!(menu.settings, "Réglages…");
    assert!(view.take_menu_change().is_none());
    assert!(view.take_open_request());
    assert!(!view.take_open_request());
    let saved = store.load()?.ok_or("saved settings")?;
    assert_eq!(saved.profile().language, "fr");
    assert!(!saved.profile().complete);
    assert!(saved.profile().root.is_empty());
    drop(store);
    let reopened = NativeProfileStore::open_native(&path)?;
    assert_eq!(
        restore_language(&reopened, Language::English)?
            .current()
            .language,
        Language::French
    );
    Ok(())
}

#[test]
fn rejected_or_failed_language_updates_preserve_effective_text_and_durable_bytes() -> TestResult {
    let roots = TestRoots::new()?;
    let path = roots.profile().join("settings");
    let store = NativeProfileStore::create_fresh(&path)?;
    store.update(|_| Ok(()))?;
    let before = fs::read(path.join("settings.json"))?;
    let mut view = restore_language(&store, Language::English)?;
    view.native_ready();
    view.take_menu_change();
    assert!(matches!(
        change_language(&store, &mut view, "de"),
        Err(StorageError::InvalidProfile)
    ));
    assert_eq!(fs::read(path.join("settings.json"))?, before);
    // Actual on-disk substitution is a failed persistence boundary, not a fake
    // callback success. The displaced original and substituted bytes survive.
    fs::rename(path.join("settings.json"), path.join("saved-settings"))?;
    fs::write(
        path.join("settings.json"),
        b"changed outside the settings owner",
    )?;
    assert!(change_language(&store, &mut view, "fr").is_err());
    assert_eq!(view.current().language, Language::English);
    assert_eq!(view.current().revision, 0);
    assert!(view.take_menu_change().is_none());
    assert_eq!(fs::read(path.join("saved-settings"))?, before);
    assert_eq!(
        fs::read(path.join("settings.json"))?,
        b"changed outside the settings owner"
    );
    Ok(())
}

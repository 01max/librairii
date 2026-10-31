#![cfg(unix)]
use librairii_app::localization::Language;
use librairii_core::{
    LibraryId,
    collection_codec::url::{Location, Scope},
    navigation::{Navigation, Presentation},
};
use librairii_storage::{
    native::NativeLibrary,
    profile::{HandoverImport, InterfacePreferences, NativeProfileStore, ProfileLocations},
    settings::TargetPreview,
};
use librairii_testkit::TestRoots;
use std::{collections::BTreeMap, error::Error, fs};

type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;
const FIXTURE: &[u8] = include_bytes!("../../librairii-core/tests/fixtures/handover-v1.json");

fn prepare(roots: &TestRoots) -> Result<(NativeProfileStore, LibraryId)> {
    let locations = ProfileLocations::for_host(
        &roots.profile().join("home"),
        Some(roots.profile()),
        Some(&roots.profile().join("data")),
    )?;
    fs::create_dir_all(&locations.data)?;
    fs::create_dir_all(locations.native.parent().ok_or("native parent")?)?;
    let store = NativeProfileStore::create_fresh(&locations.native)?;
    let id = "f1000000-0000-4000-8000-000000000017".parse()?;
    let native = NativeLibrary::initialize_confirmed(
        TargetPreview::inspect(roots.library(), &[], &Default::default())?,
        &store,
        &locations,
        id,
        Language::French,
        &Default::default(),
    )
    .map_err(|e| e.to_string())?;
    let mut database = librairii_storage::database::Database::open_native(&native)?;
    database.close()?;
    drop(database);
    native.finish().map_err(|e| e.to_string())?;
    Ok((store, id))
}
fn bytes(store: &NativeProfileStore) -> Result<BTreeMap<String, Vec<u8>>> {
    fs::read_dir(store.directory())?
        .map(|entry| {
            let entry = entry?;
            Ok((
                entry.file_name().to_string_lossy().into_owned(),
                fs::read(entry.path())?,
            ))
        })
        .collect()
}

#[test]
fn one_atomic_import_receipt_survives_restart_and_never_resets_newer_choices() -> Result {
    let roots = TestRoots::new()?;
    let (store, id) = prepare(&roots)?;
    assert!(store.load_navigation(id, 1)?.is_none());
    assert!(store.load_interface(id)?.is_none());
    let selected = roots.profile().join("selected preferences.json");
    fs::write(&selected, FIXTURE)?;
    assert_eq!(
        store.import_handover_file(&selected, id)?,
        HandoverImport::Imported
    );
    let imported = store
        .load_navigation(id, 1)?
        .ok_or("missing imported navigation")?;
    assert_eq!(imported.presentation(), Presentation::SmallCovers);
    assert!(matches!(imported.current().location.scope,Scope::Collection(group) if group.get()==7));
    assert!(
        store
            .load_interface(id)?
            .ok_or("missing interface")?
            .sync_panel_hidden
    );
    let mut newer = Navigation::new(
        id,
        1,
        Location {
            scope: Scope::Library,
            request: Default::default(),
        },
    )?;
    newer.set_presentation(Presentation::LargeCovers);
    store.save_navigation(&newer)?;
    store.save_interface(&InterfacePreferences::new(id))?;
    let receipt = fs::read(store.directory().join("handover.json"))?;
    assert_eq!(
        store.import_handover(FIXTURE, id)?,
        HandoverImport::AlreadyImported
    );
    let changed = std::str::from_utf8(FIXTURE)?.replace("small_covers", "list");
    assert_eq!(
        store.import_handover(changed.as_bytes(), id)?,
        HandoverImport::AlreadyImported
    );
    assert_eq!(fs::read(store.directory().join("handover.json"))?, receipt);
    let directory = store.directory().to_path_buf();
    drop(store);
    let reopened = NativeProfileStore::open_native(&directory)?;
    assert_eq!(
        reopened
            .load_navigation(id, 2)?
            .ok_or("missing newer choices")?
            .presentation(),
        Presentation::LargeCovers
    );
    assert!(
        !reopened
            .load_interface(id)?
            .ok_or("missing interface")?
            .sync_panel_hidden
    );
    Ok(())
}

#[test]
fn first_transfer_keeps_already_saved_native_preferences() -> Result {
    let roots = TestRoots::new()?;
    let (store, id) = prepare(&roots)?;
    let navigation = Navigation::new(
        id,
        1,
        Location {
            scope: Scope::Library,
            request: Default::default(),
        },
    )?;
    store.save_navigation(&navigation)?;
    store.save_interface(&InterfacePreferences::new(id))?;
    let nav = fs::read(store.directory().join("navigation.json"))?;
    let interface = fs::read(store.directory().join("interface.json"))?;
    assert_eq!(
        store.import_handover(FIXTURE, id)?,
        HandoverImport::ImportedPreservingNativeChoices
    );
    assert_eq!(fs::read(store.directory().join("navigation.json"))?, nav);
    assert_eq!(
        fs::read(store.directory().join("interface.json"))?,
        interface
    );
    assert_eq!(
        store
            .load_navigation(id, 1)?
            .ok_or("missing navigation")?
            .presentation(),
        Presentation::List
    );
    assert!(
        !store
            .load_interface(id)?
            .ok_or("missing interface")?
            .sync_panel_hidden
    );
    Ok(())
}

#[test]
fn wrong_library_or_invalid_transfer_preserves_every_native_profile_byte() -> Result {
    let roots = TestRoots::new()?;
    let (store, id) = prepare(&roots)?;
    let before = bytes(&store)?;
    let other: LibraryId = "f1000000-0000-4000-8000-000000000018".parse()?;
    assert!(store.import_handover(FIXTURE, other).is_err());
    for invalid in [b"{}".as_slice(), b"{corrupt", b"null"] {
        assert!(store.import_handover(invalid, id).is_err());
    }
    assert_eq!(bytes(&store)?, before);
    let document = std::str::from_utf8(FIXTURE)?.replace(&id.to_string(), &other.to_string());
    assert!(store.import_handover(document.as_bytes(), other).is_err());
    assert_eq!(bytes(&store)?, before);
    Ok(())
}

#[test]
fn future_receipt_or_aliased_receipt_is_never_overwritten() -> Result {
    for alias in [false, true] {
        let roots = TestRoots::new()?;
        let (store, id) = prepare(&roots)?;
        let path = store.directory().join("handover.json");
        let foreign = roots.profile().join("preserved-transfer");
        fs::write(&foreign, b"future or foreign receipt")?;
        if alias {
            std::os::unix::fs::symlink(&foreign, &path)?;
        } else {
            fs::write(&path, b"{\"version\":99}")?;
        }
        assert!(store.import_handover(FIXTURE, id).is_err());
        assert_eq!(fs::read(&foreign)?, b"future or foreign receipt");
        if alias {
            assert!(path.is_symlink());
        } else {
            assert_eq!(fs::read(&path)?, b"{\"version\":99}");
        }
    }
    Ok(())
}

#[test]
fn selected_transfer_refuses_links_directories_pipes_and_oversized_files_without_effects() -> Result
{
    let roots = TestRoots::new()?;
    let (store, id) = prepare(&roots)?;
    let before = bytes(&store)?;
    let selected = roots.profile().join("selected.json");
    fs::write(&selected, FIXTURE)?;
    let alias = roots.profile().join("alias.json");
    std::os::unix::fs::symlink(&selected, &alias)?;
    let oversized = roots.profile().join("large.json");
    fs::File::create(&oversized)?
        .set_len(librairii_core::navigation::MAX_STATE_BYTES as u64 + 1)?;
    let pipe = roots.profile().join("pipe.json");
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&pipe)
            .status()?
            .success()
    );
    for path in [&alias, roots.profile(), &oversized, &pipe] {
        assert!(store.import_handover_file(path, id).is_err());
        assert_eq!(bytes(&store)?, before);
    }
    assert_eq!(fs::read(selected)?, FIXTURE);
    Ok(())
}

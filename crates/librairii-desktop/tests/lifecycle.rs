#![cfg(any(target_os = "macos", target_os = "linux"))]
use librairii_app::localization::Language;
use librairii_desktop::{
    library::{Action, LibrarySession},
    startup::{StartupEnvironment, StartupSelection, select_startup},
};
use librairii_storage::{
    StorageError,
    database::WorkCancellation,
    native::NativeLibrary,
    profile::{NativeProfileStore, ProfileLocations},
    settings::TargetPreview,
};
use librairii_testkit::TestRoots;
use std::{cell::RefCell, ffi::OsString, fs};
type TestResult = Result<(), Box<dyn std::error::Error>>;
#[test]
fn override_selection_reads_no_production_inputs_and_keeps_language_in_memory() -> TestResult {
    let roots = TestRoots::new()?;
    struct Environment {
        root: OsString,
        reads: RefCell<Vec<String>>,
    }
    impl StartupEnvironment for Environment {
        fn variable(&self, name: &str) -> Option<OsString> {
            self.reads.borrow_mut().push(name.into());
            assert_eq!(name, "LIBRAIRII_DATA_ROOT");
            Some(self.root.clone())
        }
    }
    let environment = Environment {
        root: roots.library().join("override").into_os_string(),
        reads: RefCell::new(Vec::new()),
    };
    let StartupSelection::Override(mut settings) =
        select_startup(&environment, &Default::default())?
    else {
        return Err("override".into());
    };
    assert!(!settings.root().exists());
    assert!(!settings.relocation_allowed());
    assert_eq!(settings.relocation_message(), "settings.override");
    settings.change_language("fr")?;
    assert!(settings.change_language("de").is_err());
    assert_eq!(
        environment.reads.borrow().as_slice(),
        ["LIBRAIRII_DATA_ROOT"]
    );
    assert!(!settings.root().exists());
    assert_eq!(fs::read_dir(roots.profile())?.count(), 0);
    Ok(())
}
#[test]
fn desktop_move_reconstructs_real_services_and_invalidates_old_results() -> TestResult {
    let roots = TestRoots::new()?;
    let locations = ProfileLocations::for_host(
        &roots.profile().join("home"),
        Some(&roots.profile().join("config")),
        Some(&roots.library().join("data")),
    )?;
    fs::create_dir_all(&locations.data)?;
    fs::create_dir_all(locations.native.parent().ok_or("parent")?)?;
    let store = NativeProfileStore::create_fresh(&locations.native)?;
    let root = roots.library().join("initial");
    let target = roots.library().join("moved");
    let native = NativeLibrary::initialize_confirmed(
        TargetPreview::inspect(&root, &[], &Default::default())?,
        &store,
        &locations,
        "e0000000-0000-4000-8000-000000000013".parse()?,
        Language::French,
        &Default::default(),
    )
    .map_err(|f| f.to_string())?;
    let mut session = LibrarySession::new(native);
    let mut before = None;
    let mut prepared = None;
    let mut exported = None;
    let mut iteration = 0;
    let output = roots.library().join("external exports");
    fs::create_dir(&output)?;
    session
        .drive(&locations, &WorkCancellation::default(), |runtime| {
            if iteration == 0 {
                before = Some(
                    runtime
                        .generation()
                        .map_err(|_| StorageError::RuntimeFenced)?,
                );
                runtime.command(|db, _, _| {
                    db.read(|c| {
                        c.query_row("SELECT COUNT(*) FROM stories", [], |r| r.get::<_, i64>(0))
                            .map_err(StorageError::from)
                    })?
                    .wait()
                    .map(|count| assert_eq!(count, 0))
                })?;
                let source = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../preservation/corpus/archives/clockwork-forest-studio.zip");
                let prepared_import = librairii_desktop::imports::PreparedImport::new(
                    "a0000000-0000-4000-8000-000000000013"
                        .parse()
                        .map_err(|_| StorageError::InvalidIdentity)?,
                    vec![source],
                )
                .map_err(|_| StorageError::RuntimeInUse)?;
                runtime
                    .offer_import(prepared_import)
                    .map_err(|_| StorageError::RuntimeInUse)?;
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
                loop {
                    let terminal = runtime.command(|db, _, _| {
                        Ok(db
                            .operation_snapshot("a0000000-0000-4000-8000-000000000013".into())?
                            .wait()?
                            .is_some_and(|s| {
                                !matches!(
                                    s.operation.state,
                                    librairii_app::importing::OperationState::Queued
                                        | librairii_app::importing::OperationState::Running
                                )
                            }))
                    })?;
                    if terminal {
                        break;
                    }
                    if std::time::Instant::now() > deadline {
                        return Err(StorageError::RuntimeFenced);
                    }
                    std::thread::sleep(std::time::Duration::from_millis(2));
                }
                exported = runtime
                    .export_command(|exports| {
                        exports.prepare(
                            librairii_app::exporting::Scope::CurrentQuery(Default::default()),
                            &output,
                            "fr".into(),
                            &Default::default(),
                        )
                    })?
                    .preparation;
                assert!(exported.is_some());
                prepared = Some(runtime.command(|db, _, _| {
                    db.shelf_creation_preview(
                        librairii_storage::shelves::CreationRequest {
                            name: "Before move".into(),
                            ..Default::default()
                        },
                        "fr".into(),
                    )?
                    .wait()
                })?);
                iteration += 1;
                runtime
                    .move_library(
                        TargetPreview::inspect(&target, &[], &Default::default())?,
                        "f0000000-0000-4000-8000-000000000013".into(),
                    )
                    .map_err(|_| StorageError::RuntimeInUse)
            } else {
                assert_eq!(runtime.root(), target);
                assert!(!runtime.accepts(before.as_ref().ok_or(StorageError::RuntimeFenced)?));
                runtime.command(|db, files, _| {
                    let _custody = files.try_custody()?;
                    db.read(|c| {
                        c.query_row("SELECT COUNT(*) FROM stories", [], |r| r.get::<_, i64>(0))
                            .map_err(StorageError::from)
                    })?
                    .wait()
                    .map(|count| assert_eq!(count, 1))
                })?;
                runtime.export_command(|exports| {
                    assert!(
                        exports
                            .start(
                                exported.as_ref().ok_or(StorageError::RuntimeFenced)?,
                                &Default::default()
                            )
                            .is_err()
                    );
                    Ok(())
                })?;
                runtime.command(|db, _, _| {
                    let old = prepared.take().ok_or(StorageError::RuntimeFenced)?;
                    assert!(matches!(
                        db.create_shelf_reviewed(old, Vec::new(), Default::default()),
                        Err(StorageError::Shelf(_))
                    ));
                    Ok(())
                })?;
                Ok(Action::finish(()))
            }
        })
        .map_err(|f| f.to_string())?;
    session.finish().map_err(|f| f.to_string())?;
    assert!(!root.exists());
    assert!(!session_cleanup_pending(&store)?);
    Ok(())
}
fn session_cleanup_pending(store: &NativeProfileStore) -> Result<bool, StorageError> {
    Ok(store.directory().join("native-relocation-v1.json").exists())
}

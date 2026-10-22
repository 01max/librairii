#![cfg(any(target_os = "macos", target_os = "linux"))]
use librairii_app::{
    importing::OperationState,
    native_jobs::{Configuration, Handle, with_runtime},
};
use librairii_desktop::imports::{ImportExecutor, PrepareError, PreparedImport};
use librairii_storage::{
    IsolatedLibrary, database::Database, operations_repository::TerminalRepository,
};
use librairii_testkit::TestRoots;
use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};
type TestResult = Result<(), Box<dyn Error>>;
fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../preservation/corpus/archives")
        .join(name)
}
fn offer(
    executor: &ImportExecutor<'_, '_>,
    handle: &Handle,
    mut prepared: PreparedImport,
) -> TestResult {
    let end = Instant::now() + Duration::from_secs(10);
    loop {
        match executor.offer(handle, prepared) {
            Ok(()) => return Ok(()),
            Err((PrepareError::Busy, original)) if Instant::now() < end => {
                prepared = original;
                thread::sleep(Duration::from_millis(2));
            }
            Err((e, _)) => return Err(e.into()),
        }
    }
}
fn wait_terminal(db: &Database<'_>, id: &str) -> TestResult {
    let end = Instant::now() + Duration::from_secs(15);
    while Instant::now() < end {
        if db.operation_snapshot(id.into())?.wait()?.is_some_and(|s| {
            !matches!(
                s.operation.state,
                OperationState::Queued | OperationState::Running
            )
        }) {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(2));
    }
    Err("import operation did not reach a durable terminal result".into())
}
fn count_files(path: &Path) -> std::io::Result<usize> {
    let mut count = 0;
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            count += count_files(&entry.path())?;
        } else {
            count += 1;
        }
    }
    Ok(count)
}
#[test]
fn actual_import_duplicate_and_uuid_conflict_are_atomic_and_survive_reopen() -> TestResult {
    let roots = TestRoots::new()?;
    let library = IsolatedLibrary::initialize_from_override(
        Some(roots.library().as_os_str()),
        "a0000000-0000-4000-8000-000000000055".parse()?,
    )?;
    let first = roots.profile().join("Original name.PLAIN.PK");
    let exact = roots.profile().join("Same bytes.zip");
    let conflict = roots.profile().join("Conflict.v1.pk");
    fs::copy(fixture("clockwork-forest.plain.pk"), &first)?;
    fs::copy(&first, &exact)?;
    fs::copy(fixture("00112233445546778899aabbccddeeff.v1.pk"), &conflict)?;
    let originals = vec![fs::read(&first)?, fs::read(&exact)?, fs::read(&conflict)?];
    let mut db = Database::open_isolated(&library)?;
    let id = "a0000000-0000-4000-8000-000000000056";
    {
        let executor = ImportExecutor::new(&db);
        let store = TerminalRepository::new(&db, 1);
        with_runtime(
            &store,
            &executor,
            Configuration::default(),
            |handle| -> TestResult {
                offer(
                    &executor,
                    &handle,
                    PreparedImport::new(
                        id.parse()?,
                        vec![first.clone(), exact.clone(), conflict.clone()],
                    )?,
                )?;
                wait_terminal(&db, id)
            },
        )??;
    }
    let snapshot = db
        .operation_snapshot(id.into())?
        .wait()?
        .ok_or("operation missing")?;
    assert_eq!(snapshot.operation.state, OperationState::PartiallySucceeded);
    let mut outcomes: Vec<_> = snapshot
        .items
        .iter()
        .map(|i| i.outcome.code.as_deref().unwrap_or(""))
        .collect();
    outcomes.sort();
    assert_eq!(
        outcomes,
        vec!["duplicate_checksum", "imported", "uuid_conflict"]
    );
    let page = db.search(Default::default(), "en-GB".into())?.wait()?;
    assert_eq!(page.total_items, 1);
    assert_eq!(page.stories[0].title, "The Clockwork Forest");
    assert_eq!(page.stories[0].sources.title, "embedded");
    assert_eq!(count_files(&library.layout().archives())?, 1);
    assert_eq!(count_files(&library.layout().catalog())?, 1);
    assert_eq!(fs::read_dir(library.layout().staging())?.count(), 0);
    let files: Vec<_> = db
        .read(|tx| {
            let mut q = tx.prepare("SELECT managed_path,original_filename FROM story_archives")?;
            Ok(
                q.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
                    .collect::<std::result::Result<Vec<_>, _>>()?,
            )
        })?
        .wait()?;
    // Either exact contender can win; both retain their own original name.
    let (managed, name) = files.first().ok_or("archive missing")?;
    assert!(name == "Original name.PLAIN.PK" || name == "Same bytes.zip");
    assert_eq!(
        fs::read(library.layout().root().join(managed))?,
        originals[0]
    );
    db.close()?;
    let mut reopened = Database::open_isolated(&library)?;
    assert_eq!(
        reopened
            .search(Default::default(), "en-GB".into())?
            .wait()?
            .total_items,
        1
    );
    assert_eq!(
        reopened
            .operation_snapshot(id.into())?
            .wait()?
            .ok_or("reopened operation missing")?
            .items
            .len(),
        3
    );
    reopened.close()?;
    for (path, bytes) in [first, exact, conflict].iter().zip(originals) {
        assert_eq!(fs::read(path)?, bytes);
    }
    Ok(())
}

#[test]
fn actual_catalog_failure_compensates_new_archive_and_artwork_without_partial_rows() -> TestResult {
    let roots = TestRoots::new()?;
    let library = IsolatedLibrary::initialize_from_override(
        Some(roots.library().as_os_str()),
        "a0000000-0000-4000-8000-000000000061".parse()?,
    )?;
    let source = roots.profile().join("Studio.zip");
    fs::copy(fixture("clockwork-forest-studio.zip"), &source)?;
    let original = fs::read(&source)?;
    let mut db = Database::open_isolated(&library)?;
    let installed=db.transaction(Default::default(),|tx|{tx.execute_batch("CREATE TRIGGER fail_import BEFORE INSERT ON story_archives BEGIN SELECT RAISE(ABORT,'injected catalog failure'); END;")?;Ok(())})?.wait()?;
    assert!(matches!(
        installed,
        librairii_storage::database::TransactionOutcome::Committed(())
    ));
    let id = "a0000000-0000-4000-8000-000000000062";
    {
        let executor = ImportExecutor::new(&db);
        let store = TerminalRepository::new(&db, 1);
        with_runtime(
            &store,
            &executor,
            Configuration::default(),
            |handle| -> TestResult {
                offer(
                    &executor,
                    &handle,
                    PreparedImport::new(id.parse()?, vec![source.clone()])?,
                )?;
                wait_terminal(&db, id)
            },
        )??;
    }
    let result = db
        .operation_snapshot(id.into())?
        .wait()?
        .ok_or("operation missing")?;
    assert_eq!(result.operation.state, OperationState::Failed);
    assert_eq!(
        result.items[0].outcome.code.as_deref(),
        Some("catalog_failed")
    );
    assert_eq!(
        db.search(Default::default(), "en-GB".into())?
            .wait()?
            .total_items,
        0
    );
    assert_eq!(count_files(&library.layout().archives())?, 0);
    assert_eq!(count_files(&library.layout().catalog())?, 0);
    assert_eq!(count_files(&library.layout().trash())?, 2);
    assert_eq!(fs::read_dir(library.layout().staging())?.count(), 0);
    assert_eq!(fs::read(source)?, original);
    db.close()?;
    Ok(())
}
#[test]
fn actual_inspection_failures_keep_frozen_operation_reasons_and_clean_staging() -> TestResult {
    let roots = TestRoots::new()?;
    let library = IsolatedLibrary::initialize_from_override(
        Some(roots.library().as_os_str()),
        "a0000000-0000-4000-8000-000000000063".parse()?,
    )?;
    let mut db = Database::open_isolated(&library)?;
    let id = "a0000000-0000-4000-8000-000000000064";
    {
        let executor = ImportExecutor::new(&db);
        let store = TerminalRepository::new(&db, 1);
        with_runtime(
            &store,
            &executor,
            Configuration::default(),
            |handle| -> TestResult {
                offer(
                    &executor,
                    &handle,
                    PreparedImport::new(
                        id.parse()?,
                        vec![
                            fixture("invalid.zip"),
                            fixture("missing-ni.plain.pk"),
                            fixture("traversal.zip"),
                        ],
                    )?,
                )?;
                wait_terminal(&db, id)
            },
        )??;
    }
    let result = db
        .operation_snapshot(id.into())?
        .wait()?
        .ok_or("operation missing")?;
    assert_eq!(result.operation.state, OperationState::Failed);
    let outcomes: Vec<_> = result
        .items
        .iter()
        .map(|i| i.outcome.code.as_deref().unwrap_or(""))
        .collect();
    assert_eq!(
        outcomes,
        vec!["invalid_container", "missing_entry", "unsafe_path"]
    );
    assert_eq!(
        result.items[2].outcome.message.as_deref(),
        Some("The archive exceeds safe import limits.")
    );
    assert_eq!(fs::read_dir(library.layout().staging())?.count(), 0);
    assert_eq!(count_files(&library.layout().archives())?, 0);
    db.close()?;
    Ok(())
}

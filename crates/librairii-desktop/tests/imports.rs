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

use librairii_app::native_jobs::Cancellation;
use librairii_storage::imports::{ImportObserver, ImportPoint};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
#[derive(Debug)]
struct Fault {
    point: ImportPoint,
    hits: AtomicUsize,
}
impl ImportObserver for Fault {
    fn before(&self, point: ImportPoint, _: &Cancellation) -> librairii_storage::Result<()> {
        if point == self.point && self.hits.fetch_add(1, Ordering::SeqCst) == 0 {
            return Err(librairii_storage::StorageError::Io {
                operation: "injected import boundary",
                source: std::io::Error::other("injected failure"),
            });
        }
        Ok(())
    }
}
#[test]
fn actual_effect_boundary_failures_preserve_sources_and_cleanup_only_owned_temporaries()
-> TestResult {
    for (point, code) in [
        (ImportPoint::SourceOpened, "stage_failed"),
        (ImportPoint::BeforeRead, "stage_failed"),
        (ImportPoint::AfterCopyChunk, "stage_failed"),
        (ImportPoint::BeforeHash, "stage_failed"),
        (ImportPoint::Inspected, "inspection_failed"),
        (ImportPoint::BeforeArchivePublication, "publish_failed"),
        (ImportPoint::ArchivePublished, "publish_failed"),
        (ImportPoint::BeforeArtworkWrite, "artwork_failed"),
        (ImportPoint::ArtworkPublished, "artwork_failed"),
        (ImportPoint::BeforeCatalog, "catalog_failed"),
        (ImportPoint::BeforeSettlement, "imported"),
    ] {
        let roots = TestRoots::new()?;
        let library = IsolatedLibrary::initialize_from_override(
            Some(roots.library().as_os_str()),
            "a0000000-0000-4000-8000-000000000065".parse()?,
        )?;
        let source = roots.profile().join("source.zip");
        fs::copy(fixture("clockwork-forest-studio.zip"), &source)?;
        let bytes = fs::read(&source)?;
        let mut db = Database::open_isolated(&library)?;
        let id = "a0000000-0000-4000-8000-000000000066";
        let observer = Fault {
            point,
            hits: AtomicUsize::new(0),
        };
        {
            let executor = ImportExecutor::observed(&db, &observer);
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
            .ok_or("missing operation")?;
        assert_eq!(
            result.items[0].outcome.code.as_deref(),
            Some(code),
            "{point:?}"
        );
        let committed = point == ImportPoint::BeforeSettlement;
        assert_eq!(
            db.search(Default::default(), "en-GB".into())?
                .wait()?
                .total_items,
            i64::from(committed),
            "{point:?}"
        );
        assert_eq!(
            count_files(&library.layout().archives())?,
            usize::from(committed),
            "{point:?}"
        );
        assert_eq!(
            count_files(&library.layout().catalog())?,
            usize::from(committed),
            "{point:?}"
        );
        assert_eq!(
            fs::read_dir(library.layout().staging())?.count(),
            0,
            "{point:?}"
        );
        assert_eq!(fs::read(source)?, bytes, "{point:?}");
        assert!(observer.hits.load(Ordering::SeqCst) >= if committed { 2 } else { 1 });
        db.close()?;
    }
    Ok(())
}
#[derive(Debug)]
struct CorruptHash {
    staging: PathBuf,
    changed: AtomicBool,
}
impl ImportObserver for CorruptHash {
    fn before(&self, point: ImportPoint, _: &Cancellation) -> librairii_storage::Result<()> {
        if point == ImportPoint::BeforeHash {
            let action = (|| -> std::io::Result<()> {
                let directory = fs::read_dir(&self.staging)?
                    .next()
                    .ok_or_else(|| std::io::Error::other("missing actual stage"))??;
                fs::write(
                    directory.path().join("payload"),
                    b"externally corrupted staged bytes",
                )?;
                Ok(())
            })();
            action.map_err(|source| librairii_storage::StorageError::Io {
                operation: "inject stage mutation",
                source,
            })?;
            self.changed.store(true, Ordering::SeqCst);
        }
        Ok(())
    }
}
#[test]
fn actual_staged_hash_mismatch_and_missing_picker_sources_fail_per_item() -> TestResult {
    let roots = TestRoots::new()?;
    let library = IsolatedLibrary::initialize_from_override(
        Some(roots.library().as_os_str()),
        "a0000000-0000-4000-8000-000000000067".parse()?,
    )?;
    let source = roots.profile().join("source.zip");
    fs::copy(fixture("clockwork-forest-studio.zip"), &source)?;
    let bytes = fs::read(&source)?;
    let missing = roots.profile().join("missing.zip");
    let mut db = Database::open_isolated(&library)?;
    let observer = CorruptHash {
        staging: library.layout().staging(),
        changed: AtomicBool::new(false),
    };
    let id = "a0000000-0000-4000-8000-000000000068";
    {
        let executor = ImportExecutor::observed(&db, &observer);
        let store = TerminalRepository::new(&db, 1);
        with_runtime(
            &store,
            &executor,
            Configuration::new(1).ok_or("worker configuration")?,
            |handle| -> TestResult {
                offer(
                    &executor,
                    &handle,
                    PreparedImport::new(id.parse()?, vec![missing, source.clone()])?,
                )?;
                wait_terminal(&db, id)
            },
        )??;
    }
    let result = db
        .operation_snapshot(id.into())?
        .wait()?
        .ok_or("missing operation")?;
    assert!(observer.changed.load(Ordering::SeqCst));
    assert!(
        result
            .items
            .iter()
            .all(|i| i.outcome.code.as_deref() == Some("stage_failed"))
    );
    assert_eq!(fs::read(source)?, bytes);
    assert_eq!(count_files(&library.layout().archives())?, 0);
    assert_eq!(fs::read_dir(library.layout().staging())?.count(), 0);
    db.close()?;
    Ok(())
}
#[derive(Debug)]
struct CancelGate {
    point: Option<ImportPoint>,
    entered: AtomicBool,
    release: AtomicBool,
    timed_out: AtomicBool,
}
impl CancelGate {
    fn wait(&self, cancel: &Cancellation) {
        self.entered.store(true, Ordering::SeqCst);
        let end = Instant::now() + Duration::from_secs(15);
        while !(self.release.load(Ordering::SeqCst) && cancel.is_cancelled()) {
            if Instant::now() > end {
                self.timed_out.store(true, Ordering::SeqCst);
                break;
            }
            thread::sleep(Duration::from_millis(1));
        }
    }
}
impl ImportObserver for CancelGate {
    fn before(&self, point: ImportPoint, cancel: &Cancellation) -> librairii_storage::Result<()> {
        if self.point == Some(point) {
            self.wait(cancel);
        }
        Ok(())
    }
    fn committed(&self, cancel: &Cancellation) {
        if self.point.is_none() {
            self.wait(cancel);
        }
    }
}
#[test]
fn actual_native_cancellation_on_both_sides_of_commit_preserves_truth_and_bytes() -> TestResult {
    for point in [
        Some(ImportPoint::AfterCopyChunk),
        Some(ImportPoint::BeforeCatalog),
        None,
    ] {
        let roots = TestRoots::new()?;
        let library = IsolatedLibrary::initialize_from_override(
            Some(roots.library().as_os_str()),
            "a0000000-0000-4000-8000-000000000069".parse()?,
        )?;
        let source = roots.profile().join("source.zip");
        fs::copy(fixture("clockwork-forest-studio.zip"), &source)?;
        let bytes = fs::read(&source)?;
        let mut db = Database::open_isolated(&library)?;
        let observer = CancelGate {
            point,
            entered: AtomicBool::new(false),
            release: AtomicBool::new(false),
            timed_out: AtomicBool::new(false),
        };
        let id = "a0000000-0000-4000-8000-000000000070";
        {
            let executor = ImportExecutor::observed(&db, &observer);
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
                    let end = Instant::now() + Duration::from_secs(15);
                    while !observer.entered.load(Ordering::SeqCst) {
                        if Instant::now() > end {
                            return Err("cancel boundary not reached".into());
                        }
                        thread::sleep(Duration::from_millis(1));
                    }
                    while handle.cancel(id.into()).is_err() {
                        if Instant::now() > end {
                            return Err("cancel intake unavailable".into());
                        }
                        thread::sleep(Duration::from_millis(1));
                    }
                    observer.release.store(true, Ordering::SeqCst);
                    wait_terminal(&db, id)
                },
            )??;
        }
        assert!(!observer.timed_out.load(Ordering::SeqCst));
        let result = db
            .operation_snapshot(id.into())?
            .wait()?
            .ok_or("missing operation")?;
        let committed = point.is_none();
        assert_eq!(
            result.items[0].outcome.code.as_deref(),
            Some(if committed { "imported" } else { "cancelled" })
        );
        assert_eq!(
            db.search(Default::default(), "en-GB".into())?
                .wait()?
                .total_items,
            i64::from(committed)
        );
        assert_eq!(
            count_files(&library.layout().archives())?,
            usize::from(committed)
        );
        assert_eq!(fs::read_dir(library.layout().staging())?.count(), 0);
        assert_eq!(fs::read(source)?, bytes);
        db.close()?;
    }
    Ok(())
}

#[test]
fn actual_file_size_limit_short_write_preserves_source_and_retires_partial_stage() -> TestResult {
    const CHILD: &str = "LIBRAIRII_IMPORT_FSIZE_SOURCE";
    const LIMIT: u64 = 8 * 1024 * 1024 + 17;
    if let Some(source) = std::env::var_os(CHILD) {
        let source = PathBuf::from(source);
        let parent = source.parent().ok_or("missing synthetic source parent")?;
        if parent.file_name().is_none_or(|n| n != "profile")
            || parent
                .parent()
                .and_then(Path::file_name)
                .is_none_or(|n| !n.to_string_lossy().starts_with("librairii-synthetic-"))
        {
            return Err("explicit synthetic parent source required".into());
        }
        let roots = TestRoots::new()?;
        let library = IsolatedLibrary::initialize_from_override(
            Some(roots.library().as_os_str()),
            "a0000000-0000-4000-8000-000000000071".parse()?,
        )?;
        let mut db = Database::open_isolated(&library)?;
        let id = "a0000000-0000-4000-8000-000000000072";
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
                        PreparedImport::new(id.parse()?, vec![source])?,
                    )?;
                    wait_terminal(&db, id)
                },
            )??;
        }
        let result = db
            .operation_snapshot(id.into())?
            .wait()?
            .ok_or("missing operation")?;
        assert_eq!(result.operation.state, OperationState::Failed);
        assert_eq!(
            result.items[0].outcome.code.as_deref(),
            Some("stage_failed")
        );
        assert_eq!(count_files(&library.layout().archives())?, 0);
        assert_eq!(fs::read_dir(library.layout().staging())?.count(), 0);
        let retired = fs::read_dir(library.layout().trash())?
            .next()
            .ok_or("partial stage not retained in trash")??;
        assert_eq!(fs::metadata(retired.path().join("payload"))?.len(), LIMIT);
        db.close()?;
        return Ok(());
    }
    let roots = TestRoots::new()?;
    let source = roots.profile().join("large.zip");
    let mut payload = vec![0x5a; 10 * 1024 * 1024];
    payload.extend_from_slice(&fs::read(fixture("clockwork-forest-studio.zip"))?);
    fs::write(&source, &payload)?;
    let script = r#"import os, resource, signal, sys
limit=8*1024*1024+17
resource.setrlimit(resource.RLIMIT_FSIZE,(limit,limit))
if resource.getrlimit(resource.RLIMIT_FSIZE)!=(limit,limit):
    raise SystemExit('file-size control did not apply')
signal.signal(signal.SIGXFSZ,signal.SIG_IGN)
os.execv(sys.argv[1],[sys.argv[1],'--exact','actual_file_size_limit_short_write_preserves_source_and_retires_partial_stage','--nocapture'])
"#;
    let output = std::process::Command::new("python3")
        .args(["-c", script])
        .arg(std::env::current_exe()?)
        .env(CHILD, &source)
        .output()?;
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::read(source)?, payload);
    Ok(())
}

use librairii_storage::{
    adoption::{ClosedLibraryInput, ClosedManifest, IndependentCopy},
    database::WorkCancellation,
    native::NativeLibrary,
    profile::{NativeProfileStore, Profile, ProfileDocument, ProfileLocations},
};
fn native_fixture(
    roots: &TestRoots,
) -> Result<(IndependentCopy, ProfileLocations), Box<dyn Error>> {
    let source = roots.library().join("legacy-source");
    fs::create_dir(&source)?;
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../preservation/corpus");
    let manifest = fs::read(base.join("profiles/library-manifest.json"))?;
    let entries: serde_json::Value = serde_json::from_slice(&manifest)?;
    for entry in entries.as_array().ok_or("manifest array")? {
        let path = entry["path"].as_str().ok_or("manifest path")?;
        if entry["directory"] == true {
            fs::create_dir(source.join(path))?;
        } else {
            fs::copy(
                base.join("libraries/schema018").join(path),
                source.join(path),
            )?;
        }
    }
    let profile = Profile {
        version: 1,
        language: "fr".into(),
        root: source.to_str().ok_or("source UTF8")?.into(),
        library_id: "a0000000-0000-4000-8000-000000000001".into(),
        complete: true,
        recovering: false,
    };
    let bytes = serde_json::to_vec(&profile)?;
    let verified = ClosedLibraryInput::verify(
        &source,
        ProfileDocument::decode(&bytes)?,
        ClosedManifest::decode(&manifest)?,
        &WorkCancellation::default(),
    )?;
    let locations = ProfileLocations::for_host(
        &roots.profile().join("home"),
        Some(&roots.profile().join("config")),
        Some(&roots.library().join("data")),
    )?;
    fs::create_dir_all(&locations.data)?;
    fs::create_dir_all(locations.native.parent().ok_or("native parent")?)?;
    let legacy = locations.legacy.as_ref().ok_or("legacy namespace")?;
    fs::create_dir_all(legacy)?;
    fs::write(legacy.join("settings.json"), bytes)?;
    let copy = verified
        .copy_to_new_root(
            &locations.data.join("candidate"),
            &WorkCancellation::default(),
        )
        .map_err(|f| f.to_string())?;
    Ok((copy, locations))
}
#[test]
fn actual_registered_native_import_closes_and_reopens_with_unchanged_legacy_source() -> TestResult {
    let roots = TestRoots::new()?;
    let (copy, locations) = native_fixture(&roots)?;
    let legacy = PathBuf::from(copy.source_profile().profile().root.clone());
    let before = fs::read(legacy.join("db/librairii.sqlite3"))?;
    let profile = NativeProfileStore::create_fresh(&locations.native)?;
    let native =
        NativeLibrary::bind_independent(copy, &profile, &locations, &WorkCancellation::default())
            .map_err(|f| f.to_string())?;
    use std::os::unix::ffi::{OsStrExt, OsStringExt};
    #[cfg(target_os = "linux")]
    let filename = std::ffi::OsString::from_vec(b"original \xff.ZIP".to_vec());
    #[cfg(target_os = "macos")]
    let filename = std::ffi::OsString::from("Original Été 日本語.ZIP");
    let source = roots.profile().join(&filename);
    fs::copy(fixture("clockwork-forest-studio.zip"), &source)?;
    let original = fs::read(&source)?;
    let mut managed =
        format!("archives/{}/", librairii_app::metadata::digest(&original)).into_bytes();
    managed.extend_from_slice(filename.as_bytes());
    let managed_root = native.layout().root().to_owned();
    let check_names = |db: &Database<'_>| -> TestResult {
        let row = db
            .barrier_read(|tx| {
                let mut found = None;
                librairii_storage::query::visit_scope(tx, Default::default(), "en-GB", |story| {
                    if story.uuid == "00112233-4455-4677-8899-aabbccddeeff" {
                        found = Some((
                            story.original_filename_bytes,
                            story.managed_relative_path_bytes,
                            story.original_filename,
                            story.managed_relative_path,
                        ));
                    }
                    Ok(())
                })?;
                Ok(found)
            })?
            .wait()?
            .ok_or("imported query record missing")?;
        assert_eq!(row.0, filename.as_bytes());
        assert_eq!(row.1, managed);
        assert_eq!(
            row.2,
            std::str::from_utf8(filename.as_bytes()).unwrap_or_default()
        );
        assert_eq!(row.3, std::str::from_utf8(&managed).unwrap_or_default());
        let path = managed_root.join(std::ffi::OsString::from_vec(managed.clone()));
        assert_eq!(fs::read(path)?, original);
        Ok(())
    };
    let mut db = Database::open_native(&native)?;
    let before_count = db
        .search(Default::default(), "en-GB".into())?
        .wait()?
        .total_items;
    let id = "a0000000-0000-4000-8000-000000000073";
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
    assert_eq!(
        db.search(Default::default(), "en-GB".into())?
            .wait()?
            .total_items,
        before_count + 1
    );
    assert_eq!(
        db.operation_snapshot(id.into())?
            .wait()?
            .ok_or("missing import")?
            .items[0]
            .outcome
            .code
            .as_deref(),
        Some("imported")
    );
    assert_eq!(fs::read_dir(native.layout().staging())?.count(), 0);
    check_names(&db)?;
    db.close()?;
    drop(db);
    native.finish().map_err(|f| f.to_string())?;
    drop(profile);
    let profile = NativeProfileStore::open_native(&locations.native)?;
    let native = NativeLibrary::reopen(&profile, &locations, &WorkCancellation::default())?;
    let mut db = Database::open_native(&native)?;
    check_names(&db)?;
    assert_eq!(
        db.search(Default::default(), "en-GB".into())?
            .wait()?
            .total_items,
        before_count + 1
    );
    db.close()?;
    drop(db);
    native.finish().map_err(|f| f.to_string())?;
    assert_eq!(fs::read(source)?, original);
    assert_eq!(fs::read(legacy.join("db/librairii.sqlite3"))?, before);
    Ok(())
}
#[derive(Debug)]
struct CrashGate {
    point: Option<ImportPoint>,
    ready: PathBuf,
}
impl CrashGate {
    fn hold(&self) {
        if fs::write(&self.ready, b"actual import boundary reached")
            .and_then(|()| fs::File::open(&self.ready)?.sync_all())
            .is_err()
        {
            std::process::exit(74);
        }
        loop {
            thread::park();
        }
    }
}
impl ImportObserver for CrashGate {
    fn before(&self, point: ImportPoint, _: &Cancellation) -> librairii_storage::Result<()> {
        if self.point == Some(point) {
            self.hold();
        }
        Ok(())
    }
    fn committed(&self, _: &Cancellation) {
        if self.point.is_none() {
            self.hold();
        }
    }
}
struct OwnedChild(std::process::Child);
impl Drop for OwnedChild {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}
#[test]
fn actual_import_process_death_recovers_registered_stage_and_preserves_committed_bytes()
-> TestResult {
    const CONTEXT: &str = "LIBRAIRII_ACTUAL_IMPORT_CRASH_CONTEXT";
    if let Some(context) = std::env::var_os(CONTEXT) {
        let value: serde_json::Value = serde_json::from_slice(&fs::read(context)?)?;
        let path = |key: &str| -> Result<PathBuf, Box<dyn Error>> {
            Ok(value[key].as_str().ok_or("context path")?.into())
        };
        let locations = ProfileLocations {
            native: path("native")?,
            data: path("data")?,
            legacy: Some(path("legacy")?),
        };
        let profile = NativeProfileStore::open_native(&locations.native)?;
        let native = NativeLibrary::reopen(&profile, &locations, &WorkCancellation::default())?;
        let mut db = Database::open_native(&native)?;
        let point = match value["point"].as_str().ok_or("crash point")? {
            "copy" => Some(ImportPoint::AfterCopyChunk),
            "published" => Some(ImportPoint::ArchivePublished),
            "committed" => None,
            _ => return Err("unknown crash point".into()),
        };
        let observer = CrashGate {
            point,
            ready: path("ready")?,
        };
        let executor = ImportExecutor::observed(&db, &observer);
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
                        "a0000000-0000-4000-8000-000000000074".parse()?,
                        vec![path("source")?],
                    )?,
                )?;
                loop {
                    thread::park();
                }
            },
        )??;
        db.close()?;
        return Err("crash child unexpectedly returned".into());
    }
    for point in ["copy", "published", "committed"] {
        let roots = TestRoots::new()?;
        let (copy, locations) = native_fixture(&roots)?;
        let root = copy.root().to_owned();
        let profile = NativeProfileStore::create_fresh(&locations.native)?;
        let native = NativeLibrary::bind_independent(
            copy,
            &profile,
            &locations,
            &WorkCancellation::default(),
        )
        .map_err(|f| f.to_string())?;
        let mut db = Database::open_native(&native)?;
        let before = db
            .search(Default::default(), "en-GB".into())?
            .wait()?
            .total_items;
        db.close()?;
        drop(db);
        native.finish().map_err(|f| f.to_string())?;
        drop(profile);
        let source = roots.profile().join("source.zip");
        fs::copy(fixture("clockwork-forest-studio.zip"), &source)?;
        let bytes = fs::read(&source)?;
        let ready = roots.profile().join("ready");
        let context = roots.profile().join("context.json");
        fs::write(
            &context,
            serde_json::to_vec(
                &serde_json::json!({"native":locations.native,"data":locations.data,"legacy":locations.legacy,"source":source,"ready":ready,"point":point}),
            )?,
        )?;
        let output_path = roots.profile().join("child-output");
        let output = fs::File::create(&output_path)?;
        let mut child=OwnedChild(std::process::Command::new(std::env::current_exe()?).args(["--exact","actual_import_process_death_recovers_registered_stage_and_preserves_committed_bytes","--nocapture"]).env(CONTEXT,&context).stdout(std::process::Stdio::from(output.try_clone()?)).stderr(std::process::Stdio::from(output)).spawn()?);
        let end = Instant::now() + Duration::from_secs(20);
        while !ready.exists() {
            if let Some(status) = child.0.try_wait()? {
                return Err(format!(
                    "crash child {status}: {}",
                    fs::read_to_string(&output_path)?
                )
                .into());
            }
            if Instant::now() > end {
                return Err("actual import boundary not reached".into());
            }
            thread::sleep(Duration::from_millis(2));
        }
        child.0.kill()?;
        child.0.wait()?;
        let profile = NativeProfileStore::open_native(&locations.native)?;
        assert!(NativeLibrary::reopen(&profile, &locations, &WorkCancellation::default()).is_err());
        drop(profile);
        let profile = NativeProfileStore::open_native(&locations.native)?;
        let recovery =
            NativeLibrary::recover_interrupted(&profile, &locations, &WorkCancellation::default())?;
        let mut db = recovery.open()?;
        let id = "a0000000-0000-4000-8000-000000000074";
        assert_eq!(
            db.operation_snapshot(id.into())?
                .wait()?
                .ok_or("interrupted operation missing")?
                .operation
                .state,
            OperationState::Interrupted
        );
        assert_eq!(
            db.search(Default::default(), "en-GB".into())?
                .wait()?
                .total_items,
            before + i64::from(point == "committed")
        );
        assert_eq!(fs::read_dir(root.join("staging"))?.count(), 0);
        // A new explicit retry safely reuses an orphan publication, or reports
        // a checksum duplicate when SQL had already committed before death.
        let retry = "a0000000-0000-4000-8000-000000000075";
        {
            let executor = ImportExecutor::new(&db);
            let store = TerminalRepository::new(&db, 2);
            with_runtime(
                &store,
                &executor,
                Configuration::default(),
                |handle| -> TestResult {
                    offer(
                        &executor,
                        &handle,
                        PreparedImport::new(retry.parse()?, vec![source.clone()])?,
                    )?;
                    wait_terminal(&db, retry)
                },
            )??;
        }
        let expected = if point == "committed" {
            "duplicate_checksum"
        } else {
            "imported"
        };
        assert_eq!(
            db.operation_snapshot(retry.into())?
                .wait()?
                .ok_or("retry missing")?
                .items[0]
                .outcome
                .code
                .as_deref(),
            Some(expected)
        );
        assert_eq!(
            db.search(Default::default(), "en-GB".into())?
                .wait()?
                .total_items,
            before + 1
        );
        assert_eq!(fs::read(&source)?, bytes);
        db.close()?;
        drop(db);
        recovery.finish().map_err(|f| f.to_string())?;
    }
    Ok(())
}

#[test]
fn actual_native_import_inspect_export_and_reopen_preserves_all_formats_and_native_names()
-> TestResult {
    use librairii_adapters::archives::ArchiveInspector;
    use librairii_app::exporting::{Disposition, Scope};
    use librairii_app::{
        importing::Inspector,
        native_jobs::{Refusal, Start},
    };
    use librairii_storage::export::Exports;
    use std::{
        ffi::OsString,
        os::unix::ffi::{OsStrExt, OsStringExt},
    };
    let mut cases = vec![
        (
            "clockwork-forest.plain.pk",
            OsString::from("Original Été.PLAIN.PK"),
        ),
        (
            "00112233445546778899aabbccddeeff.v1.pk",
            OsString::from("Original 日本語.v1.pk"),
        ),
        (
            "00112233445546778899aabbccddeeff.v2.pk",
            OsString::from("Original Été.v2.pk"),
        ),
        ("clockwork-forest.pk", OsString::from("Original 日本語.pk")),
        ("clockwork-forest.zip", OsString::from("Original Été.ZIP")),
        (
            "clockwork-forest-studio.zip",
            OsString::from("Original 日本語.ZIP"),
        ),
        ("studio.7z", OsString::from("Original Été.7z")),
    ];
    if cfg!(target_os = "linux") {
        cases.push((
            "clockwork-forest-studio.zip",
            OsString::from_vec(b"Original \xe1\x80\xff.ZIP".to_vec()),
        ));
    }
    fn offer_start(handle: &Handle, mut start: Start) -> TestResult {
        let end = Instant::now() + Duration::from_secs(10);
        loop {
            match handle.try_start(start) {
                Ok(()) => return Ok(()),
                Err((Refusal::Busy, original)) if Instant::now() < end => {
                    start = original;
                    thread::sleep(Duration::from_millis(2));
                }
                Err(_) => return Err("export runtime intake unavailable".into()),
            }
        }
    }
    for (case, (input, filename)) in cases.into_iter().enumerate() {
        let roots = TestRoots::new()?;
        let (copy, locations) = native_fixture(&roots)?;
        let legacy = PathBuf::from(copy.source_profile().profile().root.clone());
        let legacy_database = fs::read(legacy.join("db/librairii.sqlite3"))?;
        let profile = NativeProfileStore::create_fresh(&locations.native)?;
        let native = NativeLibrary::bind_independent(
            copy,
            &profile,
            &locations,
            &WorkCancellation::default(),
        )
        .map_err(|f| f.to_string())?;
        let source = roots.profile().join(&filename);
        fs::copy(fixture(input), &source)?;
        let bytes = fs::read(&source)?;
        let expected = ArchiveInspector.inspect(
            &mut std::io::Cursor::new(&bytes),
            filename.as_bytes(),
            &Cancellation::default(),
        )?;
        let uuid = expected.identity.canonical();
        let import_id = format!("a0000000-0000-4000-8000-{:012}", 12000 + case * 2);
        let export_id = format!("a0000000-0000-4000-8000-{:012}", 12001 + case * 2);
        let mut db = Database::open_native(&native)?;
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
                        PreparedImport::new(import_id.parse()?, vec![source.clone()])?,
                    )?;
                    wait_terminal(&db, &import_id)
                },
            )??;
        }
        let imported = db
            .operation_snapshot(import_id.clone())?
            .wait()?
            .ok_or("missing import history")?;
        assert_eq!(
            imported.operation.state,
            OperationState::Succeeded,
            "{input}"
        );
        assert_eq!(imported.items[0].outcome.code.as_deref(), Some("imported"));
        let snapshot = db
            .resolve_export(Scope::CurrentQuery(Default::default()), "fr-FR".into())?
            .wait()?;
        let row = snapshot
            .stories
            .into_iter()
            .find(|s| s.uuid == uuid)
            .ok_or("missing imported story")?;
        let story_id = row.id;
        assert_eq!(row.original_filename_bytes, filename.as_bytes());
        assert_eq!(
            row.sha256,
            librairii_app::metadata::digest(&bytes).to_string()
        );
        let managed = row.managed_relative_path_bytes;
        let destination = roots.profile().join("selected destination Été");
        fs::create_dir(&destination)?;
        {
            let mut exports = Exports::new(&db);
            let preview = exports.prepare(
                Scope::Selection(vec![story_id]),
                &destination,
                "fr-FR".into(),
                &WorkCancellation::default(),
            )?;
            assert_eq!(preview.ready, 1, "{input}");
            assert_eq!(preview.items[0].disposition, Disposition::Ready);
            let preparation = preview.preparation.ok_or("no export preparation")?;
            exports.run_prepared(
                &preparation,
                export_id.clone(),
                Configuration::default(),
                |handle, start| -> TestResult {
                    offer_start(&handle, start)?;
                    wait_terminal(&db, &export_id)
                },
            )??;
        }
        let history = db
            .operation_snapshot(export_id.clone())?
            .wait()?
            .ok_or("missing export history")?;
        assert_eq!(
            history.operation.state,
            OperationState::Succeeded,
            "{input}"
        );
        assert_eq!(history.items[0].outcome.code.as_deref(), Some("exported"));
        assert_eq!(
            history.items[0]
                .output_name
                .as_ref()
                .ok_or("no stored name")?
                .as_os_str(),
            filename
        );
        assert_eq!(
            history.items[0]
                .archive_relative_path
                .as_ref()
                .ok_or("no stored path")?
                .as_os_str()
                .as_bytes(),
            managed
        );
        assert_eq!(
            history.presentation()?["items"][0]["outputName"],
            librairii_app::ports::operations::display_native_text(filename.as_bytes())
        );
        let sql_id = export_id.clone();
        let stored=db.barrier_read(move |tx| Ok(tx.query_row(
            "SELECT CAST(output_name AS BLOB),CAST(archive_relative_path AS BLOB),typeof(output_name),typeof(archive_relative_path) FROM file_operation_items WHERE operation_id=?",
            [sql_id],|r|Ok((r.get::<_,Vec<u8>>(0)?,r.get::<_,Vec<u8>>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?)))?))?.wait()?;
        assert_eq!(
            stored,
            (
                filename.as_bytes().to_vec(),
                managed.clone(),
                "text".into(),
                "text".into()
            )
        );
        let output = destination.join(&filename);
        assert_eq!(fs::read(&output)?, bytes);
        assert_eq!(fs::read_dir(&destination)?.count(), 1);
        let inspected = ArchiveInspector.inspect(
            &mut fs::File::open(&output)?,
            filename.as_bytes(),
            &Cancellation::default(),
        )?;
        assert_eq!(inspected, expected, "{input}");
        assert_eq!(
            fs::read(
                native
                    .layout()
                    .root()
                    .join(OsString::from_vec(managed.clone()))
            )?,
            bytes
        );
        db.close()?;
        drop(db);
        native.finish().map_err(|f| f.to_string())?;
        drop(profile);
        let profile = NativeProfileStore::open_native(&locations.native)?;
        let native = NativeLibrary::reopen(&profile, &locations, &WorkCancellation::default())?;
        let mut db = Database::open_native(&native)?;
        assert_eq!(
            db.operation_snapshot(export_id.clone())?
                .wait()?
                .ok_or("export lost after reopen")?,
            history
        );
        {
            let mut exports = Exports::new(&db);
            let preview = exports.prepare(
                Scope::Selection(vec![story_id]),
                &destination,
                "en-GB".into(),
                &WorkCancellation::default(),
            )?;
            assert_eq!(preview.items[0].disposition, Disposition::Conflicted);
            assert_eq!(preview.ready, 0);
            assert!(preview.preparation.is_none());
        }
        assert_eq!(fs::read(&output)?, bytes);
        assert_eq!(fs::read(&source)?, bytes);
        assert_eq!(
            fs::read(legacy.join("db/librairii.sqlite3"))?,
            legacy_database
        );
        assert_eq!(fs::read_dir(native.layout().staging())?.count(), 0);
        db.close()?;
        drop(db);
        native.finish().map_err(|f| f.to_string())?;
    }
    Ok(())
}

use librairii_storage::{
    IsolatedLibrary, StorageError,
    database::{
        Database, ORDINARY_QUEUE_CAPACITY, READ_WORKERS, REPAIR_QUEUE_CAPACITY, TransactionOutcome,
        WorkCancellation,
    },
};
use librairii_testkit::TestRoots;
use std::{error::Error, fs, sync::mpsc, time::Duration};

type TestResult = Result<(), Box<dyn Error>>;
fn isolated(roots: &TestRoots) -> Result<IsolatedLibrary, Box<dyn Error>> {
    Ok(IsolatedLibrary::initialize_from_override(
        Some(roots.library().as_os_str()),
        "A0000000-0000-4000-8000-000000000001".parse()?,
    )?)
}
fn committed<T>(outcome: TransactionOutcome<T>) -> Result<T, StorageError> {
    match outcome {
        TransactionOutcome::Committed(value) => Ok(value),
        TransactionOutcome::ProvenNotCommitted(error)
        | TransactionOutcome::CommitUncertain(error) => Err(error),
    }
}
fn synthetic_error(error: impl std::fmt::Display) -> StorageError {
    StorageError::Io {
        operation: "synthetic worker barrier",
        source: std::io::Error::other(error.to_string()),
    }
}

#[test]
fn five_owned_connections_have_actual_required_pragmas_and_close_before_reuse() -> TestResult {
    let roots = TestRoots::new()?;
    let library = isolated(&roots)?;
    let mut database = Database::open_isolated(&library)?;
    assert_eq!(database.diagnostics().len(), 5);
    for (index, diagnostic) in database.diagnostics().iter().enumerate() {
        assert_eq!(diagnostic.journal_mode, "wal");
        assert_eq!(diagnostic.foreign_keys, 1);
        assert_eq!(diagnostic.busy_timeout_ms, 5000);
        assert_eq!(diagnostic.query_only, i64::from(index != 0));
        assert_eq!(diagnostic.sqlite_version, rusqlite::version());
        assert_eq!(diagnostic.sqlite_version, "3.53.2");
        assert_eq!(
            diagnostic.sqlite_source_id,
            "2026-06-03 19:12:13 d6e03d8c777cfa2d35e3b60d8ec3e0187f3e9f99d8e2ee9cac695fd6fcdf1a24"
        );
    }
    assert!(matches!(
        Database::open_isolated(&library),
        Err(StorageError::RuntimeInUse)
    ));
    let count = database
        .read(|transaction| {
            Ok(
                transaction.query_row("SELECT count(*) FROM schema_migrations", [], |row| {
                    row.get::<_, i64>(0)
                })?,
            )
        })?
        .wait()?;
    assert_eq!(count, 18);
    assert!(
        database
            .read(|transaction| {
                transaction.execute("DELETE FROM schema_migrations", [])?;
                Ok(())
            })?
            .wait()
            .is_err()
    );
    database.close()?;
    database.close()?;
    assert!(matches!(
        database.read(|_| Ok(())),
        Err(StorageError::IntakeClosed)
    ));
    assert!(
        !library
            .layout()
            .database()
            .with_file_name("librairii.sqlite3-wal")
            .exists()
    );
    //4.5 qualified owned-root reopen verifies the schema through an independent
    // probe. It confirms the old runtime claim was actually freed after close.
    let mut reopened = Database::open_isolated(&library)?;
    reopened.close()?;
    Ok(())
}

#[test]
fn four_coherent_read_snapshots_run_concurrently_with_the_separate_writer() -> TestResult {
    fn at<T, E: std::fmt::Debug>(phase: &str, result: Result<T, E>) -> Result<T, E> {
        result.inspect_err(|error| eprintln!("coherent snapshot phase={phase}: {error:?}"))
    }
    let roots = TestRoots::new()?;
    let library = isolated(&roots)?;
    let mut database = at("open five connections", Database::open_isolated(&library))?;
    at(
        "create table outcome",
        committed(at(
            "create table wait",
            at(
                "create table admission",
                database.transaction(WorkCancellation::default(), |tx| {
                    tx.execute_batch("CREATE TABLE worker_probe(value INTEGER)")?;
                    Ok(())
                }),
            )?
            .wait(),
        )?),
    )?;
    let (entered, observed) = mpsc::channel();
    let mut gates = Vec::new();
    let mut pending = Vec::new();
    for _ in 0..READ_WORKERS {
        let (release, gate) = mpsc::channel();
        gates.push(release);
        let entered = entered.clone();
        pending.push(at(
            "held reader admission",
            database.read(move |tx| {
                let before: i64 =
                    tx.query_row("SELECT count(*) FROM worker_probe", [], |row| row.get(0))?;
                entered.send(()).map_err(synthetic_error)?;
                gate.recv_timeout(Duration::from_secs(5))
                    .map_err(synthetic_error)?;
                let after: i64 =
                    tx.query_row("SELECT count(*) FROM worker_probe", [], |row| row.get(0))?;
                Ok((before, after))
            }),
        )?);
    }
    for _ in 0..READ_WORKERS {
        observed.recv_timeout(Duration::from_secs(3))?;
    }
    let (started, started_receiver) = mpsc::channel();
    let write = at(
        "concurrent writer admission",
        database.transaction(WorkCancellation::default(), move |tx| {
            tx.execute("INSERT INTO worker_probe VALUES(1)", [])?;
            started.send(()).map_err(synthetic_error)?;
            Ok(())
        }),
    )?;
    started_receiver.recv_timeout(Duration::from_secs(3))?;
    at(
        "concurrent writer outcome",
        committed(at("concurrent writer wait", write.wait())?),
    )?;
    let mut queued = Vec::new();
    for _ in 0..ORDINARY_QUEUE_CAPACITY {
        queued.push(at("saturated reader admission", database.read(|_| Ok(())))?);
    }
    assert!(matches!(
        database.read(|_| Ok(())),
        Err(StorageError::QueueFull)
    ));
    for gate in gates {
        gate.send(())?;
    }
    for read in pending {
        assert_eq!(at("held snapshot wait", read.wait())?, (0, 0));
    }
    for read in queued {
        at("queued reader wait", read.wait())?;
    }
    assert_eq!(
        at(
            "fresh writer barrier wait",
            at(
                "fresh writer barrier admission",
                database.barrier_read(|tx| Ok(tx.query_row(
                    "SELECT count(*) FROM worker_probe",
                    [],
                    |row| row.get::<_, i64>(0)
                )?))
            )?
            .wait()
        )?,
        1
    );
    at("drain joins checkpoint close", database.close())?;
    Ok(())
}

#[test]
fn saturated_ordinary_queue_keeps_bounded_fifo_repair_and_fresh_barrier_admission() -> TestResult {
    let roots = TestRoots::new()?;
    let library = isolated(&roots)?;
    let mut database = Database::open_isolated(&library)?;
    committed(
        database
            .transaction(WorkCancellation::default(), |tx| {
                tx.execute_batch("CREATE TABLE worker_order(value INTEGER)")?;
                Ok(())
            })?
            .wait()?,
    )?;
    let (entered, observed) = mpsc::channel();
    let (release, gate) = mpsc::channel();
    let held = database.transaction(WorkCancellation::default(), move |_| {
        entered.send(()).map_err(synthetic_error)?;
        gate.recv_timeout(Duration::from_secs(5))
            .map_err(synthetic_error)?;
        Ok(())
    })?;
    observed.recv_timeout(Duration::from_secs(3))?;
    let mut writes = Vec::new();
    for value in 0..ORDINARY_QUEUE_CAPACITY {
        writes.push(
            database.transaction(WorkCancellation::default(), move |tx| {
                tx.execute("INSERT INTO worker_order VALUES(?)", [value as i64])?;
                Ok(())
            })?,
        );
    }
    assert!(matches!(
        database.transaction(WorkCancellation::default(), |_| Ok(())),
        Err(StorageError::QueueFull)
    ));
    database.stop_ordinary()?;
    assert!(matches!(
        database.read(|_| Ok(())),
        Err(StorageError::IntakeClosed)
    ));
    let barrier = database.barrier_read(|tx| {
        Ok(
            tx.query_row("SELECT count(*) FROM worker_order", [], |row| {
                row.get::<_, i64>(0)
            })?,
        )
    })?;
    let mut repairs = Vec::new();
    for _ in 1..REPAIR_QUEUE_CAPACITY {
        repairs.push(database.repair_transaction(|_| Ok(()))?);
    }
    assert!(matches!(
        database.repair_transaction(|_| Ok(())),
        Err(StorageError::QueueFull)
    ));
    release.send(())?;
    committed(held.wait()?)?;
    for write in writes {
        committed(write.wait()?)?;
    }
    assert_eq!(barrier.wait()?, ORDINARY_QUEUE_CAPACITY as i64);
    for repair in repairs {
        committed(repair.wait()?)?;
    }
    // Repair remains usable despite ordinary intake stop, and uses the same
    // writer/transaction machinery, not an extra connection or hidden retry.
    committed(
        database
            .repair_transaction(|tx| {
                tx.execute("INSERT INTO worker_order VALUES(99)", [])?;
                Ok(())
            })?
            .wait()?,
    )?;
    database.close()?;
    Ok(())
}

#[test]
fn work_cancellation_and_body_errors_rollback_but_late_cancel_preserves_commit() -> TestResult {
    let roots = TestRoots::new()?;
    let library = isolated(&roots)?;
    let mut database = Database::open_isolated(&library)?;
    committed(
        database
            .transaction(WorkCancellation::default(), |tx| {
                tx.execute_batch("CREATE TABLE worker_cancel(value INTEGER)")?;
                Ok(())
            })?
            .wait()?,
    )?;
    let cancellation = WorkCancellation::default();
    cancellation.cancel();
    assert!(matches!(
        database
            .transaction(cancellation, |_| -> librairii_storage::Result<()> {
                Err(StorageError::RuntimeFenced)
            })?
            .wait()?,
        TransactionOutcome::ProvenNotCommitted(StorageError::Cancelled)
    ));
    let cancellation = WorkCancellation::default();
    let within = cancellation.clone();
    assert!(matches!(
        database
            .transaction(cancellation, move |tx| {
                tx.execute("INSERT INTO worker_cancel VALUES(1)", [])?;
                within.cancel();
                Ok(())
            })?
            .wait()?,
        TransactionOutcome::ProvenNotCommitted(StorageError::Cancelled)
    ));
    assert!(matches!(
        database
            .transaction(
                WorkCancellation::default(),
                |tx| -> librairii_storage::Result<()> {
                    tx.execute("INSERT INTO worker_cancel VALUES(2)", [])?;
                    Err(StorageError::InvalidManagedPath)
                }
            )?
            .wait()?,
        TransactionOutcome::ProvenNotCommitted(StorageError::InvalidManagedPath)
    ));
    let cancellation = WorkCancellation::default();
    committed(
        database
            .transaction(cancellation.clone(), |tx| {
                tx.execute("INSERT INTO worker_cancel VALUES(3)", [])?;
                Ok(())
            })?
            .wait()?,
    )?;
    cancellation.cancel();
    assert_eq!(
        database
            .barrier_read(|tx| Ok(tx.query_row(
                "SELECT sum(value) FROM worker_cancel",
                [],
                |row| row.get::<_, i64>(0)
            )?))?
            .wait()?,
        3
    );
    database.close()?;
    Ok(())
}

#[test]
fn commit_error_is_uncertain_then_fresh_writer_readback_preserves_actual_truth() -> TestResult {
    let roots = TestRoots::new()?;
    let library = isolated(&roots)?;
    let mut database = Database::open_isolated(&library)?;
    committed(database.transaction(WorkCancellation::default(),|tx| {
        tx.execute_batch("CREATE TABLE worker_parent(id INTEGER PRIMARY KEY); CREATE TABLE worker_child(parent INTEGER REFERENCES worker_parent(id) DEFERRABLE INITIALLY DEFERRED)")?; Ok(())
    })?.wait()?)?;
    let (entered, observed) = mpsc::channel();
    let (release, gate) = mpsc::channel();
    let attempt = database.transaction(WorkCancellation::default(), move |tx| {
        tx.execute("INSERT INTO worker_child VALUES(404)", [])?;
        entered.send(()).map_err(synthetic_error)?;
        gate.recv_timeout(Duration::from_secs(5))
            .map_err(synthetic_error)?;
        Ok(())
    })?;
    observed.recv_timeout(Duration::from_secs(3))?;
    let queued = database.transaction(WorkCancellation::default(), |tx| {
        tx.execute("INSERT INTO worker_parent VALUES(1)", [])?;
        Ok(())
    })?;
    release.send(())?;
    assert!(matches!(
        attempt.wait()?,
        TransactionOutcome::CommitUncertain(_)
    ));
    assert!(matches!(
        queued.wait()?,
        TransactionOutcome::ProvenNotCommitted(StorageError::RuntimeFenced)
    ));
    assert!(matches!(
        database.transaction(WorkCancellation::default(), |_| Ok(())),
        Err(StorageError::RuntimeFenced)
    ));
    let truth = database
        .barrier_read(|tx| {
            Ok((
                tx.query_row("SELECT count(*) FROM worker_child", [], |row| {
                    row.get::<_, i64>(0)
                })?,
                tx.query_row("SELECT count(*) FROM worker_parent", [], |row| {
                    row.get::<_, i64>(0)
                })?,
            ))
        })?
        .wait()?;
    assert_eq!(truth, (0, 0));
    committed(
        database
            .repair_transaction(|tx| {
                tx.execute("INSERT INTO worker_parent VALUES(2)", [])?;
                Ok(())
            })?
            .wait()?,
    )?;
    database.close()?;
    Ok(())
}

#[test]
fn existing_foreign_database_and_sidecars_are_refused_before_sqlite_open() -> TestResult {
    let roots = TestRoots::new()?;
    let library = isolated(&roots)?;
    let path = library.layout().database();
    fs::write(&path, b"foreign-format\0preserved")?;
    let wal = path.with_file_name("librairii.sqlite3-wal");
    let shm = path.with_file_name("librairii.sqlite3-shm");
    fs::write(&wal, b"exact-WAL")?;
    fs::write(&shm, b"exact-SHM")?;
    assert!(matches!(
        Database::open_isolated(&library),
        Err(StorageError::ForeignSchema)
    ));
    assert_eq!(fs::read(path)?, b"foreign-format\0preserved");
    assert_eq!(fs::read(wal)?, b"exact-WAL");
    assert_eq!(fs::read(shm)?, b"exact-SHM");
    Ok(())
}

#[test]
fn queued_work_cancel_is_proven_noncommit_without_cancelling_prior_critical_work() -> TestResult {
    let roots = TestRoots::new()?;
    let library = isolated(&roots)?;
    let mut database = Database::open_isolated(&library)?;
    let (entered, observed) = mpsc::channel();
    let (release, gate) = mpsc::channel();
    let prior = database.repair_transaction(move |tx| {
        tx.execute_batch(
            "CREATE TABLE queued_cancel(value INTEGER); INSERT INTO queued_cancel VALUES(1)",
        )?;
        entered.send(()).map_err(synthetic_error)?;
        gate.recv_timeout(Duration::from_secs(5))
            .map_err(synthetic_error)?;
        Ok(())
    })?;
    observed.recv_timeout(Duration::from_secs(3))?;
    let cancellation = WorkCancellation::default();
    let queued = database.transaction(cancellation.clone(), |tx| {
        tx.execute("INSERT INTO queued_cancel VALUES(2)", [])?;
        Ok(())
    })?;
    cancellation.cancel();
    release.send(())?;
    committed(prior.wait()?)?;
    assert!(matches!(
        queued.wait()?,
        TransactionOutcome::ProvenNotCommitted(StorageError::Cancelled)
    ));
    assert_eq!(
        database
            .barrier_read(|tx| Ok(tx.query_row(
                "SELECT sum(value) FROM queued_cancel",
                [],
                |row| row.get::<_, i64>(0)
            )?))?
            .wait()?,
        1
    );
    database.close()?;
    Ok(())
}

#[test]
fn injected_lost_commit_ack_preserves_committed_bytes_and_reports_uncertainty() -> TestResult {
    let roots = TestRoots::new()?;
    let library = isolated(&roots)?;
    let mut database = Database::open_isolated(&library)?;
    let outcome=database.transaction(WorkCancellation::default(),|tx| {
        tx.execute_batch("CREATE TABLE committed_truth(value BLOB); INSERT INTO committed_truth VALUES(x'0001FF')")?;
        // Fault injection only: commit the actual SQLite transaction, swallow
        // that positive acknowledgement, then the mechanism's COMMIT reports
        // an error. Production callbacks must never perform transaction control.
        tx.execute_batch("COMMIT")?;
        Ok(())
    })?.wait()?;
    assert!(matches!(outcome, TransactionOutcome::CommitUncertain(_)));
    let bytes = database
        .barrier_read(|tx| {
            Ok(
                tx.query_row("SELECT value FROM committed_truth", [], |row| {
                    row.get::<_, Vec<u8>>(0)
                })?,
            )
        })?
        .wait()?;
    assert_eq!(bytes, vec![0, 1, 255]);
    database.close()?;
    Ok(())
}

#[test]
#[expect(
    clippy::panic,
    reason = "Intentional worker unwind injection verifies unavailable acknowledgements, queue wakeup and fenced close"
)]
fn unexpected_worker_exit_wakes_queued_results_and_never_claims_closed() -> TestResult {
    let roots = TestRoots::new()?;
    let library = isolated(&roots)?;
    let mut database = Database::open_isolated(&library)?;
    let (entered, observed) = mpsc::channel();
    let (release, gate) = mpsc::channel();
    let failed = database.transaction(
        WorkCancellation::default(),
        move |_| -> librairii_storage::Result<()> {
            entered.send(()).map_err(synthetic_error)?;
            gate.recv_timeout(Duration::from_secs(5))
                .map_err(synthetic_error)?;
            panic!("synthetic worker failure");
        },
    )?;
    observed.recv_timeout(Duration::from_secs(3))?;
    let queued = database.repair_transaction(|_| Ok(()))?;
    release.send(())?;
    assert!(matches!(failed.wait(), Err(StorageError::WorkerStopped)));
    assert!(matches!(queued.wait(), Err(StorageError::WorkerStopped)));
    assert!(matches!(database.close(), Err(StorageError::WorkerStopped)));
    assert!(matches!(database.close(), Err(StorageError::RuntimeFenced)));
    assert!(matches!(
        Database::open_isolated(&library),
        Err(StorageError::RuntimeInUse)
    ));
    Ok(())
}

#[test]
fn completed_unacknowledged_reads_retain_bounded_result_ownership() -> TestResult {
    let roots = TestRoots::new()?;
    let library = isolated(&roots)?;
    let mut database = Database::open_isolated(&library)?;
    let (finished, observed) = mpsc::channel();
    let mut pending = Vec::new();
    for _ in 0..ORDINARY_QUEUE_CAPACITY + READ_WORKERS {
        let finished = finished.clone();
        pending.push(database.read(move |_| {
            finished.send(()).map_err(synthetic_error)?;
            Ok(vec![0u8; 1024])
        })?);
        observed.recv_timeout(Duration::from_secs(3))?;
    }
    assert!(matches!(
        database.read(|_| Ok(())),
        Err(StorageError::QueueFull)
    ));
    // Acknowledgement releases the result slot; queue completion alone did not.
    let first = pending
        .pop()
        .ok_or_else(|| std::io::Error::other("missing result"))?;
    assert_eq!(first.wait()?.len(), 1024);
    database.read(|_| Ok(()))?.wait()?;
    for result in pending {
        assert_eq!(result.wait()?.len(), 1024);
    }
    database.close()?;
    Ok(())
}

#[test]
fn dropping_result_does_not_cancel_durable_repair_and_close_drains_it() -> TestResult {
    let roots = TestRoots::new()?;
    let library = isolated(&roots)?;
    let mut database = Database::open_isolated(&library)?;
    let (finished, observed) = mpsc::channel();
    database.stop_ordinary()?;
    let pending = database.repair_transaction(move |tx| {
        tx.execute_batch(
            "CREATE TABLE worker_drained(value INTEGER); INSERT INTO worker_drained VALUES(7)",
        )?;
        finished.send(()).map_err(synthetic_error)?;
        Ok(())
    })?;
    drop(pending);
    database.close()?;
    observed.recv_timeout(Duration::from_secs(3))?;
    let connection = rusqlite::Connection::open_with_flags(
        library.layout().database(),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?;
    assert_eq!(
        connection.query_row("SELECT value FROM worker_drained", [], |row| row
            .get::<_, i64>(0))?,
        7
    );
    drop(connection);
    Ok(())
}

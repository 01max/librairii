//! Owned synchronous SQLite mechanisms for storage adapters.
//!
//! No connection, row, statement or transaction can escape an admitted closure.
//! App/core ports carry owned domain DTOs; their implementations live in this
//! crate and use these mechanisms. Closures are bounded DB-only work: no I/O,
//! await, recursive queue admission or network/copy callbacks.
use crate::{
    IsolatedLibrary, ManagedPath, Result, StorageError,
    migrations::{LEDGER_SQL, MIGRATIONS},
};
use rusqlite::{Connection, OpenFlags, Transaction, TransactionBehavior};
use std::{
    collections::VecDeque,
    fs::OpenOptions,
    marker::PhantomData,
    path::{Path, PathBuf},
    sync::{
        Arc, Condvar, Mutex, Weak,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

pub const READ_WORKERS: usize = 4;
pub const ORDINARY_QUEUE_CAPACITY: usize = 16;
pub const REPAIR_QUEUE_CAPACITY: usize = 4;
pub const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// Cancellation applies at admission and precommit, never to acknowledgement
/// or required repair. It does not promise interruption inside a running SQL
/// statement; domain SQL work must have independently bounded work.
#[derive(Clone, Debug, Default)]
pub struct WorkCancellation(Arc<AtomicBool>);
impl WorkCancellation {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

/// This is transient evidence, not a serialized status or a retry instruction.
#[derive(Debug)]
pub enum TransactionOutcome<T> {
    Committed(T),
    ProvenNotCommitted(StorageError),
    /// Preserve custody; perform fresh writer-barrier readback before deciding.
    CommitUncertain(StorageError),
}

/// An owned result borrows the runtime until acknowledged/dropped. A dropped
/// receiver does not cancel admitted effects; close still drains and joins them.
#[derive(Debug)]
pub struct Pending<'runtime, T> {
    receiver: Receiver<Result<T>>,
    _admission: Arc<Admission>,
    lifetime: PhantomData<&'runtime ()>,
}
impl<T> Pending<'_, T> {
    /// Disconnection means an unavailable outcome, never rollback proof.
    pub fn wait(self) -> Result<T> {
        self.receiver
            .recv()
            .map_err(|_| StorageError::WorkerStopped)?
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConnectionDiagnostics {
    pub worker: String,
    pub sqlite_version: String,
    pub sqlite_source_id: String,
    pub journal_mode: String,
    pub foreign_keys: i64,
    pub busy_timeout_ms: i64,
    pub query_only: i64,
}

type Job = Box<dyn FnOnce(&mut Connection) + Send + 'static>;
struct Entry {
    critical: bool,
    job: Job,
}
struct QueueState {
    entries: VecDeque<Entry>,
    ordinary: usize,
    critical: usize,
    ordinary_owned: usize,
    critical_owned: usize,
    ordinary_open: bool,
    stopping: bool,
}
struct Queue {
    state: Mutex<QueueState>,
    ready: Condvar,
    repair_capacity: usize,
    workers: usize,
}
#[derive(Debug)]
struct Admission {
    queue: Weak<Queue>,
    critical: bool,
}
impl Drop for Admission {
    fn drop(&mut self) {
        if let Some(queue) = self.queue.upgrade()
            && let Ok(mut state) = queue.state.lock()
        {
            if self.critical {
                state.critical_owned -= 1;
            } else {
                state.ordinary_owned -= 1;
            }
        }
    }
}
impl std::fmt::Debug for Queue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Queue")
            .field("ordinary_capacity", &ORDINARY_QUEUE_CAPACITY)
            .field("repair_capacity", &self.repair_capacity)
            .finish_non_exhaustive()
    }
}
impl Queue {
    fn new(repair_capacity: usize, workers: usize) -> Self {
        Self {
            state: Mutex::new(QueueState {
                entries: VecDeque::new(),
                ordinary: 0,
                critical: 0,
                ordinary_owned: 0,
                critical_owned: 0,
                ordinary_open: true,
                stopping: false,
            }),
            ready: Condvar::new(),
            repair_capacity,
            workers,
        }
    }
    fn submit<T: Send + 'static>(
        self: &Arc<Self>,
        critical: bool,
        work: impl FnOnce(&mut Connection) -> Result<T> + Send + 'static,
        sender: mpsc::SyncSender<Result<T>>,
    ) -> Result<Arc<Admission>> {
        let mut state = self.state.lock().map_err(|_| StorageError::RuntimeFenced)?;
        if state.stopping || (!critical && !state.ordinary_open) {
            return Err(StorageError::IntakeClosed);
        }
        if (critical && state.critical >= self.repair_capacity)
            || (!critical && state.ordinary >= ORDINARY_QUEUE_CAPACITY)
            || (critical && state.critical_owned >= self.repair_capacity)
            || (!critical && state.ordinary_owned >= ORDINARY_QUEUE_CAPACITY + self.workers)
        {
            return Err(StorageError::QueueFull);
        }
        if critical {
            state.critical += 1;
            state.critical_owned += 1;
        } else {
            state.ordinary += 1;
            state.ordinary_owned += 1;
        }
        let admission = Arc::new(Admission {
            queue: Arc::downgrade(self),
            critical,
        });
        let execution = admission.clone();
        // One FIFO retains ordering even when reserved repair capacity is used.
        state.entries.push_back(Entry {
            critical,
            job: Box::new(move |connection| {
                let result = work(connection);
                drop(execution);
                // Execution ownership ends before acknowledgement is visible;
                // the Pending owner still retains the bounded result slot.
                let _ = sender.send(result);
            }),
        });
        self.ready.notify_one();
        Ok(admission)
    }
    fn next(&self) -> Result<Option<Job>> {
        let mut state = self.state.lock().map_err(|_| StorageError::RuntimeFenced)?;
        loop {
            if let Some(entry) = state.entries.pop_front() {
                if entry.critical {
                    state.critical -= 1;
                } else {
                    state.ordinary -= 1;
                }
                return Ok(Some(entry.job));
            }
            if state.stopping {
                return Ok(None);
            }
            state = self
                .ready
                .wait(state)
                .map_err(|_| StorageError::RuntimeFenced)?;
        }
    }
    fn stop_ordinary(&self) -> Result<()> {
        self.state
            .lock()
            .map_err(|_| StorageError::RuntimeFenced)?
            .ordinary_open = false;
        Ok(())
    }
    fn stop_all(&self) -> Result<()> {
        let mut state = self.state.lock().map_err(|_| StorageError::RuntimeFenced)?;
        state.ordinary_open = false;
        state.stopping = true;
        self.ready.notify_all();
        Ok(())
    }
    fn abort(&self) {
        let discarded = if let Ok(mut state) = self.state.lock() {
            state.ordinary_open = false;
            state.stopping = true;
            state.ordinary = 0;
            state.critical = 0;
            self.ready.notify_all();
            std::mem::take(&mut state.entries)
        } else {
            VecDeque::new()
        };
        drop(discarded);
    }
}

// On an unexpected worker exit, queued result senders must be dropped so that
// waiting callers receive an unavailable outcome instead of hanging forever.
// No queued callback runs under the queue mutex, including during unwinding.
struct WorkerExit {
    queue: Arc<Queue>,
    fenced: Arc<AtomicBool>,
    normal: bool,
}
impl Drop for WorkerExit {
    fn drop(&mut self) {
        if !self.normal {
            self.fenced.store(true, Ordering::Release);
            self.queue.abort();
        }
    }
}

/// The authority must outlive all worker joins. Initial4.3 opening accepts only
/// an absent DB beneath a freshly authorized isolated root; qualified existing
/// open/backup/adoption is added by the subsequent dedicated subtasks.
#[derive(Debug)]
pub struct Database<'library> {
    authority: &'library IsolatedLibrary,
    writer: Arc<Queue>,
    readers: Arc<Queue>,
    writer_thread: Option<JoinHandle<Result<()>>>,
    read_threads: Vec<JoinHandle<Result<()>>>,
    diagnostics: Vec<ConnectionDiagnostics>,
    fenced: Arc<AtomicBool>,
    closed: bool,
    close_failed: bool,
}

impl<'library> Database<'library> {
    pub fn open_isolated(authority: &'library IsolatedLibrary) -> Result<Self> {
        authority.claim_runtime()?;
        let mut runtime = Self {
            authority,
            writer: Arc::new(Queue::new(REPAIR_QUEUE_CAPACITY, 1)),
            readers: Arc::new(Queue::new(0, READ_WORKERS)),
            writer_thread: None,
            read_threads: Vec::new(),
            diagnostics: Vec::new(),
            fenced: Arc::new(AtomicBool::new(false)),
            closed: false,
            close_failed: false,
        };
        let path = authority
            .layout()
            .resolve(&ManagedPath::parse("db/librairii.sqlite3")?)?;
        let (sender, receiver) = mpsc::sync_channel(1);
        let queue = runtime.writer.clone();
        let fenced = runtime.fenced.clone();
        let writer_path = path.clone();
        runtime.writer_thread = Some(
            thread::Builder::new()
                .name("librairii-db-writer".into())
                .spawn(move || {
                    let mut exit = WorkerExit {
                        queue: queue.clone(),
                        fenced,
                        normal: false,
                    };
                    let mut connection = match open_new_writer(&writer_path) {
                        Ok(connection) => connection,
                        Err(error) => {
                            let _ = sender.send(Err(error));
                            return Ok(());
                        }
                    };
                    let _ = sender.send(diagnostics(&connection, "writer"));
                    while let Some(job) = queue.next()? {
                        job(&mut connection);
                    }
                    let (busy, _, _): (i64, i64, i64) =
                        connection.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| {
                            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
                        })?;
                    if busy != 0 {
                        return Err(StorageError::RuntimeFenced);
                    }
                    let result = connection
                        .close()
                        .map_err(|(_, error)| StorageError::Sqlite(error));
                    exit.normal = result.is_ok();
                    result
                })
                .map_err(|e| StorageError::io("start SQLite writer", e))?,
        );
        runtime
            .diagnostics
            .push(receiver.recv().map_err(|_| StorageError::WorkerStopped)??);
        for index in 0..READ_WORKERS {
            let (sender, receiver) = mpsc::sync_channel(1);
            let queue = runtime.readers.clone();
            let fenced = runtime.fenced.clone();
            let path = path.clone();
            runtime.read_threads.push(
                thread::Builder::new()
                    .name(format!("librairii-db-reader-{index}"))
                    .spawn(move || {
                        let mut exit = WorkerExit {
                            queue: queue.clone(),
                            fenced,
                            normal: false,
                        };
                        let mut connection = match open_reader(&path) {
                            Ok(connection) => connection,
                            Err(error) => {
                                let _ = sender.send(Err(error));
                                return Ok(());
                            }
                        };
                        let _ = sender.send(diagnostics(&connection, &format!("reader-{index}")));
                        while let Some(job) = queue.next()? {
                            job(&mut connection);
                        }
                        let result = connection
                            .close()
                            .map_err(|(_, error)| StorageError::Sqlite(error));
                        exit.normal = result.is_ok();
                        result
                    })
                    .map_err(|e| StorageError::io("start SQLite reader", e))?,
            );
            runtime
                .diagnostics
                .push(receiver.recv().map_err(|_| StorageError::WorkerStopped)??);
        }
        Ok(runtime)
    }

    pub fn diagnostics(&self) -> &[ConnectionDiagnostics] {
        &self.diagnostics
    }

    /// Storage adapter-only SQL callback; the owned result cannot borrow SQL.
    /// ```compile_fail
    /// use librairii_storage::database::Database;
    /// fn escape(db: &Database<'_>) {
    ///     let _ = db.read(|transaction| Ok(transaction));
    /// }
    /// ```
    pub fn read<T: Send + 'static>(
        &self,
        work: impl FnOnce(&Transaction<'_>) -> Result<T> + Send + 'static,
    ) -> Result<Pending<'_, T>> {
        self.enqueue(&self.readers, false, move |connection| {
            let transaction = connection.transaction()?;
            let result = work(&transaction)?;
            transaction.commit()?;
            Ok(result)
        })
    }

    /// Cross all earlier writer admissions, then read one fresh coherent view
    /// on the SAME writer. Reserved capacity remains available after intake stop.
    /// Domain matching/acknowledgement predicates belong to the repository owner.
    pub fn barrier_read<T: Send + 'static>(
        &self,
        work: impl FnOnce(&Transaction<'_>) -> Result<T> + Send + 'static,
    ) -> Result<Pending<'_, T>> {
        self.enqueue(&self.writer, true, move |connection| {
            if !connection.is_autocommit() {
                return Err(StorageError::RuntimeFenced);
            }
            let transaction = connection.transaction()?;
            let result = work(&transaction)?;
            transaction.commit()?;
            Ok(result)
        })
    }

    pub fn transaction<T: Send + 'static>(
        &self,
        cancellation: WorkCancellation,
        work: impl FnOnce(&Transaction<'_>) -> Result<T> + Send + 'static,
    ) -> Result<Pending<'_, TransactionOutcome<T>>> {
        if self.fenced.load(Ordering::Acquire) {
            return Err(StorageError::RuntimeFenced);
        }
        self.write_job(false, Some(cancellation), work)
    }

    /// Required durable report/repair is independent of work cancellation.
    /// Reserved bounded admission is not a retry; QueueFull remains explicit.
    pub fn repair_transaction<T: Send + 'static>(
        &self,
        work: impl FnOnce(&Transaction<'_>) -> Result<T> + Send + 'static,
    ) -> Result<Pending<'_, TransactionOutcome<T>>> {
        self.write_job(true, None, work)
    }

    fn write_job<T: Send + 'static>(
        &self,
        critical: bool,
        cancellation: Option<WorkCancellation>,
        work: impl FnOnce(&Transaction<'_>) -> Result<T> + Send + 'static,
    ) -> Result<Pending<'_, TransactionOutcome<T>>> {
        let fence = self.fenced.clone();
        self.enqueue(&self.writer, critical, move |connection| {
            if !critical && fence.load(Ordering::Acquire) {
                return Ok(TransactionOutcome::ProvenNotCommitted(
                    StorageError::RuntimeFenced,
                ));
            }
            let outcome = run_transaction(connection, cancellation, work);
            if matches!(outcome, TransactionOutcome::CommitUncertain(_)) {
                fence.store(true, Ordering::Release);
            }
            Ok(outcome)
        })
    }

    fn enqueue<T: Send + 'static>(
        &self,
        queue: &Arc<Queue>,
        critical: bool,
        work: impl FnOnce(&mut Connection) -> Result<T> + Send + 'static,
    ) -> Result<Pending<'_, T>> {
        if self.closed || self.close_failed {
            return Err(StorageError::IntakeClosed);
        }
        let (sender, receiver) = mpsc::sync_channel(1);
        let admission = queue.submit(critical, work, sender)?;
        Ok(Pending {
            receiver,
            _admission: admission,
            lifetime: PhantomData,
        })
    }

    pub fn stop_ordinary(&self) -> Result<()> {
        self.writer.stop_ordinary()?;
        self.readers.stop_ordinary()
    }

    /// The runtime coordinator first settles reports/readback/compensation and
    /// drains external readers/artwork. Exclusive &mut ownership prevents new
    /// admissions during close. An error retains a fenced authority; it is not
    /// successful closure or permission to activate a replacement generation.
    pub fn close(&mut self) -> Result<()> {
        if self.closed {
            return Ok(());
        }
        if self.close_failed {
            return Err(StorageError::RuntimeFenced);
        }
        self.stop_ordinary()?;
        self.readers.stop_all()?;
        let mut failure = None;
        for thread in self.read_threads.drain(..) {
            if let Err(error) = join(thread) {
                failure.get_or_insert(error);
            }
        }
        self.writer.stop_all()?;
        if let Some(thread) = self.writer_thread.take()
            && let Err(error) = join(thread)
        {
            failure.get_or_insert(error);
        }
        if let Some(error) = failure {
            self.close_failed = true;
            return Err(error);
        }
        self.closed = true;
        self.authority.release_runtime();
        Ok(())
    }
}

impl Drop for Database<'_> {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

fn join(thread: JoinHandle<Result<()>>) -> Result<()> {
    thread.join().map_err(|_| StorageError::WorkerStopped)?
}

fn run_transaction<T>(
    connection: &mut Connection,
    cancellation: Option<WorkCancellation>,
    work: impl FnOnce(&Transaction<'_>) -> Result<T>,
) -> TransactionOutcome<T> {
    let cancelled = || {
        cancellation
            .as_ref()
            .is_some_and(WorkCancellation::is_cancelled)
    };
    if cancelled() {
        return TransactionOutcome::ProvenNotCommitted(StorageError::Cancelled);
    }
    let transaction = match connection.transaction_with_behavior(TransactionBehavior::Immediate) {
        Ok(transaction) => transaction,
        Err(error) => return TransactionOutcome::ProvenNotCommitted(StorageError::Sqlite(error)),
    };
    let result = match work(&transaction) {
        Ok(result) => result,
        Err(error) => return rollback(transaction, error),
    };
    if cancelled() {
        return rollback(transaction, StorageError::Cancelled);
    }
    match transaction.commit() {
        Ok(()) => TransactionOutcome::Committed(result),
        // commit()'s error (including a subsequent implicit Drop rollback) is
        // not an acknowledgement of rollback. Never turn it into a blind retry.
        Err(error) => TransactionOutcome::CommitUncertain(StorageError::Sqlite(error)),
    }
}

fn rollback<T>(transaction: Transaction<'_>, work: StorageError) -> TransactionOutcome<T> {
    match transaction.rollback() {
        Ok(()) => TransactionOutcome::ProvenNotCommitted(work),
        Err(rollback) => TransactionOutcome::CommitUncertain(StorageError::RollbackUncertain {
            work: Box::new(work),
            rollback,
        }),
    }
}

fn open_new_writer(path: &Path) -> Result<Connection> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    match options.open(path) {
        Ok(reservation) => drop(reservation),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            return Err(StorageError::QualifiedExistingOpenRequired);
        }
        Err(error) => return Err(StorageError::io("reserve isolated database", error)),
    }
    let mut connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    connection.busy_timeout(BUSY_TIMEOUT)?;
    connection.pragma_update(None, "foreign_keys", true)?;
    connection.pragma_update(None, "journal_mode", "WAL")?;
    connection.execute_batch(LEDGER_SQL)?;
    for migration in MIGRATIONS {
        let transaction = connection.transaction()?;
        transaction.execute_batch(migration.sql)?;
        transaction.execute(
            "INSERT INTO schema_migrations(version) VALUES(?)",
            [migration.version],
        )?;
        transaction.commit()?;
    }
    Ok(connection)
}

fn open_reader(path: &PathBuf) -> Result<Connection> {
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    connection.busy_timeout(BUSY_TIMEOUT)?;
    connection.pragma_update(None, "foreign_keys", true)?;
    connection.pragma_update(None, "query_only", true)?;
    Ok(connection)
}

fn diagnostics(connection: &Connection, worker: &str) -> Result<ConnectionDiagnostics> {
    let diagnostic = ConnectionDiagnostics {
        worker: worker.into(),
        sqlite_version: connection.query_row("SELECT sqlite_version()", [], |row| row.get(0))?,
        sqlite_source_id: connection
            .query_row("SELECT sqlite_source_id()", [], |row| row.get(0))?,
        journal_mode: connection.pragma_query_value(None, "journal_mode", |row| row.get(0))?,
        foreign_keys: connection.pragma_query_value(None, "foreign_keys", |row| row.get(0))?,
        busy_timeout_ms: connection.pragma_query_value(None, "busy_timeout", |row| row.get(0))?,
        query_only: connection.pragma_query_value(None, "query_only", |row| row.get(0))?,
    };
    if diagnostic.journal_mode != "wal"
        || diagnostic.foreign_keys != 1
        || diagnostic.busy_timeout_ms != 5000
        || diagnostic.query_only != i64::from(worker != "writer")
    {
        return Err(StorageError::RuntimeFenced);
    }
    Ok(diagnostic)
}

//! TEMP C2 composition proof only. This file must never enter a shipping branch.
//! Every persistence reply originates in the same real R4 repository/Database;
//! the facade adds observation barriers or deliberately loses a real reply.
use librairii_app::{
    jobs::scoped::{
        self, Callbacks, Configuration, Delivery, Request, Retained, RunControl, RunOutcome,
        WorkReply,
    },
    ports::operations::*,
};
use librairii_core::{
    OperationState,
    operations::{Counts, ItemState, RawOutcome},
};
use librairii_storage::{
    IsolatedLibrary,
    database::{Database, TransactionOutcome},
    operations_repository::TerminalRepository,
};
use librairii_testkit::TestRoots;
use std::{
    collections::VecDeque,
    error::Error,
    io,
    sync::{
        Condvar, Mutex, MutexGuard,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

type TestResult<T = ()> = Result<T, Box<dyn Error + Send + Sync>>;
const GENERATION: u64 = 17;

fn lock<T>(value: &Mutex<T>) -> MutexGuard<'_, T> {
    value.lock().unwrap_or_else(|error| error.into_inner())
}

/// Deliberate test-only unwinding at the public callback boundary. This uses
/// the standard payload-resume API and does not replace a panic hook globally.
fn injected_unwind(message: &'static str) -> ! {
    std::panic::resume_unwind(Box::new(message))
}

fn owned_library() -> TestResult<(TestRoots, IsolatedLibrary)> {
    let roots = TestRoots::new()?;
    let library = IsolatedLibrary::initialize_from_override(
        Some(roots.library().as_os_str()),
        "A0000000-0000-4000-8000-000000000001".parse()?,
    )?;
    Ok((roots, library))
}

fn committed<T>(outcome: TransactionOutcome<T>) -> TestResult<T> {
    match outcome {
        TransactionOutcome::Committed(value) => Ok(value),
        TransactionOutcome::ProvenNotCommitted(error)
        | TransactionOutcome::CommitUncertain(error) => Err(error.into()),
    }
}

fn raw_id(index: usize) -> String {
    format!("legacy{index:030}")
}

fn seed(db: &Database<'_>, rows: &[(String, i64)]) -> TestResult {
    let rows = rows.to_vec();
    committed(
        db.repair_transaction(move |tx| {
            tx.execute(
                "INSERT INTO stories(id,uuid) VALUES(42,'A0000000-0000-4000-8000-000000000042')",
                [],
            )?;
            for (id, item) in rows {
                tx.execute(
                    "INSERT INTO file_operations(id,kind,status,total_items,total_bytes,created_at,
                started_at,export_source_type,export_source_shelf_ids,export_source_shelf_names,
                export_destination,error_code,error_message)
                VALUES(?,'import','queued',1,10,'raw CREATED+02','','shelves','[9, 2]',
                '[\"Été\",\"Été\"] ',' /raw # %2F destination ','',NULL)",
                    [&id],
                )?;
                tx.execute(
                    "INSERT INTO file_operation_items(id,operation_id,resolved_story_id,story_uuid,
                story_title,source_name,output_name,archive_relative_path,archive_sha256,
                status,completed_bytes,total_bytes,outcome_code,outcome_message)
                VALUES(?,?,41,'A0000000-0000-4000-8000-00000000000B','','Été # %2F.zip',
                NULL,'archives/RAW.zip',?,'pending',9,10,NULL,'')",
                    rusqlite::params![item, id, "Ab".repeat(32)],
                )?;
            }
            Ok(())
        })?
        .wait()?,
    )?;
    Ok(())
}

fn snapshot(store: &dyn OperationTerminalStore, id: &str) -> TestResult<Box<OperationSnapshot>> {
    match store.snapshot(id) {
        SnapshotReply::Found(value) => Ok(value),
        other => Err(io::Error::other(format!("real snapshot unavailable: {other:?}")).into()),
    }
}

fn request(store: &dyn OperationTerminalStore, id: &str) -> TestResult<Request> {
    let value = snapshot(store, id)?;
    let item = value
        .items
        .first()
        .ok_or_else(|| io::Error::other("seeded item absent"))?;
    let operation = value.operation.immutable_scope();
    Ok(Request {
        complete: CompleteItemCommand {
            generation: GENERATION,
            operation: operation.clone(),
            item: item.immutable_scope(),
            story_id: if item.identity.id == 0 { 0 } else { 42 },
            state: ItemState::Succeeded,
            completed_bytes: 1,
            outcome: RawOutcome {
                code: Some(String::new()),
                message: Some(" raw item cause ".into()),
            },
        },
        finish: Some(FinishCommand {
            generation: GENERATION,
            operation: operation.clone(),
            state: OperationState::Succeeded,
            outcome: RawOutcome {
                code: Some(String::new()),
                message: Some(" raw finish ".into()),
            },
            finished_at: "RAW planned finish # %2F".into(),
        }),
        interrupt: InterruptCommand {
            generation: GENERATION,
            operation,
            code: " ".into(),
            message: " raw supplied interruption ".into(),
            finished_at: "RAW supplied interrupt".into(),
        },
    })
}

fn completed(request: &Request, _: &RunControl) -> WorkReply {
    WorkReply::Completed {
        command: Box::new(request.complete.clone()),
        diagnostic: Some(StoreDiagnostic::new(io::Error::other(
            "independent work cause",
        ))),
    }
}

#[derive(Clone, Debug)]
struct Delivered {
    execution: Option<CompleteItemCommand>,
    witness: CommandWitness,
    state: Option<OperationState>,
    complete_acknowledged: bool,
    unknown: bool,
    work_panicked: bool,
    work_cause: Option<String>,
}

fn record_delivery(values: &Mutex<Vec<Delivered>>, owner: &Retained) -> Delivery {
    lock(values).push(Delivered {
        execution: owner.execution.clone(),
        witness: owner.witness.clone(),
        state: owner.view.as_ref().map(|view| view.operation.state),
        complete_acknowledged: owner.complete_acknowledged,
        unknown: owner.unknown,
        work_panicked: owner.work_panicked_before_db,
        work_cause: owner.work_error.as_ref().map(ToString::to_string),
    });
    Delivery::Accepted
}

/// A coordinator continues the SAME returned owner only after all H workers
/// have joined. The caller holds this fixture's exclusive claim guard across
/// real writes, exact fresh readback, and actual controlled report acceptance.
/// This guard is test exclusion, never a production custody/activation token.
fn resolve_returned_owner(
    real: &dyn OperationTerminalStore,
    owner: &mut Retained,
    _exclusive: &MutexGuard<'_, ()>,
    reports: &Mutex<Vec<Delivered>>,
) -> TestResult {
    assert!(!owner.unknown);
    assert!(!owner.complete_acknowledged);
    let original_work_cause = owner.work_error.as_ref().map(ToString::to_string);
    let command = owner
        .execution
        .as_ref()
        .ok_or_else(|| io::Error::other("returned owner lost executed command"))?
        .clone();
    assert_eq!(command, owner.request.complete);
    let witness = CommandWitness::CompleteItem(Box::new(command.clone()));
    let applied = real.complete_item(&command);
    assert_eq!(applied.witness, witness);
    assert!(matches!(
        applied.disposition,
        AttemptDisposition::Committed { .. }
    ));
    assert!(
        applied.diagnostic.is_none(),
        "unexpected completion cause: {:?}",
        applied.diagnostic
    );
    let complete = exact_fresh_view(real, &witness)?;
    assert_eq!(
        complete.operation.counts,
        Counts {
            completed: 1,
            total: 1
        }
    );
    assert_eq!(complete.siblings.items, 1);
    assert_eq!(complete.siblings.terminal, 1);
    let item = complete
        .item
        .as_ref()
        .ok_or_else(|| io::Error::other("complete readback omitted item"))?;
    assert_eq!(item.immutable_scope(), command.item);
    assert_eq!(item.identity.story_id, command.stored_story_id());
    assert_eq!(item.state, command.state);
    assert_eq!(item.progress.completed, command.completed_bytes);
    assert_eq!(item.outcome, command.stored_outcome());

    // The original request explicitly supplied Finish. Completion/readback alone
    // is not terminal reporting or permission to close this fixture's DB.
    let finish = owner
        .request
        .finish
        .as_ref()
        .ok_or_else(|| io::Error::other("required Finish plan absent"))?;
    let terminal_witness = CommandWitness::Finish(Box::new(finish.clone()));
    let finished = real.finish(finish);
    assert_eq!(finished.witness, terminal_witness);
    assert!(matches!(
        finished.disposition,
        AttemptDisposition::Committed { .. }
    ));
    assert!(
        finished.diagnostic.is_none(),
        "unexpected Finish cause: {:?}",
        finished.diagnostic
    );
    let terminal = exact_fresh_view(real, &terminal_witness)?;
    assert_eq!(terminal.operation.immutable_scope(), finish.operation);
    assert_eq!(terminal.operation.state, finish.state);
    assert_eq!(terminal.operation.outcome, finish.stored_outcome());
    assert_eq!(
        terminal.operation.identity.finished_at.as_ref(),
        Some(&finish.finished_at)
    );
    assert_eq!(
        terminal.operation.counts,
        Counts {
            completed: 1,
            total: 1
        }
    );
    assert_eq!(terminal.siblings.items, 1);
    assert_eq!(terminal.siblings.terminal, 1);

    // These fields advance from the actual acknowledgements above. No supplied
    // DTO, dropped RunOutcome, joined bit or synthetic state is used as proof.
    owner.complete_acknowledged = true;
    owner.witness = terminal_witness.clone();
    owner.view = Some(terminal);
    assert_eq!(
        owner.work_error.as_ref().map(ToString::to_string),
        original_work_cause
    );
    assert_eq!(record_delivery(reports, owner), Delivery::Accepted);
    let accepted = lock(reports);
    let last = accepted
        .last()
        .ok_or_else(|| io::Error::other("report sink accepted nothing"))?;
    assert_eq!(last.execution.as_ref(), Some(&command));
    assert_eq!(last.witness, terminal_witness);
    assert_eq!(last.state, Some(OperationState::Succeeded));
    assert!(last.complete_acknowledged && !last.unknown);
    assert_eq!(last.work_cause, original_work_cause);
    Ok(())
}

fn exact_fresh_view(
    real: &dyn OperationTerminalStore,
    witness: &CommandWitness,
) -> TestResult<Box<TerminalView>> {
    let reply = real.readback(witness);
    assert_eq!(&reply.witness, witness);
    assert!(
        reply.diagnostic.is_none(),
        "unexpected fresh-readback cause: {:?}",
        reply.diagnostic
    );
    match reply.disposition {
        ReadbackDisposition::MatchesApplied(view) => Ok(view),
        other => {
            Err(io::Error::other(format!("fresh exact command not applied: {other:?}")).into())
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum ReplyLoss {
    None,
    UnknownAfterComplete,
    PanicAfterComplete,
}

/// Every method delegates to this one borrowed real repository. No fake view,
/// nonapplication, conflict or commit acknowledgement is manufactured here.
struct TracedStore<'a> {
    real: &'a (dyn OperationTerminalStore + Sync),
    loss: ReplyLoss,
    complete_calls: AtomicUsize,
    finish_calls: AtomicUsize,
    interrupt_calls: AtomicUsize,
    read_calls: AtomicUsize,
    snapshot_calls: AtomicUsize,
    refused: AtomicUsize,
    nonapplied: AtomicUsize,
    committed_without_view: AtomicUsize,
    active_calls: AtomicUsize,
    attempt_hook: Option<&'a (dyn Fn(&AttemptReply) + Sync)>,
    read_hook: Option<&'a (dyn Fn(&ReadbackReply) + Sync)>,
}

impl<'a> TracedStore<'a> {
    fn new(real: &'a (dyn OperationTerminalStore + Sync)) -> Self {
        Self {
            real,
            loss: ReplyLoss::None,
            complete_calls: AtomicUsize::new(0),
            finish_calls: AtomicUsize::new(0),
            interrupt_calls: AtomicUsize::new(0),
            read_calls: AtomicUsize::new(0),
            snapshot_calls: AtomicUsize::new(0),
            refused: AtomicUsize::new(0),
            nonapplied: AtomicUsize::new(0),
            committed_without_view: AtomicUsize::new(0),
            active_calls: AtomicUsize::new(0),
            attempt_hook: None,
            read_hook: None,
        }
    }
    fn observe_attempt(&self, reply: AttemptReply) -> AttemptReply {
        if matches!(
            reply.disposition,
            AttemptDisposition::NotAdmitted(NonAdmission::QueueFull)
        ) {
            self.refused.fetch_add(1, Ordering::Release);
        }
        if matches!(
            reply.disposition,
            AttemptDisposition::Committed { view: None }
        ) {
            self.committed_without_view.fetch_add(1, Ordering::Release);
        }
        if let Some(hook) = self.attempt_hook {
            hook(&reply);
        }
        reply
    }
}

struct Calling<'a>(&'a AtomicUsize);
impl Drop for Calling<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

impl TracedStore<'_> {
    fn calling(&self) -> Calling<'_> {
        self.active_calls.fetch_add(1, Ordering::AcqRel);
        Calling(&self.active_calls)
    }
}

impl OperationTerminalStore for TracedStore<'_> {
    fn snapshot(&self, id: &str) -> SnapshotReply {
        let _calling = self.calling();
        self.snapshot_calls.fetch_add(1, Ordering::Release);
        self.real.snapshot(id)
    }
    fn complete_item(&self, command: &CompleteItemCommand) -> AttemptReply {
        let _calling = self.calling();
        let attempt = self.complete_calls.fetch_add(1, Ordering::AcqRel);
        let reply = self.observe_attempt(self.real.complete_item(command));
        if attempt == 0 && matches!(reply.disposition, AttemptDisposition::Committed { .. }) {
            match self.loss {
                ReplyLoss::None => {}
                ReplyLoss::PanicAfterComplete => {
                    injected_unwind("TEMP response lost after REAL SQL commit")
                }
                ReplyLoss::UnknownAfterComplete => {
                    return AttemptReply {
                        witness: reply.witness,
                        disposition: AttemptDisposition::Unknown(
                            UnknownOutcome::AcknowledgementLost,
                        ),
                        diagnostic: Some(StoreDiagnostic::new(io::Error::other(
                            "TEMP acknowledgement loss after REAL commit",
                        ))),
                    };
                }
            }
        }
        reply
    }
    fn finish(&self, command: &FinishCommand) -> AttemptReply {
        let _calling = self.calling();
        self.finish_calls.fetch_add(1, Ordering::Release);
        self.observe_attempt(self.real.finish(command))
    }
    fn interrupt(&self, command: &InterruptCommand) -> AttemptReply {
        let _calling = self.calling();
        self.interrupt_calls.fetch_add(1, Ordering::Release);
        self.observe_attempt(self.real.interrupt(command))
    }
    fn readback(&self, witness: &CommandWitness) -> ReadbackReply {
        let _calling = self.calling();
        let reply = self.real.readback(witness);
        if matches!(
            reply.disposition,
            ReadbackDisposition::NotAdmitted(NonAdmission::QueueFull)
        ) {
            self.refused.fetch_add(1, Ordering::Release);
        }
        if matches!(reply.disposition, ReadbackDisposition::ProvenNotApplied(_)) {
            self.nonapplied.fetch_add(1, Ordering::Release);
        }
        if let Some(hook) = self.read_hook {
            hook(&reply);
        }
        self.read_calls.fetch_add(1, Ordering::Release);
        reply
    }
}

#[derive(Default)]
struct Gate {
    opened: Mutex<bool>,
    wake: Condvar,
    entered: AtomicUsize,
}
impl Gate {
    fn wait(&self) {
        self.entered.fetch_add(1, Ordering::Release);
        let open = lock(&self.opened);
        drop(
            self.wake
                .wait_while(open, |open| !*open)
                .unwrap_or_else(|error| error.into_inner()),
        );
    }
    fn release(&self) {
        *lock(&self.opened) = true;
        self.wake.notify_all();
    }
}
struct Release<'a>(&'a Gate);
impl Drop for Release<'_> {
    fn drop(&mut self) {
        self.0.release();
    }
}
struct EndRun<'a>(&'a RunControl);
impl Drop for EndRun<'_> {
    fn drop(&mut self) {
        self.0.return_unresolved();
    }
}

fn wait_for(mut predicate: impl FnMut() -> bool) -> TestResult {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !predicate() {
        if Instant::now() >= deadline {
            return Err(io::Error::other("TEMP controlled observation deadline").into());
        }
        thread::yield_now();
    }
    Ok(())
}
fn join_run(handle: thread::ScopedJoinHandle<'_, RunOutcome>) -> TestResult<RunOutcome> {
    handle
        .join()
        .map_err(|_| io::Error::other("TEMP host thread panicked").into())
}

#[test]
fn scoped_sql_config_w1_w2_w8_two_reports_keep_full_raw_signed_commands() -> TestResult {
    for workers in [1, 2, 8] {
        let (_roots, library) = owned_library()?;
        let mut db = Database::open_isolated(&library)?;
        let rows = vec![(raw_id(0), 0), (raw_id(1), -7)];
        seed(&db, &rows)?;
        {
            let real = TerminalRepository::new(&db, GENERATION);
            let requests: Vec<_> = rows
                .iter()
                .map(|(id, _)| request(&real, id))
                .collect::<TestResult<_>>()?;
            let expected: Vec<_> = requests
                .iter()
                .map(|request| request.complete.clone())
                .collect();
            let input = Mutex::new(VecDeque::from(requests));
            let source = || lock(&input).pop_front();
            let delivered = Mutex::new(Vec::new());
            let deliver = |owner: &Retained| record_delivery(&delivered, owner);
            let store = TracedStore::new(&real);
            let config =
                Configuration::new(workers).ok_or_else(|| io::Error::other("worker config"))?;
            let outcome = scoped::run(
                config,
                &Callbacks {
                    store: &store,
                    source: &source,
                    work: &completed,
                    deliver: &deliver,
                },
                &RunControl::default(),
            );
            assert!(outcome.positively_settled());
            assert!(outcome.measurements.joined);
            assert_eq!(outcome.measurements.settled, 2);
            assert!(outcome.measurements.max_retained <= workers);
            assert!(outcome.measurements.max_callback_owners <= workers + 1);
            assert!(outcome.measurements.max_dispatched_handles <= 3 * workers + 1);
            assert!(outcome.measurements.max_command_bytes > 0);
            assert!(outcome.measurements.max_store_field_copy_budget_bytes > 0);
            assert_eq!(store.complete_calls.load(Ordering::Acquire), 2);
            assert_eq!(store.finish_calls.load(Ordering::Acquire), 2);
            assert_eq!(store.snapshot_calls.load(Ordering::Acquire), 0);
            let seen = lock(&delivered);
            for command in &expected {
                let value = seen
                    .iter()
                    .find(|value| value.execution.as_ref() == Some(command))
                    .ok_or_else(|| io::Error::other("full execution witness lost"))?;
                assert!(value.complete_acknowledged && !value.unknown);
                assert_eq!(value.state, Some(OperationState::Succeeded));
                assert_eq!(value.work_cause.as_deref(), Some("independent work cause"));
                let saved = snapshot(&real, &command.operation.identity.id)?;
                assert_eq!(
                    saved.operation.counts,
                    Counts {
                        completed: 1,
                        total: 1
                    }
                );
                assert!(saved.operation.identity.typed_id().is_err());
                assert_eq!(saved.items[0].identity.id, command.item.id);
                assert_eq!(saved.items[0].identity.story_id, command.stored_story_id());
                assert_eq!(saved.items[0].identity.resolved_story_id, Some(41));
                assert_eq!(saved.items[0].immutable_scope(), command.item);
                assert_eq!(saved.items[0].progress.completed, 1);
            }
        }
        db.close()?;
        library.lease().revalidate()?;
    }
    Ok(())
}

#[test]
fn scoped_sql_w8_refused_owners_finish_report_and_drain_before_close() -> TestResult {
    let (_roots, library) = owned_library()?;
    let mut db = Database::open_isolated(&library)?;
    let rows: Vec<_> = (0..8)
        .map(|index| (raw_id(index), 100 + index as i64))
        .collect();
    seed(&db, &rows)?;
    {
        let real = TerminalRepository::new(&db, GENERATION);
        let requests: Vec<_> = rows
            .iter()
            .map(|(id, _)| request(&real, id))
            .collect::<TestResult<_>>()?;
        let expected: Vec<_> = requests
            .iter()
            .map(|request| request.complete.clone())
            .collect();
        let input = Mutex::new(VecDeque::from(requests));
        let source = || lock(&input).pop_front();
        let mut held = Vec::new();
        for _ in 0..4 {
            held.push(db.repair_transaction(|_| Ok(()))?);
        }
        let store = TracedStore::new(&real);
        let delivered = Mutex::new(Vec::new());
        let deliver = |owner: &Retained| record_delivery(&delivered, owner);
        let control = RunControl::default();
        let mut outcome = thread::scope(|scope| -> TestResult<_> {
            let _end = EndRun(&control);
            let runner = scope.spawn(|| {
                scoped::run(
                    Configuration::new(8).unwrap_or_default(),
                    &Callbacks {
                        store: &store,
                        source: &source,
                        work: &completed,
                        deliver: &deliver,
                    },
                    &control,
                )
            });
            wait_for(|| store.refused.load(Ordering::Acquire) == 8)?;
            assert!(!runner.is_finished());
            assert!(lock(&delivered).is_empty());
            control.stop_intake();
            assert!(!control.work_cancelled());
            control.return_unresolved();
            join_run(runner)
        })?;
        assert!(outcome.measurements.joined);
        assert!(!outcome.positively_settled());
        assert_eq!(outcome.unresolved.len(), 8);
        assert_eq!(outcome.measurements.max_retained, 8);
        assert!(outcome.measurements.max_dispatched_handles <= 25);
        assert_eq!(store.complete_calls.load(Ordering::Acquire), 8);
        for command in &expected {
            let owner = outcome
                .unresolved
                .iter()
                .find(|owner| owner.request.complete == *command)
                .ok_or_else(|| io::Error::other("QueueFull lost original owner"))?;
            assert_eq!(owner.execution.as_ref(), Some(command));
            assert!(!owner.complete_acknowledged && !owner.unknown);
            assert_eq!(
                owner
                    .work_error
                    .as_ref()
                    .map(ToString::to_string)
                    .as_deref(),
                Some("independent work cause")
            );
            assert_eq!(
                snapshot(&real, &command.operation.identity.id)?
                    .operation
                    .counts
                    .completed,
                0
            );
        }
        // Return from H is explicitly unresolved. Retain all eight move-only
        // owners under a coordinator claim guard; do not close on joined=true.
        assert!(outcome.unstarted.is_empty());
        assert!(outcome.source_failure_before_db.is_none());
        assert!(outcome.host_failure.is_none());
        let exclusion = Mutex::new(());
        let exclusive = lock(&exclusion);
        let accepted = Mutex::new(Vec::new());
        // Release the four real unacknowledged results before required repair.
        for pending in held {
            committed(pending.wait()?)?;
        }
        for owner in &mut outcome.unresolved {
            resolve_returned_owner(&real, owner, &exclusive, &accepted)?;
        }
        assert_eq!(lock(&accepted).len(), 8);
        assert!(lock(&delivered).is_empty()); // H did not pretend to settle.
        for (id, _) in &rows {
            let saved = snapshot(&real, id)?;
            assert_eq!(saved.operation.state, OperationState::Succeeded);
            assert_eq!(
                saved.operation.counts,
                Counts {
                    completed: 1,
                    total: 1
                }
            );
        }
        // All planned commands and reports are now acknowledged. Only here may
        // returned owners, coordinator exclusion and borrowed repository drop.
        drop(outcome);
        drop(exclusive);
    }
    db.close()?;
    library.lease().revalidate()?;
    Ok(())
}

#[test]
fn scoped_sql_real_commit_error_unknown_survives_full_repair_then_fresh_retry() -> TestResult {
    let (_roots, library) = owned_library()?;
    let mut db = Database::open_isolated(&library)?;
    let id = raw_id(0);
    seed(&db, &[(id.clone(), -7)])?;
    committed(db.repair_transaction(|tx| {
        tx.execute_batch("CREATE TABLE scoped_deferred(value INTEGER REFERENCES stories(id) DEFERRABLE INITIALLY DEFERRED);
            CREATE TRIGGER scoped_fail_commit AFTER UPDATE OF status ON file_operation_items BEGIN INSERT INTO scoped_deferred VALUES(999999); END;")?;
        Ok(())
    })?.wait()?)?;
    {
        let real = TerminalRepository::new(&db, GENERATION);
        let before = snapshot(&real, &id)?;
        let input = Mutex::new(VecDeque::from([request(&real, &id)?]));
        let source = || lock(&input).pop_front();
        let unknown_gate = Gate::default();
        let proof_gate = Gate::default();
        let attempt_hook = |reply: &AttemptReply| {
            if matches!(
                reply.disposition,
                AttemptDisposition::Unknown(UnknownOutcome::AcknowledgementLost)
            ) {
                unknown_gate.wait();
            }
        };
        let read_hook = |reply: &ReadbackReply| {
            if matches!(reply.disposition, ReadbackDisposition::ProvenNotApplied(_)) {
                proof_gate.wait();
            }
        };
        let mut store = TracedStore::new(&real);
        store.attempt_hook = Some(&attempt_hook);
        store.read_hook = Some(&read_hook);
        let delivered = Mutex::new(Vec::new());
        let deliver = |owner: &Retained| record_delivery(&delivered, owner);
        let control = RunControl::default();
        let outcome = thread::scope(|scope| -> TestResult<_> {
            let _end = EndRun(&control);
            let _release_unknown = Release(&unknown_gate);
            let _release_proof = Release(&proof_gate);
            let runner = scope.spawn(|| {
                scoped::run(
                    Configuration::new(1).unwrap_or_default(),
                    &Callbacks {
                        store: &store,
                        source: &source,
                        work: &completed,
                        deliver: &deliver,
                    },
                    &control,
                )
            });
            wait_for(|| unknown_gate.entered.load(Ordering::Acquire) == 1)?;
            let mut held = Vec::new();
            for _ in 0..4 {
                held.push(db.repair_transaction(|_| Ok(()))?);
            }
            unknown_gate.release();
            wait_for(|| store.read_calls.load(Ordering::Acquire) >= 1)?;
            assert_eq!(store.refused.load(Ordering::Acquire), 1);
            assert_eq!(store.complete_calls.load(Ordering::Acquire), 1);
            assert!(lock(&delivered).is_empty());
            for pending in held {
                committed(pending.wait()?)?;
            }
            let mut tick = 0;
            wait_for(|| {
                if proof_gate.entered.load(Ordering::Acquire) == 1 {
                    return true;
                }
                // No clock derives from wall time or catches up a running DB
                // call. Advance only after the observed call has returned.
                if store.active_calls.load(Ordering::Acquire) == 0 {
                    tick += 1000;
                    let _ = control.advance(tick);
                }
                false
            })?;
            assert_eq!(snapshot(&real, &id)?, before);
            committed(
                db.repair_transaction(|tx| {
                    tx.execute_batch("DROP TRIGGER scoped_fail_commit")?;
                    Ok(())
                })?
                .wait()?,
            )?;
            proof_gate.release();
            wait_for(|| {
                if runner.is_finished() {
                    return true;
                }
                if store.active_calls.load(Ordering::Acquire) == 0 {
                    tick += 10;
                    let _ = control.advance(tick);
                }
                false
            })?;
            join_run(runner)
        })?;
        assert!(outcome.positively_settled() && outcome.measurements.joined);
        assert_eq!(store.complete_calls.load(Ordering::Acquire), 2);
        assert_eq!(store.finish_calls.load(Ordering::Acquire), 1);
        assert_eq!(store.interrupt_calls.load(Ordering::Acquire), 0);
        assert_eq!(snapshot(&real, &id)?.operation.counts.completed, 1);
        assert_eq!(lock(&delivered).len(), 1);
    }
    db.close()?;
    library.lease().revalidate()?;
    Ok(())
}

#[test]
fn scoped_sql_real_lost_ack_and_postcommit_store_unwind_never_replay_work() -> TestResult {
    for loss in [
        ReplyLoss::UnknownAfterComplete,
        ReplyLoss::PanicAfterComplete,
    ] {
        let (_roots, library) = owned_library()?;
        let mut db = Database::open_isolated(&library)?;
        let id = raw_id(0);
        seed(&db, &[(id.clone(), 0)])?;
        {
            let real = TerminalRepository::new(&db, GENERATION);
            let supplied = request(&real, &id)?;
            let expected = supplied.complete.clone();
            let input = Mutex::new(VecDeque::from([supplied]));
            let source = || lock(&input).pop_front();
            let executions = AtomicUsize::new(0);
            let work = |request: &Request, control: &RunControl| {
                executions.fetch_add(1, Ordering::Release);
                completed(request, control)
            };
            let delivered = Mutex::new(Vec::new());
            let deliver = |owner: &Retained| record_delivery(&delivered, owner);
            let mut store = TracedStore::new(&real);
            store.loss = loss;
            let outcome = scoped::run(
                Configuration::new(1).unwrap_or_default(),
                &Callbacks {
                    store: &store,
                    source: &source,
                    work: &work,
                    deliver: &deliver,
                },
                &RunControl::default(),
            );
            assert!(outcome.positively_settled() && outcome.measurements.joined);
            assert_eq!(executions.load(Ordering::Acquire), 1);
            assert_eq!(store.complete_calls.load(Ordering::Acquire), 1);
            assert_eq!(store.finish_calls.load(Ordering::Acquire), 1);
            assert_eq!(store.read_calls.load(Ordering::Acquire), 2);
            assert_eq!(snapshot(&real, &id)?.operation.counts.completed, 1);
            let seen = lock(&delivered);
            assert_eq!(seen.len(), 1);
            assert_eq!(seen[0].execution.as_ref(), Some(&expected));
            assert!(seen[0].complete_acknowledged && !seen[0].unknown);
            assert_eq!(seen[0].state, Some(OperationState::Succeeded));
        }
        db.close()?;
        library.lease().revalidate()?;
    }
    Ok(())
}

#[test]
fn scoped_sql_sticky_committed_none_drains_after_real_readback_queue_refusal() -> TestResult {
    let (_roots, library) = owned_library()?;
    let mut db = Database::open_isolated(&library)?;
    let id = raw_id(0);
    seed(&db, &[(id.clone(), -7)])?;
    {
        let real = TerminalRepository::new(&db, GENERATION);
        let supplied = request(&real, &id)?;
        let expected = supplied.complete.clone();
        let input = Mutex::new(VecDeque::from([supplied]));
        let source = || lock(&input).pop_front();
        let committed_gate = Gate::default();
        let attempt_hook = |reply: &AttemptReply| {
            if matches!(reply.witness, CommandWitness::CompleteItem(_))
                && matches!(
                    reply.disposition,
                    AttemptDisposition::Committed { view: None }
                )
            {
                committed_gate.wait();
            }
        };
        let mut store = TracedStore::new(&real);
        store.attempt_hook = Some(&attempt_hook);
        let delivered = Mutex::new(Vec::new());
        let deliver = |owner: &Retained| record_delivery(&delivered, owner);
        let control = RunControl::default();
        db.stop_ordinary()?;
        let outcome = thread::scope(|scope| -> TestResult<_> {
            let _end = EndRun(&control);
            let _release = Release(&committed_gate);
            let runner = scope.spawn(|| {
                scoped::run(
                    Configuration::new(1).unwrap_or_default(),
                    &Callbacks {
                        store: &store,
                        source: &source,
                        work: &completed,
                        deliver: &deliver,
                    },
                    &control,
                )
            });
            wait_for(|| committed_gate.entered.load(Ordering::Acquire) == 1)?;
            let mut held = Vec::new();
            for _ in 0..4 {
                held.push(db.repair_transaction(|_| Ok(()))?);
            }
            committed_gate.release();
            wait_for(|| store.read_calls.load(Ordering::Acquire) >= 1)?;
            assert_eq!(store.refused.load(Ordering::Acquire), 1);
            assert_eq!(store.committed_without_view.load(Ordering::Acquire), 1);
            assert!(!runner.is_finished());
            control.stop_intake();
            control.cancel_work();
            assert!(lock(&delivered).is_empty());
            for pending in held {
                committed(pending.wait()?)?;
            }
            let mut tick = 0;
            wait_for(|| {
                if runner.is_finished() {
                    return true;
                }
                if store.active_calls.load(Ordering::Acquire) == 0 {
                    tick += 1000;
                    let _ = control.advance(tick);
                }
                false
            })?;
            join_run(runner)
        })?;
        assert!(outcome.positively_settled() && outcome.measurements.joined);
        assert_eq!(store.complete_calls.load(Ordering::Acquire), 1);
        assert_eq!(store.finish_calls.load(Ordering::Acquire), 1);
        assert_eq!(store.committed_without_view.load(Ordering::Acquire), 2);
        let seen = lock(&delivered);
        assert_eq!(seen[0].execution.as_ref(), Some(&expected));
        assert!(seen[0].complete_acknowledged && !seen[0].unknown);
        assert_eq!(seen[0].state, Some(OperationState::Succeeded));
        // Ordinary intake remains stopped: final truth comes from the real
        // reserved writer barrier, never a fabricated pooled snapshot.
        let raw = id.clone();
        let (count, state): (i64, String) = db
            .barrier_read(move |tx| {
                Ok(tx.query_row(
                    "SELECT completed_items,status FROM file_operations WHERE id=?",
                    [raw],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )?)
            })?
            .wait()?;
        assert_eq!((count, state.as_str()), (1, "succeeded"));
    }
    db.close()?;
    library.lease().revalidate()?;
    Ok(())
}

#[test]
fn scoped_sql_source_and_work_panics_keep_predb_distinction() -> TestResult {
    for source_panics in [true, false] {
        let (_roots, library) = owned_library()?;
        let mut db = Database::open_isolated(&library)?;
        let id = raw_id(0);
        seed(&db, &[(id.clone(), 0)])?;
        {
            let real = TerminalRepository::new(&db, GENERATION);
            let before = snapshot(&real, &id)?;
            let input = Mutex::new(VecDeque::from([request(&real, &id)?]));
            let source = || {
                if source_panics {
                    injected_unwind("TEMP source failure BEFORE DB");
                }
                lock(&input).pop_front()
            };
            let work = |_: &Request, _: &RunControl| -> WorkReply {
                injected_unwind("TEMP work failure BEFORE DB")
            };
            let delivered = Mutex::new(Vec::new());
            let deliver = |owner: &Retained| record_delivery(&delivered, owner);
            let store = TracedStore::new(&real);
            let outcome = scoped::run(
                Configuration::default(),
                &Callbacks {
                    store: &store,
                    source: &source,
                    work: &work,
                    deliver: &deliver,
                },
                &RunControl::default(),
            );
            assert!(outcome.measurements.joined);
            assert_eq!(store.complete_calls.load(Ordering::Acquire), 0);
            assert_eq!(store.finish_calls.load(Ordering::Acquire), 0);
            if source_panics {
                assert!(outcome.source_failure_before_db.is_some());
                assert!(!outcome.positively_settled());
                assert_eq!(store.interrupt_calls.load(Ordering::Acquire), 0);
                assert_eq!(snapshot(&real, &id)?, before);
                assert!(lock(&delivered).is_empty());
            } else {
                assert!(outcome.positively_settled());
                assert_eq!(store.interrupt_calls.load(Ordering::Acquire), 1);
                let seen = lock(&delivered);
                assert!(seen[0].work_panicked);
                assert!(!seen[0].unknown && !seen[0].complete_acknowledged);
                assert!(seen[0].execution.is_none());
                assert!(matches!(seen[0].witness, CommandWitness::Interrupt(_)));
                let saved = snapshot(&real, &id)?;
                assert_eq!(saved.operation.state, OperationState::Interrupted);
                assert_eq!(saved.items[0].progress, before.items[0].progress);
                assert_eq!(saved.items[0].state, ItemState::Cancelled);
            }
        }
        // In the source-panic branch the deliberately unadmitted input remains
        // fixture-owned. This is negative-fixture teardown, NOT a successful
        // operation/drain-before-close witness. The work-panic branch did execute
        // and report the supplied real Interrupt above.
        db.close()?;
        library.lease().revalidate()?;
    }
    Ok(())
}

#[test]
fn scoped_sql_stop_cancel_and_unresolved_wait_join_blocked_work_before_close() -> TestResult {
    let (_roots, library) = owned_library()?;
    let mut db = Database::open_isolated(&library)?;
    let id = raw_id(0);
    seed(&db, &[(id.clone(), -7)])?;
    {
        let real = TerminalRepository::new(&db, GENERATION);
        let supplied = request(&real, &id)?;
        let expected = supplied.complete.clone();
        let input = Mutex::new(VecDeque::from([supplied]));
        let source = || lock(&input).pop_front();
        let gate = Gate::default();
        let work = |request: &Request, control: &RunControl| {
            gate.wait();
            assert!(control.work_cancelled());
            completed(request, control)
        };
        let store = TracedStore::new(&real);
        let delivered = Mutex::new(Vec::new());
        let deliver = |owner: &Retained| record_delivery(&delivered, owner);
        let control = RunControl::default();
        let mut outcome = thread::scope(|scope| -> TestResult<_> {
            let _end = EndRun(&control);
            let _release = Release(&gate);
            let runner = scope.spawn(|| {
                scoped::run(
                    Configuration::new(1).unwrap_or_default(),
                    &Callbacks {
                        store: &store,
                        source: &source,
                        work: &work,
                        deliver: &deliver,
                    },
                    &control,
                )
            });
            wait_for(|| gate.entered.load(Ordering::Acquire) == 1)?;
            control.stop_intake();
            assert!(!control.work_cancelled());
            control.cancel_work();
            control.return_unresolved();
            assert!(!runner.is_finished());
            assert_eq!(store.complete_calls.load(Ordering::Acquire), 0);
            gate.release();
            join_run(runner)
        })?;
        assert!(outcome.measurements.joined);
        assert!(!outcome.positively_settled());
        assert_eq!(outcome.unresolved.len(), 1);
        assert_eq!(outcome.unresolved[0].execution.as_ref(), Some(&expected));
        assert_eq!(outcome.unresolved[0].request.complete, expected);
        assert!(!outcome.unresolved[0].unknown);
        assert!(lock(&delivered).is_empty());
        assert_eq!(store.complete_calls.load(Ordering::Acquire), 0);
        assert!(outcome.unstarted.is_empty());
        assert!(outcome.source_failure_before_db.is_none());
        assert!(outcome.host_failure.is_none());
        let exclusion = Mutex::new(());
        let exclusive = lock(&exclusion);
        let accepted = Mutex::new(Vec::new());
        resolve_returned_owner(&real, &mut outcome.unresolved[0], &exclusive, &accepted)?;
        assert_eq!(gate.entered.load(Ordering::Acquire), 1); // No work replay.
        assert_eq!(store.complete_calls.load(Ordering::Acquire), 0); // No H replay.
        assert_eq!(lock(&accepted).len(), 1);
        let saved = snapshot(&real, &id)?;
        assert_eq!(saved.operation.state, OperationState::Succeeded);
        assert_eq!(
            saved.operation.counts,
            Counts {
                completed: 1,
                total: 1
            }
        );
        assert_eq!(
            saved.operation.identity.finished_at.as_deref(),
            Some("RAW planned finish # %2F")
        );
        drop(outcome);
        drop(exclusive);
    }
    db.close()?;
    library.lease().revalidate()?;
    Ok(())
}

/// Requires owner3's original3923063 active-parent/item-terminal fix in the TEMP
/// composition. It deliberately asserts REAL R4 truth, not a fake
/// operation terminal state or an ignored/waived assertion.
#[test]
fn scoped_sql_future_active_parent_terminal_item_preserves_pending_sibling() -> TestResult {
    let (_roots, library) = owned_library()?;
    let mut db = Database::open_isolated(&library)?;
    let id = raw_id(0);
    seed(&db, &[(id.clone(), 0)])?;
    let raw = id.clone();
    committed(
        db.repair_transaction(move |tx| {
            tx.execute(
                "UPDATE file_operations SET total_items=2,total_bytes=20 WHERE id=?",
                [&raw],
            )?;
            tx.execute(
                "INSERT INTO file_operation_items(id,operation_id,source_name,status,total_bytes)
            VALUES(-7,?,'pending sibling','pending',10)",
                [raw],
            )?;
            Ok(())
        })?
        .wait()?,
    )?;
    {
        let real = TerminalRepository::new(&db, GENERATION);
        let mut supplied = request(&real, &id)?;
        // row-ID ordering puts -7 first; target the explicit0 instead.
        let initial = snapshot(&real, &id)?;
        let target = initial
            .items
            .iter()
            .find(|item| item.identity.id == 0)
            .ok_or_else(|| io::Error::other("target missing"))?;
        supplied.complete.item = target.immutable_scope();
        supplied.complete.story_id = 0;
        supplied.finish = None;
        let mut other = supplied.complete.clone();
        other.state = ItemState::Failed;
        other.outcome.code = Some("actual different terminal".into());
        assert!(matches!(
            real.complete_item(&other).disposition,
            AttemptDisposition::Committed { .. }
        ));
        let expected = snapshot(&real, &id)?;
        assert_eq!(expected.operation.state, OperationState::Queued);
        let input = Mutex::new(VecDeque::from([supplied]));
        let source = || lock(&input).pop_front();
        let delivered = Mutex::new(Vec::new());
        let deliver = |owner: &Retained| record_delivery(&delivered, owner);
        let store = TracedStore::new(&real);
        let control = RunControl::default();
        let outcome = thread::scope(|scope| -> TestResult<_> {
            let _end = EndRun(&control);
            let runner = scope.spawn(|| {
                scoped::run(
                    Configuration::new(1).unwrap_or_default(),
                    &Callbacks {
                        store: &store,
                        source: &source,
                        work: &completed,
                        deliver: &deliver,
                    },
                    &control,
                )
            });
            wait_for(|| runner.is_finished())?;
            join_run(runner)
        })?;
        assert!(outcome.positively_settled() && outcome.measurements.joined);
        assert_eq!(store.complete_calls.load(Ordering::Acquire), 1);
        assert_eq!(store.interrupt_calls.load(Ordering::Acquire), 0);
        assert_eq!(snapshot(&real, &id)?, expected);
        assert_eq!(lock(&delivered).len(), 1);
    }
    // This case deliberately observes one terminal target under an ACTIVE
    // parent with an externally seeded pending sibling. Its close is negative
    // fixture teardown, not a whole-operation shutdown/drain qualification.
    db.close()?;
    library.lease().revalidate()?;
    Ok(())
}

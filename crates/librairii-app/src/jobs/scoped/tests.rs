//! Controlled assertions only; real SQL composition belongs to the TEMP host.
#![allow(clippy::unwrap_used, clippy::panic)]
use super::{driver::Driver, *};
use crate::ports::operations::*;
use librairii_core::{OperationKind, OperationState, operations::*};
use std::{
    collections::VecDeque,
    sync::{
        Condvar, Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

fn request(id: i64, finish: bool) -> Request {
    let raw = format!("{:036}", id + 100);
    let operation = OperationScope {
        identity: HistoryKey {
            id: raw.clone(),
            created_at: "RAW time +02".into(),
        },
        kind: OperationKind::Export,
        export_scope: StoredExportScope {
            discriminator: Some("shelves".into()),
            group_ids: Some("[3, 1]".into()),
            group_names: Some("[] ".into()),
        },
        destination: Some(String::new()),
        total_items: 1,
        total_bytes: 7,
    };
    let complete = CompleteItemCommand {
        generation: 42,
        operation: operation.clone(),
        item: ItemScope {
            operation_id: raw,
            id,
            resolved_story_id: Some(19),
            story_uuid: Some("UPPER raw uuid".into()),
            story_title: Some(String::new()),
            source_name: "Été #%2F".into(),
            output_name: None,
            archive_relative_path: None,
            archive_sha256: None,
            export_skip_code: None,
            total_bytes: 7,
        },
        story_id: 0,
        state: ItemState::Succeeded,
        completed_bytes: 3,
        outcome: RawOutcome {
            code: Some(String::new()),
            message: None,
        },
    };
    Request {
        finish: finish.then(|| FinishCommand {
            generation: 42,
            operation: operation.clone(),
            state: OperationState::Succeeded,
            outcome: RawOutcome {
                code: None,
                message: Some(String::new()),
            },
            finished_at: "RAW planned finish".into(),
        }),
        interrupt: InterruptCommand {
            generation: 42,
            operation,
            code: "work_failed".into(),
            message: " raw reason ".into(),
            finished_at: "RAW interruption".into(),
        },
        complete,
    }
}
fn initial(request: &Request) -> TerminalView {
    let command = &request.complete;
    TerminalView {
        operation: OperationHeader {
            identity: StoredOperationIdentity {
                id: command.operation.identity.id.clone(),
                created_at: command.operation.identity.created_at.clone(),
                started_at: Some(String::new()),
                finished_at: None,
            },
            kind: command.operation.kind,
            state: OperationState::Running,
            export_scope: command.operation.export_scope.clone(),
            destination: command.operation.destination.clone(),
            counts: Counts {
                completed: 0,
                total: 1,
            },
            total_bytes: 7,
            cancel_requested: false,
            outcome: RawOutcome {
                code: None,
                message: Some(String::new()),
            },
        },
        item: Some(OperationItem {
            operation_id: command.item.operation_id.clone(),
            identity: StoredItemIdentity {
                id: command.item.id,
                story_id: Some(8),
                resolved_story_id: command.item.resolved_story_id,
                story_uuid: command.item.story_uuid.clone(),
            },
            story_title: command.item.story_title.clone(),
            source_name: command.item.source_name.clone(),
            output_name: command.item.output_name.clone(),
            archive_relative_path: None,
            archive_sha256: None,
            export_skip_code: None,
            state: ItemState::Running,
            progress: Progress {
                completed: 6,
                total: 7,
            },
            outcome: RawOutcome {
                code: None,
                message: None,
            },
        }),
        siblings: SiblingCounts {
            items: 1,
            terminal: 0,
        },
    }
}
#[derive(Clone, Copy)]
enum Behavior {
    Pass,
    Refuse,
    Rollback,
    Unknown,
    PanicAfterCommit,
    PanicBefore,
    WrongWitness,
    Conflict,
}
struct Store<'a> {
    views: Mutex<Vec<TerminalView>>,
    writes: AtomicUsize,
    reads: AtomicUsize,
    attempt: &'a (dyn Fn(&CommandWitness) -> Behavior + Sync),
    read: &'a (dyn Fn(&CommandWitness) -> Behavior + Sync),
}
fn pass(_: &CommandWitness) -> Behavior {
    Behavior::Pass
}
fn scope(witness: &CommandWitness) -> &OperationScope {
    match witness {
        CommandWitness::CompleteItem(c) => &c.operation,
        CommandWitness::Finish(c) => &c.operation,
        CommandWitness::Interrupt(c) => &c.operation,
    }
}
impl Store<'_> {
    fn new(requests: &[Request]) -> Self {
        Self {
            views: Mutex::new(requests.iter().map(initial).collect()),
            writes: AtomicUsize::new(0),
            reads: AtomicUsize::new(0),
            attempt: &pass,
            read: &pass,
        }
    }
    fn attempt(&self, witness: CommandWitness) -> AttemptReply {
        self.writes.fetch_add(1, Ordering::AcqRel);
        let behavior = (self.attempt)(&witness);
        let disposition = match behavior {
            Behavior::Refuse => AttemptDisposition::NotAdmitted(NonAdmission::QueueFull),
            Behavior::Rollback => AttemptDisposition::ProvenNotCommitted,
            Behavior::Unknown => AttemptDisposition::Unknown(UnknownOutcome::AcknowledgementLost),
            Behavior::PanicBefore => {
                std::panic::panic_any("controlled store panic before admission")
            }
            Behavior::WrongWitness => {
                return AttemptReply {
                    witness: CommandWitness::CompleteItem(Box::new(request(999, false).complete)),
                    disposition: AttemptDisposition::Committed { view: None },
                    diagnostic: None,
                };
            }
            _ => {
                {
                    let mut views = self.views.lock().unwrap();
                    let view = views
                        .iter_mut()
                        .find(|v| v.operation.identity.id == scope(&witness).identity.id)
                        .unwrap();
                    match &witness {
                        CommandWitness::CompleteItem(command) => {
                            let item = view.item.as_mut().unwrap();
                            assert!(
                                !item.state.is_terminal(),
                                "acknowledged completion was replayed"
                            );
                            item.state = command.state;
                            item.identity.story_id = command.stored_story_id();
                            item.progress.completed = command.completed_bytes;
                            item.outcome = command.stored_outcome();
                            view.operation.counts.completed += 1;
                            view.siblings.terminal += 1;
                        }
                        CommandWitness::Finish(command) => {
                            view.operation.state = command.state;
                            view.operation.identity.finished_at = Some(command.finished_at.clone());
                            view.operation.outcome = command.stored_outcome();
                        }
                        CommandWitness::Interrupt(command) => {
                            view.operation.state = OperationState::Interrupted;
                            view.operation.identity.finished_at = Some(command.finished_at.clone());
                            view.operation.outcome = RawOutcome {
                                code: Some(command.code.clone()),
                                message: Some(command.message.clone()),
                            };
                            view.operation.counts.completed = view.operation.counts.total;
                            view.siblings.terminal = view.siblings.items;
                            if let Some(item) = &mut view.item
                                && !item.state.is_terminal()
                            {
                                item.state = ItemState::Cancelled;
                            }
                        }
                    }
                }
                if matches!(behavior, Behavior::PanicAfterCommit) {
                    std::panic::panic_any("controlled response lost after commit");
                }
                AttemptDisposition::Committed { view: None }
            }
        };
        AttemptReply {
            witness,
            disposition,
            diagnostic: Some(StoreDiagnostic::new(std::io::Error::other(
                "independent view cause",
            ))),
        }
    }
}
impl OperationTerminalStore for Store<'_> {
    fn snapshot(&self, _: &str) -> SnapshotReply {
        panic!("driver must use exact writer readback")
    }
    fn complete_item(&self, command: &CompleteItemCommand) -> AttemptReply {
        self.attempt(CommandWitness::CompleteItem(Box::new(command.clone())))
    }
    fn finish(&self, command: &FinishCommand) -> AttemptReply {
        self.attempt(CommandWitness::Finish(Box::new(command.clone())))
    }
    fn interrupt(&self, command: &InterruptCommand) -> AttemptReply {
        self.attempt(CommandWitness::Interrupt(Box::new(command.clone())))
    }
    fn readback(&self, witness: &CommandWitness) -> ReadbackReply {
        self.reads.fetch_add(1, Ordering::AcqRel);
        let behavior = (self.read)(witness);
        let disposition = match behavior {
            Behavior::Conflict => {
                let mut view = self
                    .views
                    .lock()
                    .unwrap()
                    .iter()
                    .find(|v| v.operation.identity.id == scope(witness).identity.id)
                    .unwrap()
                    .clone();
                view.operation.state = OperationState::Cancelled;
                view.operation.identity.finished_at = Some("different RAW terminal time".into());
                ReadbackDisposition::TerminalConflict(Box::new(view))
            }
            Behavior::Refuse => ReadbackDisposition::NotAdmitted(NonAdmission::QueueFull),
            Behavior::Unknown => ReadbackDisposition::Unknown(UnknownOutcome::Unavailable),
            Behavior::PanicBefore => std::panic::panic_any("controlled readback panic"),
            Behavior::WrongWitness => {
                return ReadbackReply {
                    witness: CommandWitness::CompleteItem(Box::new(request(999, false).complete)),
                    disposition: ReadbackDisposition::Unknown(UnknownOutcome::Contradictory),
                    diagnostic: None,
                };
            }
            _ => {
                let views = self.views.lock().unwrap();
                let view = Box::new(
                    views
                        .iter()
                        .find(|v| v.operation.identity.id == scope(witness).identity.id)
                        .unwrap()
                        .clone(),
                );
                let applied = match witness {
                    CommandWitness::CompleteItem(_) => {
                        view.item.as_ref().unwrap().state.is_terminal()
                    }
                    CommandWitness::Finish(_) | CommandWitness::Interrupt(_) => {
                        is_terminal(view.operation.state)
                    }
                };
                if applied {
                    ReadbackDisposition::MatchesApplied(view)
                } else {
                    ReadbackDisposition::ProvenNotApplied(view)
                }
            }
        };
        ReadbackReply {
            witness: witness.clone(),
            disposition,
            diagnostic: None,
        }
    }
}
fn completed(request: &Request, _: &RunControl) -> WorkReply {
    WorkReply::completed(request.complete.clone())
}
fn delivered(_: &Retained) -> Delivery {
    Delivery::Accepted
}
fn wait_for(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !condition() {
        assert!(
            Instant::now() < deadline,
            "controlled barrier did not arrive"
        );
        thread::yield_now();
    }
}
fn pump(control: &RunControl, mut condition: impl FnMut() -> bool) {
    let mut tick = control.observe().0;
    wait_for(|| {
        if condition() {
            return true;
        }
        tick += 1000;
        control.advance(tick).unwrap();
        false
    });
}
struct EndRun<'a>(&'a RunControl);
impl Drop for EndRun<'_> {
    fn drop(&mut self) {
        self.0.return_unresolved();
    }
}

#[test]
fn sh_borrowed_typed_store_keeps_raw_signed_ids_and_sticky_complete_then_finish() {
    let requests = vec![request(0, true), request(-7, true)];
    let store = Store::new(&requests);
    let inputs = Mutex::new(VecDeque::from(requests));
    let source = || inputs.lock().unwrap().pop_front();
    let seen = Mutex::new(Vec::new());
    let deliver = |owner: &Retained| {
        assert!(owner.complete_acknowledged);
        assert!(!owner.unknown);
        assert_eq!(
            owner.view.as_ref().unwrap().operation.state,
            OperationState::Succeeded
        );
        assert!(owner.latest_store_cause.is_some());
        seen.lock().unwrap().push(owner.request.complete.item.id);
        Delivery::Accepted
    };
    let outcome = run(
        Configuration::default(),
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
    assert_eq!(store.writes.load(Ordering::Acquire), 4);
    let mut ids = seen.into_inner().unwrap();
    ids.sort();
    assert_eq!(ids, [-7, 0]);
}

#[test]
fn sh_store_unwind_after_commit_reads_exact_witness_without_replaying_work_or_complete() {
    let requests = vec![request(-7, false)];
    let attempts = AtomicUsize::new(0);
    let attempt = |_: &CommandWitness| {
        if attempts.fetch_add(1, Ordering::AcqRel) == 0 {
            Behavior::PanicAfterCommit
        } else {
            Behavior::Pass
        }
    };
    let mut store = Store::new(&requests);
    store.attempt = &attempt;
    let inputs = Mutex::new(VecDeque::from(requests));
    let source = || inputs.lock().unwrap().pop_front();
    let executions = AtomicUsize::new(0);
    let work = |request: &Request, _: &RunControl| {
        executions.fetch_add(1, Ordering::AcqRel);
        WorkReply::completed(request.complete.clone())
    };
    let outcome = run(
        Configuration::new(1).unwrap(),
        &Callbacks {
            store: &store,
            source: &source,
            work: &work,
            deliver: &delivered,
        },
        &RunControl::default(),
    );
    assert!(outcome.positively_settled());
    assert_eq!(executions.load(Ordering::Acquire), 1);
    assert_eq!(attempts.load(Ordering::Acquire), 1);
    assert_eq!(store.reads.load(Ordering::Acquire), 1);
}

#[test]
fn sh_store_panic_before_admission_is_unknown_until_positive_fresh_nonapplication() {
    let request = request(0, false);
    let attempts = AtomicUsize::new(0);
    let attempt = |_: &CommandWitness| {
        if attempts.fetch_add(1, Ordering::AcqRel) == 0 {
            Behavior::PanicBefore
        } else {
            Behavior::Pass
        }
    };
    let mut store = Store::new(&[request]);
    store.attempt = &attempt;
    let request = super::tests::request(0, false);
    let control = RunControl::default();
    let mut driver = Driver::new(request, 0);
    driver.accept_work(Ok(completed(&driver.owner.request, &control)), 0);
    driver.step(&store, &control);
    assert!(driver.owner.unknown);
    assert_eq!(store.writes.load(Ordering::Acquire), 1);
    driver.step(&store, &control);
    assert!(!driver.owner.unknown);
    assert!(!driver.ready(9));
    control.advance(10).unwrap();
    driver.step(&store, &control);
    assert!(driver.owner.complete_acknowledged);
    driver.step(&store, &control);
    assert!(driver.delivery());
    assert_eq!(store.writes.load(Ordering::Acquire), 2);
}

#[test]
fn sh_typed_execution_fact_and_independent_work_error_survive_reporting_together() {
    let requests = vec![request(0, false)];
    let store = Store::new(&requests);
    let inputs = Mutex::new(VecDeque::from(requests));
    let source = || inputs.lock().unwrap().pop_front();
    let work = |request: &Request, _: &RunControl| WorkReply::Completed {
        command: Box::new(request.complete.clone()),
        diagnostic: Some(StoreDiagnostic::new(std::io::Error::other(
            "independent work cleanup cause",
        ))),
    };
    let deliver = |owner: &Retained| {
        assert!(owner.execution.is_some());
        assert!(owner.complete_acknowledged);
        assert_eq!(
            owner.work_error.as_ref().unwrap().to_string(),
            "independent work cleanup cause"
        );
        Delivery::Accepted
    };
    let outcome = run(
        Configuration::new(1).unwrap(),
        &Callbacks {
            store: &store,
            source: &source,
            work: &work,
            deliver: &deliver,
        },
        &RunControl::default(),
    );
    assert!(outcome.positively_settled());
}

#[test]
fn sh_work_panic_before_db_keeps_owner_and_uses_only_supplied_interruption() {
    let requests = vec![request(0, false)];
    let store = Store::new(&requests);
    let inputs = Mutex::new(VecDeque::from(requests));
    let source = || inputs.lock().unwrap().pop_front();
    let work =
        |_: &Request, _: &RunControl| -> WorkReply { panic!("controlled work panic before DB") };
    let deliver = |owner: &Retained| {
        assert!(owner.work_panicked_before_db);
        assert!(owner.execution.is_none());
        assert!(!owner.complete_acknowledged);
        assert!(!owner.unknown);
        assert!(matches!(owner.witness, CommandWitness::Interrupt(_)));
        Delivery::Accepted
    };
    let outcome = run(
        Configuration::new(1).unwrap(),
        &Callbacks {
            store: &store,
            source: &source,
            work: &work,
            deliver: &deliver,
        },
        &RunControl::default(),
    );
    assert!(outcome.positively_settled());
    assert_eq!(store.writes.load(Ordering::Acquire), 1);
}

#[test]
fn sh_source_panic_is_no_db_admission_and_not_a_fabricated_item_attempt() {
    let store = Store::new(&[]);
    let source = || -> Option<Request> { panic!("controlled source failure") };
    let outcome = run(
        Configuration::default(),
        &Callbacks {
            store: &store,
            source: &source,
            work: &completed,
            deliver: &delivered,
        },
        &RunControl::default(),
    );
    assert!(!outcome.positively_settled());
    assert!(outcome.source_failure_before_db.is_some());
    assert!(outcome.unresolved.is_empty());
    assert_eq!(store.writes.load(Ordering::Acquire), 0);
}

#[test]
fn sh_unknown_survives_queue_refusal_and_returns_same_full_owner_without_deadline_settlement() {
    let requests = vec![request(-7, true)];
    let attempt = |_: &CommandWitness| Behavior::Unknown;
    let first = AtomicUsize::new(0);
    let read = |_: &CommandWitness| {
        if first.fetch_add(1, Ordering::AcqRel) == 0 {
            Behavior::Refuse
        } else {
            Behavior::Unknown
        }
    };
    let mut store = Store::new(&requests);
    store.attempt = &attempt;
    store.read = &read;
    let inputs = Mutex::new(VecDeque::from(requests));
    let source = || inputs.lock().unwrap().pop_front();
    let control = RunControl::default();
    thread::scope(|scope| {
        let _end_run = EndRun(&control);
        let runner = scope.spawn(|| {
            run(
                Configuration::new(1).unwrap(),
                &Callbacks {
                    store: &store,
                    source: &source,
                    work: &completed,
                    deliver: &delivered,
                },
                &control,
            )
        });
        wait_for(|| store.reads.load(Ordering::Acquire) >= 1);
        pump(&control, || store.reads.load(Ordering::Acquire) >= 2);
        control.return_unresolved();
        let outcome = runner.join().unwrap();
        assert!(!outcome.positively_settled());
        assert!(outcome.measurements.joined);
        assert_eq!(outcome.unresolved.len(), 1);
        let owner = &outcome.unresolved[0];
        assert!(owner.unknown);
        assert_eq!(owner.request.complete.item.id, -7);
        assert_eq!(
            owner.request.complete.item.story_uuid.as_deref(),
            Some("UPPER raw uuid")
        );
        assert!(owner.execution.is_some());
        assert_eq!(store.writes.load(Ordering::Acquire), 1);
    });
}

#[test]
fn sh_committed_without_view_never_recounts_after_fresh_view_refusal() {
    let requests = vec![request(0, true)];
    let first = AtomicUsize::new(0);
    let read = |_: &CommandWitness| {
        if first.fetch_add(1, Ordering::AcqRel) == 0 {
            Behavior::Refuse
        } else {
            Behavior::Pass
        }
    };
    let mut store = Store::new(&requests);
    store.read = &read;
    let inputs = Mutex::new(VecDeque::from(requests));
    let source = || inputs.lock().unwrap().pop_front();
    let control = RunControl::default();
    thread::scope(|scope| {
        let _end_run = EndRun(&control);
        let runner = scope.spawn(|| {
            run(
                Configuration::new(1).unwrap(),
                &Callbacks {
                    store: &store,
                    source: &source,
                    work: &completed,
                    deliver: &delivered,
                },
                &control,
            )
        });
        wait_for(|| store.reads.load(Ordering::Acquire) >= 1);
        pump(&control, || runner.is_finished());
        let outcome = runner.join().unwrap();
        assert!(outcome.positively_settled());
        assert_eq!(store.writes.load(Ordering::Acquire), 2); // Complete ONCE + supplied Finish ONCE.
    });
}

#[test]
fn sh_virtual_pacing_has_three_attempts_no_catch_up_and_checked_ticks() {
    let request = request(0, false);
    let store = Store::new(&[]);
    let rollback = |_: &CommandWitness| Behavior::Rollback;
    let mut store = store;
    store.attempt = &rollback;
    let control = RunControl::default();
    let mut driver = Driver::new(request, 0);
    driver.accept_work(Ok(completed(&driver.owner.request, &control)), 0);
    for tick in [0, 10, 20] {
        control.advance(tick).unwrap();
        assert!(driver.ready(tick));
        driver.step(&store, &control);
    }
    assert!(matches!(driver.owner.witness, CommandWitness::Interrupt(_)));
    control.advance(5000).unwrap();
    driver.step(&store, &control);
    assert!(!driver.ready(5000));
    assert!(!driver.ready(5009));
    assert!(driver.ready(5010));
    assert_eq!(control.advance(4999), Err(PaceError::Regression));
    assert_eq!(control.advance(u64::MAX), Err(PaceError::Overflow));
}

#[derive(Default)]
struct Gate {
    open: Mutex<bool>,
    wake: Condvar,
}
impl Gate {
    fn wait(&self) {
        let open = self.open.lock().unwrap();
        drop(self.wake.wait_while(open, |open| !*open).unwrap());
    }
    fn release(&self) {
        *self.open.lock().unwrap() = true;
        self.wake.notify_all();
    }
}
struct Release<'a>(&'a Gate);
impl Drop for Release<'_> {
    fn drop(&mut self) {
        self.0.release();
    }
}

#[test]
fn sh_intake_stop_work_cancel_and_unresolved_observation_do_not_detach_blocked_join() {
    let requests = vec![request(0, false)];
    let store = Store::new(&requests);
    let inputs = Mutex::new(VecDeque::from(requests));
    let source = || inputs.lock().unwrap().pop_front();
    let gate = Gate::default();
    let _release = Release(&gate);
    let entered = AtomicUsize::new(0);
    let work = |request: &Request, control: &RunControl| {
        entered.store(1, Ordering::Release);
        gate.wait();
        assert!(control.work_cancelled());
        WorkReply::completed(request.complete.clone())
    };
    let control = RunControl::default();
    let (tx, rx) = mpsc::channel();
    thread::scope(|scope| {
        let _end_run = EndRun(&control);
        let _release_in_scope = Release(&gate);
        let runner = scope.spawn(|| {
            let value = run(
                Configuration::new(1).unwrap(),
                &Callbacks {
                    store: &store,
                    source: &source,
                    work: &work,
                    deliver: &delivered,
                },
                &control,
            );
            tx.send(()).unwrap();
            value
        });
        wait_for(|| entered.load(Ordering::Acquire) == 1);
        control.stop_intake();
        assert!(!control.work_cancelled());
        control.cancel_work();
        control.return_unresolved();
        assert!(rx.try_recv().is_err());
        assert!(!runner.is_finished()); // Deadline is pending, not a detached join.
        gate.release();
        let outcome = runner.join().unwrap();
        assert!(!outcome.positively_settled());
        assert!(outcome.measurements.joined);
        assert_eq!(outcome.unresolved.len(), 1);
        assert!(outcome.unresolved[0].execution.is_some());
        assert_eq!(store.writes.load(Ordering::Acquire), 0);
    });
}

#[test]
fn sh_w8_shared_four_controlled_calls_refuse_without_extra_owners_or_hidden_queue() {
    let requests: Vec<_> = (0..40).map(|id| request(id, false)).collect();
    let gate = Gate::default();
    let _release = Release(&gate);
    let entered = AtomicUsize::new(0);
    let attempt = |_: &CommandWitness| {
        let order = entered.fetch_add(1, Ordering::AcqRel);
        if order < 4 {
            gate.wait();
            Behavior::Unknown
        } else {
            Behavior::Refuse
        }
    };
    let mut store = Store::new(&requests);
    store.attempt = &attempt;
    let inputs = Mutex::new(VecDeque::from(requests));
    let offered = AtomicUsize::new(0);
    let source = || {
        let value = inputs.lock().unwrap().pop_front();
        if value.is_some() {
            offered.fetch_add(1, Ordering::AcqRel);
        }
        value
    };
    let control = RunControl::default();
    thread::scope(|scope| {
        let _end_run = EndRun(&control);
        let _release_in_scope = Release(&gate);
        let runner = scope.spawn(|| {
            run(
                Configuration::new(8).unwrap(),
                &Callbacks {
                    store: &store,
                    source: &source,
                    work: &completed,
                    deliver: &delivered,
                },
                &control,
            )
        });
        wait_for(|| entered.load(Ordering::Acquire) == 8 && offered.load(Ordering::Acquire) == 25);
        control.return_unresolved();
        gate.release();
        let outcome = runner.join().unwrap();
        assert_eq!(outcome.unresolved.len(), 8);
        assert_eq!(outcome.unstarted.len(), 17);
        let m = &outcome.measurements;
        assert_eq!(m.max_ready, 16);
        assert_eq!(m.max_prefetch, 1);
        assert_eq!(m.max_retained, 8);
        assert_eq!(m.max_dispatched_handles, 25);
        assert!(m.max_callback_owners <= 9);
        assert!(m.max_command_bytes > 0);
        assert_eq!(m.settled, 0);
        assert!(
            outcome
                .unresolved
                .iter()
                .all(|owner| owner.execution.is_some())
        );
    });
}

#[test]
fn sh_delivery_panic_preserves_committed_fact_and_original_error_fields() {
    let requests = vec![request(0, false)];
    let store = Store::new(&requests);
    let inputs = Mutex::new(VecDeque::from(requests));
    let source = || inputs.lock().unwrap().pop_front();
    let observed = AtomicUsize::new(0);
    let deliver = |_: &Retained| -> Delivery {
        observed.store(1, Ordering::Release);
        panic!("controlled observer failure")
    };
    let control = RunControl::default();
    thread::scope(|scope| {
        let _end_run = EndRun(&control);
        let runner = scope.spawn(|| {
            run(
                Configuration::new(1).unwrap(),
                &Callbacks {
                    store: &store,
                    source: &source,
                    work: &completed,
                    deliver: &deliver,
                },
                &control,
            )
        });
        wait_for(|| observed.load(Ordering::Acquire) == 1);
        control.return_unresolved();
        let outcome = runner.join().unwrap();
        assert!(!outcome.positively_settled());
        let owner = &outcome.unresolved[0];
        assert!(owner.complete_acknowledged);
        assert!(!owner.unknown);
        assert!(owner.latest_delivery_cause.is_some());
        assert!(owner.latest_store_cause.is_some());
        assert_eq!(store.writes.load(Ordering::Acquire), 1);
    });
}

#[test]
fn sh_wrong_full_witness_never_acknowledges_and_keeps_only_bounded_latest_diagnostics() {
    let request = request(-7, false);
    let store = Store::new(&[]);
    let wrong = |_: &CommandWitness| Behavior::WrongWitness;
    let mut store = store;
    store.attempt = &wrong;
    store.read = &wrong;
    let control = RunControl::default();
    let mut driver = Driver::new(request, 0);
    driver.accept_work(Ok(completed(&driver.owner.request, &control)), 0);
    driver.step(&store, &control);
    assert!(driver.owner.unknown);
    assert!(!driver.owner.complete_acknowledged);
    for tick in [0, 1000, 2000, 3000] {
        control.advance(tick).unwrap();
        driver.step(&store, &control);
    }
    assert!(driver.owner.unknown);
    assert_eq!(store.writes.load(Ordering::Acquire), 1);
    assert!(driver.owner.latest_store_cause.as_ref().unwrap().text.len() <= 1024);
    assert_eq!(driver.owner.request.complete.item.id, -7);
}

#[test]
fn sh_blocked_attempt_uses_observed_return_tick_instead_of_catching_up() {
    let request = request(0, false);
    let control = RunControl::default();
    let blocked = |_: &CommandWitness| {
        control.advance(5000).unwrap();
        Behavior::Rollback
    };
    let mut store = Store::new(&[]);
    store.attempt = &blocked;
    let mut driver = Driver::new(request, 0);
    driver.accept_work(Ok(completed(&driver.owner.request, &control)), 0);
    driver.step(&store, &control);
    assert!(!driver.ready(5000));
    assert!(!driver.ready(5009));
    assert!(driver.ready(5010));
}

#[test]
fn sh_actual_different_terminal_truth_is_preserved_without_overwriting_finish() {
    let requests = vec![request(0, true)];
    let conflict = |_: &CommandWitness| Behavior::Conflict;
    let mut store = Store::new(&requests);
    store.read = &conflict;
    let inputs = Mutex::new(VecDeque::from(requests));
    let source = || inputs.lock().unwrap().pop_front();
    let deliver = |owner: &Retained| {
        assert!(owner.complete_acknowledged);
        assert!(owner.different_terminal);
        assert_eq!(
            owner
                .view
                .as_ref()
                .unwrap()
                .operation
                .identity
                .finished_at
                .as_deref(),
            Some("different RAW terminal time")
        );
        Delivery::Accepted
    };
    let outcome = run(
        Configuration::new(1).unwrap(),
        &Callbacks {
            store: &store,
            source: &source,
            work: &completed,
            deliver: &deliver,
        },
        &RunControl::default(),
    );
    assert!(outcome.positively_settled());
    assert_eq!(store.writes.load(Ordering::Acquire), 1);
}

#[test]
fn sh_interruption_has_own_budget_then_one_reconciliation_step_per_observed_second() {
    let request = request(0, false);
    let mut store = Store::new(&[super::tests::request(0, false)]);
    let rollback = |_: &CommandWitness| Behavior::Rollback;
    store.attempt = &rollback;
    let control = RunControl::default();
    let mut driver = Driver::new(request, 0);
    driver.accept_work(Ok(completed(&driver.owner.request, &control)), 0);
    for tick in [0, 10, 20, 20, 30, 40] {
        control.advance(tick).unwrap();
        driver.step(&store, &control);
    }
    assert_eq!(store.writes.load(Ordering::Acquire), 6);
    assert!(!driver.ready(1039));
    control.advance(1040).unwrap();
    driver.step(&store, &control);
    assert_eq!(store.reads.load(Ordering::Acquire), 1);
    assert!(!driver.ready(1040));
    control.advance(2040).unwrap();
    driver.step(&store, &control);
    assert!(!driver.ready(2040));
    assert_eq!(store.writes.load(Ordering::Acquire), 7);
    control.advance(3040).unwrap();
    driver.step(&store, &control);
    assert_eq!(store.reads.load(Ordering::Acquire), 2);
}

#[test]
fn sh_invalid_supplied_scope_returns_original_without_work_or_db_admission() {
    let mut request = request(-7, false);
    request.interrupt.generation = 99;
    let store = Store::new(&[super::tests::request(-7, false)]);
    let inputs = Mutex::new(VecDeque::from([request]));
    let source = || inputs.lock().unwrap().pop_front();
    let work =
        |_: &Request, _: &RunControl| -> WorkReply { panic!("invalid request must not execute") };
    let outcome = run(
        Configuration::default(),
        &Callbacks {
            store: &store,
            source: &source,
            work: &work,
            deliver: &delivered,
        },
        &RunControl::default(),
    );
    assert!(!outcome.positively_settled());
    assert_eq!(outcome.unresolved.len(), 1);
    assert_eq!(outcome.unresolved[0].request.complete.item.id, -7);
    assert!(!outcome.unresolved[0].unknown);
    assert_eq!(store.writes.load(Ordering::Acquire), 0);
}

#[test]
fn sh_active_parent_target_terminal_conflict_keeps_actual_item_without_recount_or_finish() {
    struct ItemConflict<'a> {
        saved: &'a TerminalView,
        via_readback: bool,
        attempts: AtomicUsize,
        reads: AtomicUsize,
    }
    impl OperationTerminalStore for ItemConflict<'_> {
        fn snapshot(&self, _: &str) -> SnapshotReply {
            panic!("no pooled snapshot")
        }
        fn complete_item(&self, command: &CompleteItemCommand) -> AttemptReply {
            self.attempts.fetch_add(1, Ordering::AcqRel);
            AttemptReply {
                witness: CommandWitness::CompleteItem(Box::new(command.clone())),
                diagnostic: None,
                disposition: if self.via_readback {
                    AttemptDisposition::Unknown(UnknownOutcome::AcknowledgementLost)
                } else {
                    AttemptDisposition::TerminalConflict(Box::new(self.saved.clone()))
                },
            }
        }
        fn finish(&self, _: &FinishCommand) -> AttemptReply {
            panic!("item conflict is not a Finish plan")
        }
        fn interrupt(&self, _: &InterruptCommand) -> AttemptReply {
            panic!("saved target must not be overwritten")
        }
        fn readback(&self, witness: &CommandWitness) -> ReadbackReply {
            self.reads.fetch_add(1, Ordering::AcqRel);
            ReadbackReply {
                witness: witness.clone(),
                diagnostic: None,
                disposition: ReadbackDisposition::TerminalConflict(Box::new(self.saved.clone())),
            }
        }
    }
    // R4 policy::inspect returns Conflict for terminal target OR terminal
    // parent, after checking immutable inputs and ALL-sibling coherence.
    for via_readback in [false, true] {
        let request = request(-7, true);
        let mut saved = initial(&request);
        saved.operation.counts.completed = 1;
        saved.siblings.terminal = 1;
        let item = saved.item.as_mut().unwrap();
        item.state = ItemState::Failed;
        item.identity.story_id = Some(321);
        item.progress.completed = 6;
        item.outcome = RawOutcome {
            code: Some("actual_saved_item".into()),
            message: Some(String::new()),
        };
        let store = ItemConflict {
            saved: &saved,
            via_readback,
            attempts: AtomicUsize::new(0),
            reads: AtomicUsize::new(0),
        };
        let control = RunControl::default();
        let mut driver = Driver::new(request, 0);
        driver.accept_work(Ok(completed(&driver.owner.request, &control)), 0);
        driver.step(&store, &control);
        if via_readback {
            driver.step(&store, &control);
        }
        assert!(
            driver.delivery(),
            "coherent item-only terminal conflict must resolve to actual saved truth"
        );
        assert!(!driver.owner.unknown);
        assert!(!driver.owner.complete_acknowledged);
        assert!(driver.owner.different_terminal);
        assert_eq!(driver.owner.view.as_deref(), Some(&saved));
        assert_eq!(saved.operation.state, OperationState::Running);
        assert_eq!(saved.operation.identity.finished_at, None);
        assert!(matches!(
            driver.owner.witness,
            CommandWitness::CompleteItem(_)
        ));
        assert_eq!(
            driver.owner.execution.as_ref().unwrap().state,
            ItemState::Succeeded
        );
        assert_eq!(store.attempts.load(Ordering::Acquire), 1);
        assert_eq!(
            store.reads.load(Ordering::Acquire),
            usize::from(via_readback)
        );
    }
}

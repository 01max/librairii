use super::*;
use crate::jobs::terminal::*;
use librairii_core::{
    OperationState,
    operations::{Counts, ObservationKind, OperationObservation, RawOutcome},
};
use std::ops::Deref;

#[derive(Clone, Copy)]
enum WriteStep {
    Apply,
    Fail,
    Unknown,
    CommitThenUnknown,
}
#[derive(Clone, Copy)]
enum ReadStep {
    Match,
    NotApplied,
    Unavailable,
    Contradictory,
    WrongGeneration,
    WrongJob,
    WrongItem,
    WrongCommand,
    Provisional,
    DifferentTerminal,
}

#[derive(Default)]
struct Script {
    writes: Vec<(Witness, u64)>,
    reads: Vec<(Witness, u64)>,
    write_steps: VecDeque<WriteStep>,
    read_steps: VecDeque<ReadStep>,
    applied: Vec<Witness>,
    completion_count: usize,
    settle: bool,
    totals: BTreeMap<JobId, i64>,
}

fn observation(state: OperationState) -> OperationObservation {
    OperationObservation {
        state,
        kind: ObservationKind::StoredSnapshot,
    }
}

fn finish() -> FinishIntent {
    FinishIntent {
        state: OperationState::Succeeded,
        counts: Counts {
            completed: 1,
            total: 1,
        },
        outcome: RawOutcome {
            code: None,
            message: Some(String::new()),
        },
        finished_at: Some("RAW-legacy-time+00:00".into()),
    }
}

fn applied(witness: &Witness) -> Applied {
    let (state, next_finish) = match &witness.command {
        Command::CompleteItem { .. } => (OperationState::Running, Some(finish())),
        Command::Finish(intent) => (intent.state, None),
        Command::Interrupt { .. } => (OperationState::Interrupted, None),
    };
    Applied {
        witness: witness.clone(),
        observation: observation(state),
        next_finish,
    }
}

impl Script {
    fn applied_response(&self, witness: &Witness) -> Applied {
        let mut value = applied(witness);
        if matches!(witness.command, Command::CompleteItem { .. }) {
            let completed = self
                .applied
                .iter()
                .filter(|w| {
                    w.stamp.item.job == witness.stamp.item.job
                        && matches!(w.command, Command::CompleteItem { .. })
                })
                .count() as i64;
            let total = self
                .totals
                .get(&witness.stamp.item.job)
                .copied()
                .unwrap_or(1);
            value.next_finish = (completed == total).then(|| FinishIntent {
                counts: Counts { completed, total },
                ..finish()
            });
        }
        value
    }
    fn record_application(&mut self, witness: &Witness) {
        if !self.applied.contains(witness) {
            if matches!(witness.command, Command::CompleteItem { .. }) {
                self.completion_count += 1;
            }
            self.applied.push(witness.clone());
        }
    }

    fn attempt(&mut self, witness: &Witness, tick: u64) -> Attempt {
        self.writes.push((witness.clone(), tick));
        let step = if self.settle {
            WriteStep::Apply
        } else {
            self.write_steps.pop_front().unwrap_or(WriteStep::Apply)
        };
        match step {
            WriteStep::Apply => {
                self.record_application(witness);
                Attempt::Applied(self.applied_response(witness))
            }
            WriteStep::Fail => Attempt::ReportedNotApplied {
                witness: witness.clone(),
                cause: Cause::new("reported nonapplication"),
            },
            WriteStep::Unknown => Attempt::Unknown {
                witness: witness.clone(),
                cause: Cause::new("unknown acknowledgement"),
            },
            WriteStep::CommitThenUnknown => {
                self.record_application(witness);
                Attempt::Unknown {
                    witness: witness.clone(),
                    cause: Cause::new("applied before response loss"),
                }
            }
        }
    }

    fn readback(&mut self, witness: &Witness, tick: u64) -> Readback {
        self.reads.push((witness.clone(), tick));
        let step = if self.settle {
            ReadStep::Match
        } else {
            self.read_steps.pop_front().unwrap_or(ReadStep::Match)
        };
        match step {
            ReadStep::Match => {
                self.record_application(witness);
                Readback::Matches(self.applied_response(witness))
            }
            ReadStep::NotApplied => Readback::ReportedNotApplied {
                witness: witness.clone(),
                cause: Cause::new("reported coherent nonapplication"),
            },
            ReadStep::Unavailable => Readback::Unavailable(Cause::new("unavailable fresh view")),
            ReadStep::Contradictory => {
                Readback::Contradictory(Cause::new("contradictory counters"))
            }
            ReadStep::DifferentTerminal => Readback::DifferentSavedTerminal {
                witness: witness.clone(),
                observation: observation(OperationState::Cancelled),
            },
            ReadStep::Provisional => {
                let mut value = applied(witness);
                value.observation = OperationObservation {
                    state: OperationState::Interrupted,
                    kind: ObservationKind::ProvisionalPersistenceFailure,
                };
                Readback::Matches(value)
            }
            identity => {
                let mut value = applied(witness);
                match identity {
                    ReadStep::WrongGeneration => value.witness.stamp.generation = Generation(999),
                    ReadStep::WrongJob => {
                        value.witness.stamp.item.job = JobId::from_bytes([222; 16])
                    }
                    ReadStep::WrongItem => value.witness.stamp.item.row += 1,
                    ReadStep::WrongCommand => value.witness.command = Command::Finish(finish()),
                    _ => return Readback::Unavailable(Cause::new("unsupported script")),
                }
                Readback::Matches(value)
            }
        }
    }
}

struct TerminalRig {
    rig: Rig,
    script: Arc<Mutex<Script>>,
}
impl Deref for TerminalRig {
    type Target = Rig;
    fn deref(&self) -> &Rig {
        &self.rig
    }
}
impl TerminalRig {
    fn created(&self, input: StartInput) -> TestResult {
        guard(&self.script)
            .totals
            .insert(input.job, input.items.len() as i64);
        self.rig.created(input)
    }
    fn new(
        workers: usize,
        gates: Gates,
        writes: &[WriteStep],
        reads: &[ReadStep],
    ) -> io::Result<Self> {
        Self::with_report_uncertainty(workers, gates, writes, reads, false)
    }

    fn with_report_uncertainty(
        workers: usize,
        gates: Gates,
        writes: &[WriteStep],
        reads: &[ReadStep],
        uncertain_report: bool,
    ) -> io::Result<Self> {
        let script = Arc::new(Mutex::new(Script {
            write_steps: writes.iter().copied().collect(),
            read_steps: reads.iter().copied().collect(),
            ..Script::default()
        }));
        let clock = Arc::new(Mutex::new(None::<Handle>));
        let callbacks = Callbacks {
            attempt: {
                let gate = Arc::clone(&gates.terminal_attempt);
                let script = Arc::clone(&script);
                let clock = Arc::clone(&clock);
                Arc::new(move |witness, _completed| {
                    gate.pass();
                    let now = guard(&clock).as_ref().map_or(0, Handle::persistence_time);
                    guard(&script).attempt(witness, now)
                })
            },
            readback: {
                let gate = Arc::clone(&gates.terminal_readback);
                let script = Arc::clone(&script);
                let clock = Arc::clone(&clock);
                Arc::new(move |witness| {
                    gate.pass();
                    let now = guard(&clock).as_ref().map_or(0, Handle::persistence_time);
                    guard(&script).readback(witness, now)
                })
            },
        };
        let rig = Rig::with_terminal(
            workers,
            gates,
            StartPolicy::Create,
            uncertain_report,
            Arc::new(Mutex::new(Store::default())),
            Some(callbacks),
        )?;
        *guard(&clock) = Some(rig.handle.clone());
        Ok(Self { rig, script })
    }

    fn due(&self, due: u64, action: Action) -> io::Result<()> {
        until(|| {
            self.handle
                .persistence_status()
                .iter()
                .any(|s| s.due == due && s.action == action)
        })
    }

    fn before_due(&self, tick: u64, writes: usize, reads: usize) -> TestResult {
        self.handle.advance_persistence_time(tick)?;
        until(|| {
            self.handle
                .persistence_status()
                .iter()
                .all(|s| s.observed_tick >= tick)
        })?;
        let script = guard(&self.script);
        assert_eq!((script.writes.len(), script.reads.len()), (writes, reads));
        Ok(())
    }

    fn finish(&mut self) -> Result<Measurements, Box<dyn Error>> {
        self.rig.gates.release();
        self.rig.handle.wake_repair();
        self.rig
            .engine
            .take()
            .ok_or("missing terminal engine")?
            .join()
            .map_err(Into::into)
    }
}
impl Drop for TerminalRig {
    fn drop(&mut self) {
        // Harness cleanup on assertion failure, not a product deadline escape:
        // supply settlement, release actual barriers, advance once, then join.
        guard(&self.script).settle = true;
        self.rig.gates.release();
        if let Some(next) = self.rig.handle.persistence_time().checked_add(1_000) {
            let _ = self.rig.handle.advance_persistence_time(next);
        }
        self.rig.handle.wake_repair();
    }
}

#[test]
fn j65_complete_retries_three_times_at_ten_ms_without_replaying_for_finish() -> TestResult {
    let mut rig = TerminalRig::new(
        1,
        Gates::new(),
        &[WriteStep::Fail, WriteStep::Fail, WriteStep::Apply],
        &[],
    )?;
    rig.created(input(81, OperationKind::Import, 1, 32))?;
    rig.due(10, Action::Write)?;
    rig.before_due(9, 1, 0)?;
    rig.handle.advance_persistence_time(10)?;
    rig.due(20, Action::Write)?;
    rig.before_due(19, 2, 0)?;
    rig.handle.advance_persistence_time(20)?;
    until(|| rig.handle.measurements().recorded == 1)?;
    let script = guard(&rig.script);
    assert_eq!(
        script
            .writes
            .iter()
            .filter(|(w, _)| matches!(w.command, Command::CompleteItem { .. }))
            .map(|(_, t)| *t)
            .collect::<Vec<_>>(),
        [0, 10, 20]
    );
    assert_eq!(
        script
            .writes
            .iter()
            .filter(|(w, _)| matches!(w.command, Command::Finish(_)))
            .count(),
        1
    );
    assert_eq!(script.completion_count, 1);
    drop(script);
    assert_eq!(guard(&rig.store).recorded[0].fact, "observed-success");
    assert_eq!(
        guard(&rig.store).recorded[0].errors,
        ["independent-cleanup-diagnostic"]
    );
    assert!(rig.finish()?.joined);
    Ok(())
}

#[test]
fn j65_finish_and_interrupt_have_separate_budgets_then_paced_reconciliation() -> TestResult {
    let steps = [
        WriteStep::Apply,
        WriteStep::Fail,
        WriteStep::Fail,
        WriteStep::Fail,
        WriteStep::Fail,
        WriteStep::Fail,
        WriteStep::Fail,
        WriteStep::Fail,
        WriteStep::Apply,
    ];
    let mut rig = TerminalRig::new(1, Gates::new(), &steps, &[ReadStep::NotApplied])?;
    rig.created(input(82, OperationKind::MetadataSync, 1, 64))?;
    for (tick, due) in [(10, 20), (20, 30), (30, 40), (40, 1040)] {
        rig.due(tick, Action::Write)?;
        rig.handle.advance_persistence_time(tick)?;
        rig.due(due, Action::Write)?;
    }
    let pending = rig.handle.terminal_observations();
    assert_eq!(pending.len(), 1);
    assert!(!pending[0].1.is_saved_terminal());
    assert_eq!(
        pending[0].1.kind,
        ObservationKind::ProvisionalPersistenceFailure
    );
    assert!(guard(&rig.store).terminal_truth.is_empty());
    rig.before_due(1039, 7, 0)?;
    rig.handle.advance_persistence_time(1040)?;
    rig.due(2040, Action::Write)?;
    assert_eq!(guard(&rig.script).reads.len(), 1);
    rig.before_due(2039, 8, 1)?;
    rig.handle.advance_persistence_time(2040)?;
    until(|| rig.handle.measurements().recorded == 1)?;
    let script = guard(&rig.script);
    assert_eq!(
        script
            .writes
            .iter()
            .filter(|(w, _)| matches!(w.command, Command::Finish(_)))
            .map(|(_, t)| *t)
            .collect::<Vec<_>>(),
        [0, 10, 20]
    );
    assert_eq!(
        script
            .writes
            .iter()
            .filter(|(w, _)| matches!(w.command, Command::Interrupt { .. }))
            .map(|(_, t)| *t)
            .collect::<Vec<_>>(),
        [20, 30, 40, 1040, 2040]
    );
    assert_eq!(script.completion_count, 1);
    drop(script);
    let store = guard(&rig.store);
    assert_eq!(store.recorded[0].fact, "observed-success");
    assert_eq!(store.recorded[0].errors, ["independent-cleanup-diagnostic"]);
    assert_eq!(
        store.terminal_truth[0].observation,
        observation(OperationState::Interrupted)
    );
    drop(store);
    assert!(rig.finish()?.joined);
    Ok(())
}

#[test]
fn j65_lost_complete_or_finish_ack_uses_readback_without_second_write_or_count() -> TestResult {
    for steps in [
        vec![WriteStep::CommitThenUnknown],
        vec![WriteStep::Apply, WriteStep::CommitThenUnknown],
    ] {
        let mut rig = TerminalRig::new(1, Gates::new(), &steps, &[ReadStep::Match])?;
        rig.created(input(83, OperationKind::Export, 1, 19))?;
        until(|| rig.handle.measurements().recorded == 1)?;
        let script = guard(&rig.script);
        assert_eq!(script.writes.len(), 2);
        assert_eq!(script.reads.len(), 1);
        assert_eq!(script.completion_count, 1);
        assert_eq!(script.reads[0].0, script.writes[steps.len() - 1].0);
        drop(script);
        assert_eq!(
            guard(&rig.store).terminal_truth[0].observation.state,
            OperationState::Succeeded
        );
        assert!(rig.finish()?.joined);
    }
    Ok(())
}

#[test]
fn j65_interrupt_commit_then_snapshot_error_is_unknown_until_matching_readback() -> TestResult {
    let gates = Gates::new();
    gates.terminal_readback.close();
    let mut rig = TerminalRig::new(
        1,
        gates.clone(),
        &[
            WriteStep::Fail,
            WriteStep::Fail,
            WriteStep::Fail,
            WriteStep::CommitThenUnknown,
        ],
        &[ReadStep::Match],
    )?;
    let request = input(84, OperationKind::Import, 1, 27);
    let allocation = request.items.front().map(|i| i.payload.as_ptr() as usize);
    rig.created(request)?;
    rig.due(10, Action::Write)?;
    rig.handle.advance_persistence_time(10)?;
    rig.due(20, Action::Write)?;
    rig.handle.advance_persistence_time(20)?;
    until(|| gates.terminal_readback.entries() == 1)?;
    assert_eq!(guard(&rig.script).writes.len(), 4);
    assert!(guard(&rig.store).recorded.is_empty());
    assert_eq!(rig.handle.measurements().recorded, 0);
    gates.terminal_readback.open();
    until(|| rig.handle.measurements().recorded == 1)?;
    let store = guard(&rig.store);
    assert_eq!(
        Some(store.recorded[0].item.payload.as_ptr() as usize),
        allocation
    );
    assert_eq!(
        store.terminal_truth[0].observation,
        observation(OperationState::Interrupted)
    );
    drop(store);
    assert_eq!(guard(&rig.script).writes.len(), 4);
    assert!(rig.finish()?.joined);
    Ok(())
}

#[test]
fn j65_unknown_ack_rejects_wrong_or_provisional_readback_and_keeps_terminal_conflict() -> TestResult
{
    let reads = [
        ReadStep::Unavailable,
        ReadStep::Contradictory,
        ReadStep::WrongGeneration,
        ReadStep::WrongJob,
        ReadStep::WrongItem,
        ReadStep::WrongCommand,
        ReadStep::Provisional,
        ReadStep::DifferentTerminal,
    ];
    let mut rig = TerminalRig::new(1, Gates::new(), &[WriteStep::Unknown], &reads)?;
    let request = input(85, OperationKind::Import, 1, 29);
    let allocation = request.items.front().map(|i| i.payload.as_ptr() as usize);
    rig.created(request)?;
    for index in 1..=7 {
        let due = index * 1_000;
        rig.due(due, Action::Readback)?;
        rig.before_due(due - 1, 1, index as usize)?;
        assert!(guard(&rig.store).recorded.is_empty());
        rig.handle.advance_persistence_time(due)?;
    }
    until(|| rig.handle.measurements().recorded == 1)?;
    let script = guard(&rig.script);
    assert_eq!(
        (
            script.writes.len(),
            script.reads.len(),
            script.completion_count
        ),
        (1, 8, 0)
    );
    drop(script);
    let store = guard(&rig.store);
    assert_eq!(store.recorded[0].fact, "observed-success");
    assert_eq!(store.recorded[0].errors, ["independent-cleanup-diagnostic"]);
    assert_eq!(
        Some(store.recorded[0].item.payload.as_ptr() as usize),
        allocation
    );
    assert!(store.terminal_truth[0].different_terminal);
    assert_eq!(
        store.terminal_truth[0].observation,
        observation(OperationState::Cancelled)
    );
    drop(store);
    assert!(rig.finish()?.joined);
    Ok(())
}

#[test]
fn j65_blocked_callback_and_tick_jump_schedule_from_observed_response_without_catchup() -> TestResult
{
    let gates = Gates::new();
    gates.terminal_attempt.close();
    let mut rig = TerminalRig::new(
        1,
        gates.clone(),
        &[WriteStep::Fail, WriteStep::Fail, WriteStep::Apply],
        &[],
    )?;
    rig.created(input(86, OperationKind::Import, 1, 43))?;
    until(|| gates.terminal_attempt.entries() == 1)?;
    rig.handle.advance_persistence_time(100_000)?;
    gates.terminal_attempt.open();
    rig.due(100_010, Action::Write)?;
    rig.before_due(100_009, 1, 0)?;
    assert_eq!(rig.handle.persistence_status()[0].attempts, 1);
    rig.handle.advance_persistence_time(100_010)?;
    rig.due(100_020, Action::Write)?;
    rig.before_due(100_019, 2, 0)?;
    rig.handle.advance_persistence_time(200_000)?;
    until(|| rig.handle.measurements().recorded == 1)?;
    let script = guard(&rig.script);
    assert_eq!(
        script
            .writes
            .iter()
            .filter(|(w, _)| matches!(w.command, Command::CompleteItem { .. }))
            .map(|(_, t)| *t)
            .collect::<Vec<_>>(),
        [100_000, 100_010, 200_000]
    );
    assert_eq!(script.writes.len(), 4); // one separate Finish, no retry catch-up burst
    drop(script);
    assert!(rig.finish()?.joined);
    Ok(())
}

#[test]
fn j65_tick_regression_overflow_and_latest_diagnostics_are_bounded_without_losing_originals()
-> TestResult {
    assert_eq!(checked_tick(10, 9), Err(PaceError::Regression));
    assert_eq!(checked_tick(0, u64::MAX), Err(PaceError::Overflow));
    let mut request = input(87, OperationKind::Import, 1, 31);
    let completed = Completed {
        item: request.items.pop_front().ok_or("missing item")?,
        fact: "actual-executor-fact".into(),
        errors: vec!["Original-é-".repeat(2_000)],
    };
    let original = completed.errors.clone();
    let mut driver = Driver::new(&completed, 0);
    for now in [0, 10, 20, 20, 30, 40] {
        driver.before_write();
        driver.accept_attempt(
            now,
            Attempt::ReportedNotApplied {
                witness: driver.witness.clone(),
                cause: Cause::new("initial-failure"),
            },
        );
    }
    assert!(driver.provisional && driver.reconciling);
    assert_eq!(driver.attempts, 3);
    for _ in 0..500 {
        let now = driver.due;
        driver.before_write();
        driver.accept_attempt(
            now,
            Attempt::ReportedNotApplied {
                witness: driver.witness.clone(),
                cause: Cause::new(&"é".repeat(2_000)),
            },
        );
        driver.accept_readback(
            now,
            Readback::Contradictory(Cause::new(&"界".repeat(2_000))),
        );
        assert!(driver.diagnostic_bytes() <= 2_048);
        assert_eq!(driver.attempts, 3);
        assert_eq!(driver.due, now + 1_000);
    }
    let cause = driver
        .latest_readback
        .as_ref()
        .ok_or("missing latest cause")?;
    assert!(cause.truncated && cause.original_bytes == 6_000 && cause.text.len() <= 1_024);
    assert!(cause.text.is_char_boundary(cause.text.len()));
    assert_eq!(completed.errors, original);
    assert_eq!(completed.fact, "actual-executor-fact");
    assert!(
        matches!(&driver.witness.command,Command::Interrupt{outcome} if outcome.code.as_deref()==Some("persistence_failed"))
    );
    let mut overflow = Driver::new(&completed, 0);
    overflow.before_write();
    overflow.accept_attempt(
        u64::MAX,
        Attempt::ReportedNotApplied {
            witness: overflow.witness.clone(),
            cause: Cause::new("failure at exhausted tick"),
        },
    );
    assert_eq!(overflow.action, Action::ClockBlocked);
    assert_eq!(overflow.ready(u64::MAX), None);
    let gates = Gates::new();
    gates.terminal_attempt.close();
    let mut rig = TerminalRig::new(1, gates.clone(), &[], &[])?;
    rig.created(input(88, OperationKind::Import, 1, 2))?;
    until(|| gates.terminal_attempt.entries() == 1)?;
    rig.handle.advance_persistence_time(100)?;
    assert_eq!(
        rig.handle.advance_persistence_time(99),
        Err(PaceError::Regression)
    );
    assert_eq!(
        rig.handle.advance_persistence_time(u64::MAX),
        Err(PaceError::Overflow)
    );
    assert_eq!(rig.handle.persistence_time(), 100);
    gates.terminal_attempt.open();
    until(|| rig.handle.measurements().recorded == 1)?;
    assert!(rig.finish()?.joined);
    Ok(())
}

#[test]
fn j65_saturated_stop_and_expired_wait_keep_all_required_owners_until_readback_accepts()
-> TestResult {
    let gates = Gates::new();
    gates.terminal_attempt.close();
    gates.terminal_readback.close();
    let mut rig = TerminalRig::new(
        2,
        gates.clone(),
        &[WriteStep::Unknown, WriteStep::Unknown],
        &[ReadStep::Match, ReadStep::Match],
    )?;
    rig.created(input(89, OperationKind::Export, 100, 53))?;
    until(|| {
        let m = rig.handle.measurements();
        m.dispatched_handles == 7 && m.item_bytes >= 7 * 53 && gates.terminal_attempt.entries() == 2
    })?;
    gates.admit.close();
    let pending = rig.start(input(90, OperationKind::Import, 1, 17))?;
    until(|| gates.admit.entries() == 2)?;
    assert_eq!(rig.handle.measurements().calls, 3);
    gates.terminal_attempt.open();
    until(|| gates.terminal_readback.entries() == 2)?;
    let m = rig.handle.measurements();
    assert_eq!(
        (m.repair_owners, m.fences, m.ingress, m.dispatched_handles),
        (2, 1, 1, 7)
    );
    assert!(m.witness_bytes > 0 && m.retry_diagnostic_bytes <= 4_096);
    let joining = rig.rig.joining()?;
    assert!(matches!(
        joining.receiver.recv_timeout(Duration::from_millis(5)),
        Err(mpsc::RecvTimeoutError::Timeout)
    ));
    assert!(guard(&rig.store).recorded.is_empty());
    assert_eq!(rig.handle.measurements().workers, 2);
    rig.handle.advance_persistence_time(100_000)?;
    assert!(matches!(
        joining.receiver.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    drop(pending);
    let measured = joining.finish()?;
    assert!(
        measured.joined
            && measured.dispatched_handles <= 7
            && measured.calls <= 3
            && measured.fences <= 2
    );
    assert_eq!(guard(&rig.store).recorded.len(), 7);
    assert!(guard(&rig.store).recorded[..2].iter().all(|v|v.fact=="observed-success" && v.errors==["independent-cleanup-diagnostic"]));
    Ok(())
}

#[test]
fn j65_full_command_payload_and_supplied_terminal_truth_are_checked_without_rewriting_raw_values()
-> TestResult {
    let mut request = input(91, OperationKind::Export, 1, 1);
    let completed = Completed {
        item: request.items.pop_front().ok_or("missing item")?,
        fact: "raw-domain-fact".into(),
        errors: vec!["cleanup-error".into()],
    };
    let mut acknowledged = Driver::new(&completed, 0);
    acknowledged.before_write();
    acknowledged.accept_attempt(
        0,
        Attempt::Applied(Applied {
            witness: acknowledged.witness.clone(),
            observation: observation(OperationState::Running),
            next_finish: Some(FinishIntent {
                counts: Counts {
                    completed: 0,
                    total: 1,
                },
                ..finish()
            }),
        }),
    );
    assert!(acknowledged.acknowledged_complete);
    assert_eq!(acknowledged.action, Action::Readback);
    acknowledged.accept_readback(
        1_000,
        Readback::ReportedNotApplied {
            witness: acknowledged.witness.clone(),
            cause: Cause::new("contradictory missing completion after ACK"),
        },
    );
    assert_eq!(acknowledged.action, Action::Readback);
    assert_eq!(acknowledged.due, 2_000);
    assert_eq!(acknowledged.attempts, 1); // no acknowledged CompleteItem replay
    let mut driver = Driver::new(&completed, 0);
    driver.before_write();
    let mut wrong = driver.witness.clone();
    wrong.command = Command::CompleteItem {
        fact: "different-domain-fact".into(),
    };
    driver.accept_attempt(
        0,
        Attempt::ReportedNotApplied {
            witness: wrong,
            cause: Cause::new("mismatched claimed rejection"),
        },
    );
    assert_eq!(driver.ready(0), Some(Action::Readback));
    assert_eq!(driver.attempts, 1);
    assert!(driver.resolution.is_none());
    let original = FinishIntent {
        counts: Counts {
            completed: 0,
            total: 0,
        },
        ..finish()
    };
    driver.witness.command = Command::Finish(original.clone());
    for change in 0..3 {
        let mut mismatch = driver.witness.clone();
        if let Command::Finish(intent) = &mut mismatch.command {
            match change {
                0 => intent.outcome.code = Some(String::new()), // None and empty are distinct
                1 => intent.finished_at = Some("raw-LEGACY-time+00:00".into()),
                _ => {
                    intent.counts = Counts {
                        completed: 1,
                        total: 1,
                    }
                }
            }
        }
        driver.accept_readback(
            change * 1_000,
            Readback::Matches(Applied {
                witness: mismatch,
                observation: observation(OperationState::Succeeded),
                next_finish: None,
            }),
        );
        assert!(driver.resolution.is_none());
        assert_eq!(driver.witness.command, Command::Finish(original.clone()));
    }
    driver.accept_readback(
        3_000,
        Readback::Matches(Applied {
            witness: driver.witness.clone(),
            observation: observation(OperationState::Cancelled),
            next_finish: None,
        }),
    );
    let truth = driver.resolution.ok_or("missing terminal conflict")?;
    assert!(truth.different_terminal);
    assert_eq!(truth.observation, observation(OperationState::Cancelled));
    assert_eq!(driver.witness.command, Command::Finish(original));
    assert_eq!(completed.fact, "raw-domain-fact");
    assert_eq!(completed.errors, ["cleanup-error"]);
    Ok(())
}

#[test]
fn j65_saved_terminal_keeps_owned_result_until_actual_report_transfer_without_persist_replay()
-> TestResult {
    let gates = Gates::new();
    gates.report_readback.close();
    let mut rig = TerminalRig::with_report_uncertainty(1, gates.clone(), &[], &[], true)?;
    let request = input(92, OperationKind::MetadataSync, 1, 65);
    let allocation = request.items.front().map(|i| i.payload.as_ptr() as usize);
    rig.created(request)?;
    until(|| rig.handle.measurements().uncertain_reports == 1)?;
    let seen = rig.handle.terminal_observations();
    assert_eq!(seen.len(), 1);
    assert!(seen[0].1.is_saved_terminal());
    assert_eq!(seen[0].1.state, OperationState::Succeeded);
    assert!(guard(&rig.store).recorded.is_empty());
    assert!(matches!(
        rig.handle
            .try_start(input(93, OperationKind::MetadataSync, 1, 1)),
        Err(Unaccepted {
            reason: Refusal::MetadataActive,
            ..
        })
    ));
    let joining = rig.rig.joining()?;
    rig.handle.wake_repair();
    until(|| gates.report_readback.entries() == 1)?;
    assert!(matches!(
        joining.receiver.recv_timeout(Duration::from_millis(5)),
        Err(mpsc::RecvTimeoutError::Timeout)
    ));
    assert_eq!(guard(&rig.script).writes.len(), 2);
    assert_eq!(guard(&rig.script).reads.len(), 0);
    gates.report_readback.open();
    assert!(joining.finish()?.joined);
    let store = guard(&rig.store);
    assert_eq!(
        Some(store.recorded[0].item.payload.as_ptr() as usize),
        allocation
    );
    assert_eq!(store.recorded[0].fact, "observed-success");
    assert_eq!(store.recorded[0].errors, ["independent-cleanup-diagnostic"]);
    assert_eq!(
        store.terminal_truth[0].observation,
        observation(OperationState::Succeeded)
    );
    Ok(())
}

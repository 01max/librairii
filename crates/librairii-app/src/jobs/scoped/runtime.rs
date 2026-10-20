use super::{driver::Driver, model::*};
use crate::ports::operations::*;
use std::{
    collections::VecDeque,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
};

#[derive(Default)]
struct Slot {
    key: Option<(u64, String)>,
    fenced: bool,
    commands: usize,
    views: usize,
    causes: usize,
    copies: usize,
}
struct State {
    ready: VecDeque<Request>,
    slots: Vec<Slot>,
    source_done: bool,
    measurements: Measurements,
    prefetch_bytes: usize,
}
struct Shared {
    state: Mutex<State>,
    calls: AtomicUsize,
    max_calls: AtomicUsize,
}
struct Calling<'a>(&'a Shared);
impl Drop for Calling<'_> {
    fn drop(&mut self) {
        self.0.calls.fetch_sub(1, Ordering::AcqRel);
    }
}
impl Shared {
    fn calling(&self) -> Calling<'_> {
        let count = self.calls.fetch_add(1, Ordering::AcqRel) + 1;
        self.max_calls.fetch_max(count, Ordering::AcqRel);
        Calling(self)
    }
    fn update(state: &mut State) {
        let retained = state.slots.iter().filter(|slot| slot.key.is_some()).count();
        let prefetch = usize::from(state.prefetch_bytes != 0);
        let bytes = state
            .slots
            .iter()
            .map(|slot| slot.commands + slot.key.as_ref().map_or(0, |(_, id)| id.capacity()))
            .sum::<usize>()
            + state.ready.iter().map(request_bytes).sum::<usize>()
            + state.prefetch_bytes;
        let measurements = &mut state.measurements;
        measurements.max_ready = measurements.max_ready.max(state.ready.len());
        measurements.max_prefetch = measurements.max_prefetch.max(prefetch);
        measurements.max_retained = measurements.max_retained.max(retained);
        measurements.max_dispatched_handles = measurements
            .max_dispatched_handles
            .max(state.ready.len() + retained + prefetch);
        measurements.max_command_bytes = measurements.max_command_bytes.max(bytes);
        measurements.max_store_field_copy_budget_bytes = measurements
            .max_store_field_copy_budget_bytes
            .max(state.slots.iter().map(|slot| slot.copies).sum());
        measurements.max_view_bytes = measurements
            .max_view_bytes
            .max(state.slots.iter().map(|slot| slot.views).sum());
        measurements.max_latest_diagnostic_bytes = measurements
            .max_latest_diagnostic_bytes
            .max(state.slots.iter().map(|slot| slot.causes).sum());
    }
    fn observe_owner(&self, index: usize, driver: &Driver) {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        let slot = &mut state.slots[index];
        slot.copies = 2 * witness_bytes(&driver.owner.witness);
        slot.fenced = driver.fenced();
        slot.commands = request_bytes(&driver.owner.request)
            + witness_bytes(&driver.owner.witness)
            + driver
                .owner
                .execution
                .as_ref()
                .map_or(0, witness_complete_bytes);
        slot.views = driver
            .owner
            .view
            .as_ref()
            .map_or(0, |view| view_bytes(view));
        slot.causes = driver
            .owner
            .latest_store_cause
            .iter()
            .chain(driver.owner.latest_delivery_cause.iter())
            .map(|cause| cause.text.capacity())
            .sum();
        Self::update(&mut state);
    }
}

/// Run a finite controlled source over scoped borrowed callbacks. No worker can
/// outlive this call. Returning unresolved owners joins first and leaves the
/// host responsible for retaining its fence/library/DB until actual resolution.
pub fn run(
    configuration: Configuration,
    callbacks: &Callbacks<'_>,
    control: &RunControl,
) -> RunOutcome {
    let shared = Shared {
        state: Mutex::new(State {
            ready: VecDeque::with_capacity(configuration.queue_capacity()),
            slots: (0..configuration.workers)
                .map(|_| Slot::default())
                .collect(),
            source_done: false,
            measurements: Measurements::default(),
            prefetch_bytes: 0,
        }),
        calls: AtomicUsize::new(0),
        max_calls: AtomicUsize::new(0),
    };
    let mut source_failure = None;
    let mut host_failure = None;
    let mut prefetch = None;
    let mut unresolved = Vec::with_capacity(configuration.workers);
    thread::scope(|scope| {
        let mut handles = Vec::with_capacity(configuration.workers);
        for index in 0..configuration.workers {
            let shared = &shared;
            match thread::Builder::new()
                .name(format!("scoped-terminal-{index}"))
                .spawn_scoped(scope, move || worker(index, shared, callbacks, control))
            {
                Ok(handle) => handles.push(handle),
                Err(error) => {
                    host_failure = Some(Cause::new(error));
                    control.return_unresolved();
                    break;
                }
            }
        }
        loop {
            let (_, revision) = control.observe();
            if control.returning() {
                break;
            }
            let (done, room) = {
                let state = shared
                    .state
                    .lock()
                    .unwrap_or_else(|error| error.into_inner());
                (
                    state.source_done,
                    state.ready.len() < configuration.queue_capacity(),
                )
            };
            if !done && !control.stopping() && prefetch.is_none() {
                let next = {
                    let _calling = shared.calling();
                    catch_unwind(AssertUnwindSafe(|| (callbacks.source)()))
                };
                match next {
                    Ok(Some(request)) => {
                        prefetch = Some(request);
                        let mut state = shared
                            .state
                            .lock()
                            .unwrap_or_else(|error| error.into_inner());
                        state.prefetch_bytes = prefetch.as_ref().map_or(0, request_bytes);
                        Shared::update(&mut state);
                    }
                    Ok(None) => {
                        shared
                            .state
                            .lock()
                            .unwrap_or_else(|error| error.into_inner())
                            .source_done = true;
                        control.notify();
                    }
                    Err(_) => {
                        source_failure =
                            Some(Cause::new("controlled source panicked before DB admission"));
                        shared
                            .state
                            .lock()
                            .unwrap_or_else(|error| error.into_inner())
                            .source_done = true;
                        control.stop_intake();
                    }
                }
            }
            if room && let Some(request) = prefetch.take() {
                let mut state = shared
                    .state
                    .lock()
                    .unwrap_or_else(|error| error.into_inner());
                state.ready.push_back(request);
                state.prefetch_bytes = 0;
                Shared::update(&mut state);
                drop(state);
                control.notify();
                continue;
            }
            if control.stopping() {
                let mut state = shared
                    .state
                    .lock()
                    .unwrap_or_else(|error| error.into_inner());
                let changed = !state.source_done;
                state.source_done = true;
                drop(state);
                if changed {
                    control.notify();
                }
            }
            let finished = {
                let state = shared
                    .state
                    .lock()
                    .unwrap_or_else(|error| error.into_inner());
                state.source_done
                    && prefetch.is_none()
                    && state.ready.is_empty()
                    && state.slots.iter().all(|slot| slot.key.is_none())
            };
            if finished {
                break;
            }
            control.wait(revision);
        }
        {
            let mut state = shared
                .state
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            state.source_done = true;
        }
        control.notify();
        for handle in handles {
            match handle.join() {
                Ok(Some(owner)) => unresolved.push(owner),
                Ok(None) => {}
                Err(_) => {
                    host_failure = Some(Cause::new(
                        "worker failed outside controlled callback boundary",
                    ));
                    control.return_unresolved();
                }
            }
        }
    });
    let mut state = shared
        .state
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let mut unstarted: Vec<_> = state.ready.drain(..).collect();
    unstarted.extend(prefetch);
    state.measurements.joined = true;
    state.measurements.max_callback_owners = shared.max_calls.load(Ordering::Acquire);
    RunOutcome {
        unresolved,
        unstarted,
        source_failure_before_db: source_failure,
        host_failure,
        measurements: state.measurements.clone(),
    }
}

fn worker(
    index: usize,
    shared: &Shared,
    callbacks: &Callbacks<'_>,
    control: &RunControl,
) -> Option<Retained> {
    loop {
        let (_, revision) = control.observe();
        if control.returning() {
            return None;
        }
        let request = {
            let mut state = shared
                .state
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let available = state.ready.iter().position(|request| {
                !state.slots.iter().any(|slot| {
                    slot.fenced
                        && slot.key.as_ref().is_some_and(|(generation, id)| {
                            *generation == request.complete.generation
                                && *id == request.complete.operation.identity.id
                        })
                })
            });
            match available {
                Some(position) => {
                    let request = state.ready.remove(position);
                    if let Some(request) = &request {
                        state.slots[index].key = Some((
                            request.complete.generation,
                            request.complete.operation.identity.id.clone(),
                        ));
                        state.slots[index].commands = request_bytes(request);
                    }
                    Shared::update(&mut state);
                    request
                }
                None if state.source_done && state.ready.is_empty() => return None,
                None => None,
            }
        };
        let Some(request) = request else {
            control.wait(revision);
            continue;
        };
        control.notify();
        let (now, _) = control.observe();
        let mut driver = Driver::new(request, now);
        shared.observe_owner(index, &driver);
        if driver.valid_request() {
            let reply = {
                let _calling = shared.calling();
                catch_unwind(AssertUnwindSafe(|| {
                    (callbacks.work)(&driver.owner.request, control)
                }))
            };
            driver.accept_work(reply, control.observe().0);
        } else {
            driver.invalid_request();
            control.return_unresolved();
        }
        shared.observe_owner(index, &driver);
        control.notify();
        loop {
            let (now, revision) = control.observe();
            if control.returning() {
                return Some(driver.owner);
            }
            if driver.ready(now) {
                let _calling = shared.calling();
                if driver.delivery() {
                    if driver.deliver(callbacks.deliver, control) {
                        let mut state = shared
                            .state
                            .lock()
                            .unwrap_or_else(|error| error.into_inner());
                        state.measurements.settled += 1;
                        state.slots[index] = Slot::default();
                        drop(state);
                        control.notify();
                        break;
                    }
                } else {
                    driver.step(callbacks.store, control);
                }
                shared.observe_owner(index, &driver);
                control.notify();
            } else {
                control.wait(revision);
            }
        }
    }
}

fn text(value: &Option<String>) -> usize {
    value.as_ref().map_or(0, String::capacity)
}
fn scope_bytes(scope: &OperationScope) -> usize {
    scope.identity.id.capacity()
        + scope.identity.created_at.capacity()
        + text(&scope.export_scope.discriminator)
        + text(&scope.export_scope.group_ids)
        + text(&scope.export_scope.group_names)
        + text(&scope.destination)
}
fn item_bytes(item: &ItemScope) -> usize {
    item.operation_id.capacity()
        + text(&item.story_uuid)
        + text(&item.story_title)
        + item.source_name.capacity()
        + text(&item.output_name)
        + text(&item.archive_relative_path)
        + text(&item.archive_sha256)
        + text(&item.export_skip_code)
}
fn witness_complete_bytes(command: &CompleteItemCommand) -> usize {
    size_of::<CompleteItemCommand>()
        + scope_bytes(&command.operation)
        + item_bytes(&command.item)
        + text(&command.outcome.code)
        + text(&command.outcome.message)
}
fn finish_bytes(command: &FinishCommand) -> usize {
    size_of::<FinishCommand>()
        + scope_bytes(&command.operation)
        + text(&command.outcome.code)
        + text(&command.outcome.message)
        + command.finished_at.capacity()
}
fn interrupt_bytes(command: &InterruptCommand) -> usize {
    size_of::<InterruptCommand>()
        + scope_bytes(&command.operation)
        + command.code.capacity()
        + command.message.capacity()
        + command.finished_at.capacity()
}
fn request_bytes(request: &Request) -> usize {
    witness_complete_bytes(&request.complete)
        + request.finish.as_ref().map_or(0, finish_bytes)
        + interrupt_bytes(&request.interrupt)
}
fn witness_bytes(witness: &CommandWitness) -> usize {
    size_of::<CommandWitness>()
        + match witness {
            CommandWitness::CompleteItem(command) => witness_complete_bytes(command),
            CommandWitness::Finish(command) => finish_bytes(command),
            CommandWitness::Interrupt(command) => interrupt_bytes(command),
        }
}
fn view_bytes(view: &TerminalView) -> usize {
    size_of::<TerminalView>()
        + view.operation.identity.id.capacity()
        + view.operation.identity.created_at.capacity()
        + text(&view.operation.identity.started_at)
        + text(&view.operation.identity.finished_at)
        + text(&view.operation.export_scope.discriminator)
        + text(&view.operation.export_scope.group_ids)
        + text(&view.operation.export_scope.group_names)
        + text(&view.operation.destination)
        + text(&view.operation.outcome.code)
        + text(&view.operation.outcome.message)
        + view.item.as_ref().map_or(0, |item| {
            size_of::<OperationItem>()
                + item.operation_id.capacity()
                + text(&item.identity.story_uuid)
                + text(&item.story_title)
                + item.source_name.capacity()
                + text(&item.output_name)
                + text(&item.archive_relative_path)
                + text(&item.archive_sha256)
                + text(&item.export_skip_code)
                + text(&item.outcome.code)
                + text(&item.outcome.message)
        })
}

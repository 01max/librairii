//! Library command admission and actual reader/artwork drain, separate from preferences.
use std::{
    fmt,
    sync::{Arc, Condvar, Mutex},
};

#[derive(Clone, Debug)]
pub struct LibraryGeneration(Arc<()>);
impl PartialEq for LibraryGeneration {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for LibraryGeneration {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Phase {
    Active,
    Draining,
    Recovery,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifecycleError {
    Busy,
    RecoveryRequired,
}
impl fmt::Display for LifecycleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Busy => "settings.busy",
            Self::RecoveryRequired => "settings.library_missing",
        })
    }
}
impl std::error::Error for LifecycleError {}
#[derive(Debug)]
struct State {
    phase: Phase,
    uses: usize,
    mutators: usize,
    generation: LibraryGeneration,
}
#[derive(Debug)]
pub struct LifecycleGate {
    state: Mutex<State>,
    drained: Condvar,
}
impl Default for LifecycleGate {
    fn default() -> Self {
        Self::new()
    }
}
impl LifecycleGate {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(State {
                phase: Phase::Active,
                uses: 0,
                mutators: 0,
                generation: LibraryGeneration(Arc::new(())),
            }),
            drained: Condvar::new(),
        }
    }
    pub fn phase(&self) -> Result<Phase, LifecycleError> {
        self.state
            .lock()
            .map(|state| state.phase)
            .map_err(|_| LifecycleError::RecoveryRequired)
    }
    pub fn enter(&self) -> Result<LibraryUse<'_>, LifecycleError> {
        self.enter_kind(false)
    }
    /// Mutation-capable callbacks are admitted separately from drainable
    /// readers/artwork. A concurrent move must lose immediately to a mutator.
    pub fn enter_mutation(&self) -> Result<LibraryUse<'_>, LifecycleError> {
        self.enter_kind(true)
    }
    fn enter_kind(&self, mutation: bool) -> Result<LibraryUse<'_>, LifecycleError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| LifecycleError::RecoveryRequired)?;
        if state.phase != Phase::Active {
            return Err(LifecycleError::Busy);
        }
        let uses = state.uses.checked_add(1).ok_or(LifecycleError::Busy)?;
        let mutators = state
            .mutators
            .checked_add(usize::from(mutation))
            .ok_or(LifecycleError::Busy)?;
        state.uses = uses;
        state.mutators = mutators;
        Ok(LibraryUse {
            gate: self,
            generation: state.generation.clone(),
            mutation,
        })
    }
    pub fn accepts(&self, generation: &LibraryGeneration) -> bool {
        self.state
            .lock()
            .is_ok_and(|state| state.phase == Phase::Active && state.generation == *generation)
    }
    /// Refuse admitted mutators, then stop intake before draining readers.
    /// Idle queries run before and after drain, outside the mutex with intake closed.
    /// Errors are not mistaken for idle: callers must return false on read failure.
    pub fn begin_switch(
        &self,
        mut operations_idle: impl FnMut() -> bool,
    ) -> Result<Switch<'_>, LifecycleError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| LifecycleError::RecoveryRequired)?;
        if state.phase != Phase::Active || state.mutators != 0 {
            return Err(LifecycleError::Busy);
        }
        state.phase = Phase::Draining;
        drop(state);
        let guard = Switch {
            gate: self,
            settled: false,
        };
        if !operations_idle() {
            guard.restore_unchanged()?;
            return Err(LifecycleError::Busy);
        }
        let mut state = self
            .state
            .lock()
            .map_err(|_| LifecycleError::RecoveryRequired)?;
        while state.uses != 0 {
            state = self
                .drained
                .wait(state)
                .map_err(|_| LifecycleError::RecoveryRequired)?;
        }
        drop(state);
        if !operations_idle() {
            guard.restore_unchanged()?;
            return Err(LifecycleError::Busy);
        }
        Ok(guard)
    }
    /// Recovery orchestration still needs a real verified storage owner before
    /// settling this gate. Phase data itself never opens or mutates a library.
    pub fn begin_recovery(&self) -> Result<Switch<'_>, LifecycleError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| LifecycleError::RecoveryRequired)?;
        if state.phase != Phase::Recovery || state.uses != 0 {
            return Err(LifecycleError::Busy);
        }
        state.phase = Phase::Draining;
        Ok(Switch {
            gate: self,
            settled: false,
        })
    }
}

#[derive(Debug)]
pub struct LibraryUse<'gate> {
    gate: &'gate LifecycleGate,
    generation: LibraryGeneration,
    mutation: bool,
}
impl LibraryUse<'_> {
    pub fn generation(&self) -> &LibraryGeneration {
        &self.generation
    }
}
impl Drop for LibraryUse<'_> {
    fn drop(&mut self) {
        let mut state = self
            .gate
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // A use is move-only; exactly one decrement corresponds to each entry.
        state.uses = state.uses.saturating_sub(1);
        state.mutators = state.mutators.saturating_sub(usize::from(self.mutation));
        self.gate.drained.notify_all();
    }
}

/// Held across DB/logger close, copying, profile commitment and reconstruction.
/// Unsettled Drop fences the facade into recovery; it never silently resumes a
/// potentially closed or partially replaced root after a failed move.
#[derive(Debug)]
pub struct Switch<'gate> {
    gate: &'gate LifecycleGate,
    settled: bool,
}
impl Switch<'_> {
    fn restore_unchanged(mut self) -> Result<(), LifecycleError> {
        let mut state = self
            .gate
            .state
            .lock()
            .map_err(|_| LifecycleError::RecoveryRequired)?;
        state.phase = Phase::Active;
        self.settled = true;
        Ok(())
    }
    /// Called only after the composition root reconstructs real services. A
    /// resumed source also gets a new generation; old query/preparation results
    /// cannot become current merely because the visible path stayed the same.
    pub fn activated(mut self) -> Result<LibraryGeneration, LifecycleError> {
        let mut state = self
            .gate
            .state
            .lock()
            .map_err(|_| LifecycleError::RecoveryRequired)?;
        state.generation = LibraryGeneration(Arc::new(()));
        state.phase = Phase::Active;
        self.settled = true;
        Ok(state.generation.clone())
    }
}
impl Drop for Switch<'_> {
    fn drop(&mut self) {
        if !self.settled {
            self.gate
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .phase = Phase::Recovery;
            self.gate.drained.notify_all();
        }
    }
}

use librairii_app::lifecycle::{LifecycleError, LifecycleGate, Phase};
use std::{
    error::Error,
    sync::mpsc,
    time::{Duration, Instant},
};

#[test]
fn lifecycle_exclusion_waits_for_actual_uses_and_rejects_old_generations()
-> Result<(), Box<dyn Error>> {
    let gate = LifecycleGate::new();
    let active = gate.enter()?;
    let old = active.generation().clone();
    let (completed, result) = mpsc::sync_channel(1);
    std::thread::scope(|scope| -> Result<(), Box<dyn Error>> {
        let gate_ref = &gate;
        let worker = scope.spawn(move || -> Result<(), LifecycleError> {
            let exclusive = gate_ref.begin_switch(|| true)?;
            completed
                .send(())
                .map_err(|_| LifecycleError::RecoveryRequired)?;
            exclusive.activated()?;
            Ok(())
        });
        let deadline = Instant::now() + Duration::from_secs(5);
        while gate.phase()? == Phase::Active && Instant::now() < deadline {
            std::thread::yield_now();
        }
        assert_eq!(gate.phase()?, Phase::Draining);
        assert!(gate.enter().is_err());
        assert!(!gate.accepts(&old));
        assert!(result.try_recv().is_err());
        drop(active);
        result.recv_timeout(Duration::from_secs(5))?;
        worker.join().map_err(|_| "lifecycle worker panicked")??;
        Ok(())
    })?;
    let current = gate.enter()?;
    assert_ne!(&old, current.generation());
    assert!(gate.accepts(current.generation()));
    assert!(!gate.accepts(&old));
    Ok(())
}

#[test]
fn busy_operations_keep_original_generation_and_unsettled_switches_require_recovery()
-> Result<(), Box<dyn Error>> {
    let gate = LifecycleGate::new();
    let original = gate.enter()?.generation().clone();
    assert!(matches!(
        gate.begin_switch(|| false),
        Err(LifecycleError::Busy)
    ));
    assert!(gate.accepts(&original));
    drop(gate.begin_switch(|| true)?);
    assert_eq!(gate.phase()?, Phase::Recovery);
    assert!(gate.enter().is_err());
    let recovery = gate.begin_recovery()?;
    recovery.activated()?;
    assert!(!gate.accepts(&original));
    assert_eq!(gate.phase()?, Phase::Active);
    Ok(())
}

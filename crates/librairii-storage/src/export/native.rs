//! Prepared plans join the caller's one native job runtime. The registry retains
//! actual destination witnesses; UI previews and persisted labels grant no copy
//! authority. The existing durable copier owns every temporary through reporting.
use super::{
    Exports, MAX_PREPARED, MAX_PREPARED_BYTES,
    durable::{CopyExecutor, ExportResources, operation_input},
    preparation::Prepared,
};
use crate::{Result, StorageError, database::Database, platform::unix::LibraryFiles};
use librairii_app::{
    native_jobs::{
        Boundary, Cancellation, DurableReport, Execution, Executor, Handle, ProgressSink, Report,
        Start,
    },
    ports::jobs::WorkItem,
};
use librairii_core::{
    OperationKind,
    export::{ExportError, PreparationId},
    operations::{ItemState, RawOutcome},
};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

#[derive(Debug)]
pub struct ExportExecutor<'files, 'library> {
    database: &'files Database<'library>,
    files: &'files LibraryFiles<'library>,
    plans: Mutex<BTreeMap<String, Arc<Prepared>>>,
}
impl<'files, 'library> ExportExecutor<'files, 'library> {
    pub fn new(
        database: &'files Database<'library>,
        files: &'files LibraryFiles<'library>,
    ) -> Self {
        Self {
            database,
            files,
            plans: Mutex::default(),
        }
    }
    fn prune(&self) {
        let ids: Vec<_> = self
            .plans
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .keys()
            .cloned()
            .collect();
        for id in ids {
            let key = id.clone();
            let terminal = self.database.barrier_read(move |tx| {
                use rusqlite::OptionalExtension;
                Ok(tx.query_row("SELECT status NOT IN ('queued','running') FROM file_operations WHERE id=?", [key], |r| r.get::<_, bool>(0)).optional()?.unwrap_or(false))
            }).and_then(|p| p.wait()).unwrap_or(false);
            if terminal {
                self.plans
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .remove(&id);
            }
        }
    }
    pub fn reject_admission(&self, id: &str) {
        // Only the actual scheduler's rejected-admission receipt calls this.
        self.plans
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(id);
    }
    pub fn offer(
        &self,
        handle: &Handle,
        exports: &mut Exports<'_, '_>,
        preparation: &PreparationId,
        operation: String,
    ) -> Result<()> {
        self.prune();
        let mut plans = self.plans.lock().map_err(|_| StorageError::RuntimeFenced)?;
        let preview = exports
            .prepared
            .iter()
            .find(|p| p.id.same(preparation))
            .ok_or(ExportError::Stale)?;
        if plans.len() >= MAX_PREPARED
            || plans.contains_key(&operation)
            || plans
                .values()
                .map(|p| p.bytes)
                .sum::<usize>()
                .saturating_add(preview.bytes)
                > MAX_PREPARED_BYTES
        {
            return Err(ExportError::ResourceLimit.into());
        }
        let plan = exports.take_prepared(preparation, &Default::default())?;
        let input = operation_input(&plan, operation.clone())?;
        let start = Start::new(input).map_err(|_| StorageError::CompatibilityInvariant)?;
        plans.insert(operation.clone(), Arc::new(plan));
        if handle.try_start(start).is_err() {
            let plan = plans
                .remove(&operation)
                .ok_or(StorageError::CompatibilityInvariant)?;
            // Refused ingress cannot have handed this plan to an executor.
            exports.prepared.push_back(
                Arc::try_unwrap(plan).map_err(|_| StorageError::CompatibilityInvariant)?,
            );
            return Err(StorageError::QueueFull);
        }
        Ok(())
    }
}
impl<'files, 'library> Executor for ExportExecutor<'files, 'library> {
    type Resources = ExportResources<'files, 'library>;
    fn execute(
        &self,
        work: &WorkItem,
        cancel: &Cancellation,
        progress: &ProgressSink,
    ) -> Execution<Self::Resources> {
        let plan = self
            .plans
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&work.operation.identity.id)
            .cloned();
        let Some(plan) = plan.filter(|_| work.operation.kind == OperationKind::Export) else {
            return Execution {
                boundary: Boundary::BeforeCommit,
                resources: None,
                report: Report {
                    state: ItemState::Failed,
                    story_id: work.item.identity.historical_story_id(),
                    completed_bytes: 0,
                    outcome: RawOutcome {
                        code: Some("export_failed".into()),
                        message: Some("The story archive could not be exported.".into()),
                    },
                },
            };
        };
        CopyExecutor {
            database: self.database,
            plan: &plan,
            files: self.files,
            operation: work.operation.identity.id.clone(),
        }
        .execute(work, cancel, progress)
    }
    fn compensate(&self, resources: &mut Self::Resources) -> bool {
        resources.retire()
    }
    fn settle(
        &self,
        mut resources: Self::Resources,
        _: &DurableReport<'_>,
    ) -> std::result::Result<(), Self::Resources> {
        if resources.retire() {
            Ok(())
        } else {
            Err(resources)
        }
    }
}

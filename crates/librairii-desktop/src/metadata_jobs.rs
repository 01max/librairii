//! Explicit metadata jobs share the import scheduler and retain the actual
//! metadata owner through durable reporting, including uncertain file effects.
use crate::imports::ImportExecutor;
use librairii_adapters::lunii::CatalogClient;
use librairii_app::{
    metadata::{RefreshCode, RefreshSession},
    native_jobs::{
        Boundary, Cancellation, DurableReport, Execution, Executor, ProgressSink, Report,
    },
    ports::jobs::WorkItem,
};
use librairii_core::{
    OperationKind,
    operations::{ItemState, RawOutcome},
};
use librairii_storage::{database::Database, imports::ImportResources, metadata::MetadataStorage};
use std::sync::Arc;
#[derive(Debug)]
pub struct MetadataExecutor<'db, 'library> {
    database: &'db Database<'library>,
    client: Option<CatalogClient>,
}
#[derive(Debug)]
pub struct MetadataResources<'db, 'library> {
    store: Arc<MetadataStorage<'db, 'library>>,
    operation: String,
    item: i64,
    expected: (ItemState, RawOutcome),
}
impl<'db, 'library> MetadataExecutor<'db, 'library> {
    pub fn new(database: &'db Database<'library>, client: Option<CatalogClient>) -> Self {
        Self { database, client }
    }
    fn run(
        &self,
        store: &MetadataStorage<'db, 'library>,
        work: &WorkItem,
        cancel: &Cancellation,
        progress: &ProgressSink,
    ) -> std::result::Result<librairii_app::metadata::RefreshReport, RefreshCode> {
        if work.item.source_name != "en-GB" {
            return Err(RefreshCode::InvalidCatalog);
        }
        let id = work
            .operation
            .identity
            .id
            .parse()
            .map_err(|_| RefreshCode::InvalidCatalog)?;
        let at = librairii_adapters::datetime::utc_now().ok_or(RefreshCode::PersistenceFailed)?;
        let session = RefreshSession::begin(store, id, "en-GB", &at)?;
        let client = self
            .client
            .clone()
            .map(Ok)
            .unwrap_or_else(CatalogClient::production);
        let payload = client.and_then(|client| client.fetch_for_job(cancel));
        let at = librairii_adapters::datetime::utc_now().unwrap_or(at);
        session.finish_with_activation(
            payload,
            &at,
            || cancel.is_cancelled(),
            || {
                let _phase = progress.phase(librairii_app::native_jobs::Phase::Committing);
            },
        )
    }
}
impl<'db, 'library> Executor for MetadataExecutor<'db, 'library> {
    type Resources = Box<MetadataResources<'db, 'library>>;
    fn execute(
        &self,
        work: &WorkItem,
        cancel: &Cancellation,
        progress: &ProgressSink,
    ) -> Execution<Self::Resources> {
        let _phase = progress.phase(librairii_app::native_jobs::Phase::Running);
        let store = Arc::new(MetadataStorage::for_job(self.database, cancel));
        let result = self.run(store.as_ref(), work, cancel, progress);
        let uncertain = result
            .as_ref()
            .is_err_and(|code| *code == RefreshCode::OutcomeUncertain)
            || !store.retained_failures().is_empty();
        let (state, code, message) = match result {
            Ok(report) => (
                ItemState::Succeeded,
                "metadata_refreshed",
                format!(
                    "Official metadata refreshed; {} local stories matched.",
                    report.matched_stories
                ),
            ),
            Err(code) => (
                if code == RefreshCode::Cancelled {
                    ItemState::Cancelled
                } else {
                    ItemState::Failed
                },
                code.as_str(),
                match code {
                    RefreshCode::FetchFailed => "Official metadata could not be downloaded.",
                    RefreshCode::InvalidCatalog => "The downloaded official catalog was invalid.",
                    RefreshCode::StageFailed => "Official metadata could not be staged safely.",
                    RefreshCode::Cancelled => "Official metadata refresh was cancelled.",
                    _ => "Official metadata could not be saved.",
                }
                .into(),
            ),
        };
        let outcome = RawOutcome {
            code: Some(code.into()),
            message: Some(message),
        };
        Execution {
            boundary: if uncertain {
                Boundary::Compensating
            } else if state == ItemState::Succeeded {
                Boundary::Committed
            } else {
                Boundary::BeforeCommit
            },
            report: Report {
                state,
                story_id: 0,
                completed_bytes: 0,
                outcome: outcome.clone(),
            },
            resources: Some(Box::new(MetadataResources {
                store,
                operation: work.operation.identity.id.clone(),
                item: work.item.identity.id,
                expected: (state, outcome),
            })),
        }
    }
    fn compensate(&self, _resources: &mut Self::Resources) -> bool {
        // An uncertain publication retains its original metadata owner. A
        // cancellation flag or DTO cannot certify compensation or clean close.
        false
    }
    fn settle(
        &self,
        resources: Self::Resources,
        durable: &DurableReport<'_>,
    ) -> std::result::Result<(), Self::Resources> {
        let command = durable.command();
        if command.operation.identity.id != resources.operation
            || command.item.id != resources.item
            || (command.state, command.outcome.clone()) != resources.expected
            || !resources.store.retained_failures().is_empty()
        {
            return Err(resources);
        }
        Ok(())
    }
}
#[derive(Debug)]
pub struct Executors<'a, 'db, 'library> {
    pub imports: &'a ImportExecutor<'db, 'library>,
    pub metadata: &'a MetadataExecutor<'db, 'library>,
}
#[derive(Debug)]
pub enum Resources<'db, 'library> {
    Import(Box<ImportResources<'db>>),
    Metadata(Box<MetadataResources<'db, 'library>>),
}
impl<'db, 'library> Executor for Executors<'_, 'db, 'library> {
    type Resources = Resources<'db, 'library>;
    fn execute(
        &self,
        work: &WorkItem,
        cancel: &Cancellation,
        progress: &ProgressSink,
    ) -> Execution<Self::Resources> {
        match work.operation.kind {
            OperationKind::Import => {
                let e = self.imports.execute(work, cancel, progress);
                Execution {
                    boundary: e.boundary,
                    report: e.report,
                    resources: e.resources.map(Resources::Import),
                }
            }
            OperationKind::MetadataSync => {
                let e = self.metadata.execute(work, cancel, progress);
                Execution {
                    boundary: e.boundary,
                    report: e.report,
                    resources: e.resources.map(Resources::Metadata),
                }
            }
            OperationKind::Export => Execution {
                boundary: Boundary::BeforeCommit,
                report: Report {
                    state: ItemState::Failed,
                    story_id: 0,
                    completed_bytes: 0,
                    outcome: RawOutcome {
                        code: Some("export_unavailable".into()),
                        message: Some("The action could not be completed".into()),
                    },
                },
                resources: None,
            },
        }
    }
    fn compensate(&self, resources: &mut Self::Resources) -> bool {
        match resources {
            Resources::Import(r) => self.imports.compensate(r),
            Resources::Metadata(r) => self.metadata.compensate(r),
        }
    }
    fn settle(
        &self,
        resources: Self::Resources,
        durable: &DurableReport<'_>,
    ) -> std::result::Result<(), Self::Resources> {
        match resources {
            Resources::Import(r) => self.imports.settle(r, durable).map_err(Resources::Import),
            Resources::Metadata(r) => self
                .metadata
                .settle(r, durable)
                .map_err(Resources::Metadata),
        }
    }
}

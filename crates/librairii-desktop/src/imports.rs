//! Prepared picker inputs and the real archive/storage executor for native jobs.
use librairii_app::importing::{JobId, OperationKind, OperationState};
use librairii_app::{
    native_jobs::{Cancellation, DurableReport, Execution, Executor, Handle, ProgressSink, Start},
    ports::{
        creation::{MAX_COMMAND_BYTES, NewItem, NewOperation},
        jobs::WorkItem,
    },
};
use librairii_storage::{
    database::Database,
    imports::{ImportObserver, ImportResources, ImportStorage},
    platform::unix::PickedSource,
};
use std::{collections::BTreeMap, fmt, path::PathBuf, sync::Mutex};
const MAX_BATCHES: usize = 8;
const SLOT_PREFIX: &str = "native-import/";
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrepareError {
    Invalid,
    SourceUnavailable,
    Busy,
}
impl fmt::Display for PrepareError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Invalid => "import.invalid_request",
            Self::SourceUnavailable => "import.source_unavailable",
            Self::Busy => "import.busy",
        })
    }
}
impl std::error::Error for PrepareError {}
#[derive(Debug)]
pub struct PreparedImport {
    start: Start,
    paths: Vec<PathBuf>,
    bytes: usize,
}
impl PreparedImport {
    /// Run on an admission worker after an explicit picker selection. Only
    /// paths/labels are retained; source FDs stay bounded by executing workers.
    pub fn new(id: JobId, paths: Vec<PathBuf>) -> Result<Self, PrepareError> {
        if !(1..=1000).contains(&paths.len()) {
            return Err(PrepareError::Invalid);
        }
        let mut bytes = paths
            .capacity()
            .checked_mul(std::mem::size_of::<PathBuf>())
            .ok_or(PrepareError::Invalid)?;
        let mut items = Vec::with_capacity(paths.len());
        for (index, path) in paths.iter().enumerate() {
            bytes = bytes
                .checked_add(path.capacity())
                .ok_or(PrepareError::Invalid)?;
            if bytes > MAX_COMMAND_BYTES {
                return Err(PrepareError::Invalid);
            }
            let name = path.file_name().ok_or(PrepareError::SourceUnavailable)?;
            let total = PickedSource::open(path).map(|s| s.bytes()).unwrap_or(0);
            items.push(NewItem {
                source_name: name.to_string_lossy().into_owned(),
                archive_relative_path: format!("{SLOT_PREFIX}{index}"),
                total_bytes: i64::try_from(total).map_err(|_| PrepareError::Invalid)?,
                ..Default::default()
            });
        }
        let start = Start::new(NewOperation {
            id: id.to_string(),
            kind: OperationKind::Import,
            source: None,
            destination: None,
            items,
        })
        .map_err(|_| PrepareError::Invalid)?;
        bytes = bytes
            .checked_add(
                start
                    .input()
                    .retained_bytes()
                    .map_err(|_| PrepareError::Invalid)?,
            )
            .ok_or(PrepareError::Invalid)?;
        if bytes > MAX_COMMAND_BYTES {
            return Err(PrepareError::Invalid);
        }
        Ok(Self {
            start,
            paths,
            bytes,
        })
    }
    pub fn operation_id(&self) -> &str {
        &self.start.input().id
    }
}
#[derive(Debug)]
struct Batch {
    paths: Vec<Option<PathBuf>>,
    items: Vec<NewItem>,
    bytes: usize,
}
#[derive(Debug)]
pub struct ImportExecutor<'db, 'library> {
    store: ImportStorage<'db, 'library>,
    database: &'db Database<'library>,
    batches: Mutex<BTreeMap<String, Batch>>,
}
impl<'db, 'library> ImportExecutor<'db, 'library> {
    pub fn observed(database: &'db Database<'library>, observer: &'db dyn ImportObserver) -> Self {
        Self {
            store: ImportStorage::observed(database, observer),
            database,
            batches: Mutex::new(BTreeMap::new()),
        }
    }
    pub fn new(database: &'db Database<'library>) -> Self {
        Self {
            store: ImportStorage::new(database),
            database,
            batches: Mutex::new(BTreeMap::new()),
        }
    }
    /// The same move-only preparation is returned on refusal. Opaque slot
    /// strings in immutable import item data identify this memory table; they
    /// are never interpreted as paths or restored as authority after restart.
    pub fn offer(
        &self,
        handle: &Handle,
        prepared: PreparedImport,
    ) -> Result<(), (PrepareError, PreparedImport)> {
        self.prune();
        let mut batches = self
            .batches
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let id = prepared.start.input().id.clone();
        let retained = batches.values().map(|b| b.bytes).sum::<usize>();
        if batches.len() >= MAX_BATCHES
            || retained.saturating_add(prepared.bytes) > MAX_COMMAND_BYTES
            || batches.contains_key(&id)
        {
            return Err((PrepareError::Busy, prepared));
        }
        let PreparedImport {
            start,
            paths,
            bytes,
        } = prepared;
        let items = start.input().items.clone();
        batches.insert(
            id.clone(),
            Batch {
                paths: paths.into_iter().map(Some).collect(),
                items,
                bytes,
            },
        );
        match handle.try_start(start) {
            Ok(()) => Ok(()),
            Err((_, start)) => {
                let batch = batches.remove(&id);
                let paths = batch
                    .map(|b| b.paths.into_iter().flatten().collect())
                    .unwrap_or_default();
                Err((
                    PrepareError::Busy,
                    PreparedImport {
                        start,
                        paths,
                        bytes,
                    },
                ))
            }
        }
    }
    fn prune(&self) {
        let ids: Vec<_> = self
            .batches
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .keys()
            .cloned()
            .collect();
        for id in ids {
            if matches!(self.database.operation_snapshot(id.clone()).and_then(|p|p.wait()),Ok(Some(s)) if !matches!(s.operation.state, OperationState::Queued | OperationState::Running))
            {
                self.batches
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .remove(&id);
            }
        }
    }
    fn source(&self, work: &WorkItem) -> Option<PathBuf> {
        let token = work.item.archive_relative_path.as_deref()?;
        let index: usize = token.strip_prefix(SLOT_PREFIX)?.parse().ok()?;
        if token != format!("{SLOT_PREFIX}{index}") {
            return None;
        }
        let mut batches = self
            .batches
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let batch = batches.get_mut(&work.operation.identity.id)?;
        let item = batch.items.get(index)?;
        if item.source_name != work.item.source_name || item.total_bytes != work.item.progress.total
        {
            return None;
        }
        batch.paths.get_mut(index)?.take()
    }
}
impl<'db> Executor for ImportExecutor<'db, '_> {
    type Resources = Box<ImportResources<'db>>;
    fn execute(
        &self,
        work: &WorkItem,
        cancel: &Cancellation,
        progress: &ProgressSink,
    ) -> Execution<Self::Resources> {
        match self.source(work) {
            Some(path) => self.store.execute(
                work,
                &path,
                &librairii_adapters::archives::ArchiveInspector,
                cancel,
                progress,
            ),
            None => Execution {
                boundary: librairii_app::native_jobs::Boundary::BeforeCommit,
                report: librairii_app::native_jobs::Report {
                    state: librairii_app::native_jobs::ItemState::Failed,
                    story_id: 0,
                    completed_bytes: 0,
                    outcome: librairii_app::native_jobs::RawOutcome {
                        code: Some("stage_failed".into()),
                        message: Some("The file could not be imported.".into()),
                    },
                },
                resources: None,
            },
        }
    }
    fn compensate(&self, r: &mut Self::Resources) -> bool {
        self.store.compensate(r)
    }
    fn settle(
        &self,
        r: Self::Resources,
        durable: &DurableReport<'_>,
    ) -> Result<(), Self::Resources> {
        self.store.settle(r, durable)
    }
}

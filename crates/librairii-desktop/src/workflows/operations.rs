//! Bounded observations of the one shared native job runtime. Picker paths are
//! session inputs; persisted labels and reports never grant source-file access.
use crate::{imports::PreparedImport, library::Runtime};
use librairii_app::{
    lifecycle::LibraryGeneration,
    native_jobs::{ProgressUpdate, RuntimeStatus},
};
use librairii_core::{OperationKind, OperationState, operations::is_terminal};
use librairii_storage::{
    Result, StorageError,
    operations_repository::{OperationCard, OperationItems},
};
use std::{
    collections::{BTreeMap, VecDeque},
    path::PathBuf,
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Action {
    Sync,
    HideSync(bool),
    Import(Vec<PathBuf>),
    Cancel(String),
    Inspect { id: String, page: usize },
    Refresh,
}
impl Action {
    pub fn bounded(&self) -> bool {
        match self {
            Self::Import(paths) => {
                (1..=1000).contains(&paths.len())
                    && paths
                        .iter()
                        .try_fold(0usize, |bytes, p| bytes.checked_add(p.as_os_str().len()))
                        .is_some_and(|bytes| bytes <= 1024 * 1024)
            }
            Self::Cancel(id) | Self::Inspect { id, .. } => !id.is_empty() && id.len() <= 256,
            Self::Refresh | Self::Sync | Self::HideSync(_) => true,
        }
    }
}
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub critical: bool,
    pub catalog_revision: Option<librairii_core::CatalogSnapshotId>,
    pub sync_hidden: bool,
    pub sync_preparing: bool,
    pub preference_error: bool,
    pub catalog: Option<librairii_core::metadata::CatalogStatus>,
    pub last_success_local: Option<librairii_adapters::datetime::LocalDateTime>,
    pub recovery_required: bool,
    pub revision: u64,
    pub observation: u64,
    pub effects_revision: u64,
    pub cards: Vec<OperationCard>,
    pub details: Option<OperationItems>,
    pub preparing: Option<String>,
    pub error: Option<String>,
    pub status: Option<RuntimeStatus>,
    pub progress: Vec<ProgressUpdate>,
}
#[derive(Debug, Default)]
pub struct Controller {
    preferences: Option<librairii_storage::profile::InterfacePreferences>,
    pub snapshot: Snapshot,
    recent: VecDeque<String>,
    selected: Option<(String, usize)>,
    succeeded: BTreeMap<String, i64>,
    last: Option<Instant>,
    dirty: bool,
    initialized: bool,
}
impl Controller {
    pub fn restore_preferences(
        &mut self,
        store: &librairii_storage::profile::NativeProfileStore,
        library: librairii_core::LibraryId,
    ) -> Result<()> {
        match store.load_interface(library) {
            Ok(value) => {
                let value = value.unwrap_or_else(|| {
                    librairii_storage::profile::InterfacePreferences::new(library)
                });
                self.snapshot.sync_hidden = value.sync_panel_hidden;
                self.preferences = Some(value);
            }
            Err(StorageError::InvalidNavigation | StorageError::InvalidProfile) => {
                self.snapshot.preference_error = true;
                self.preferences = None;
            }
            Err(error) => return Err(error),
        }
        Ok(())
    }
    pub fn handle(
        &mut self,
        runtime: &Runtime<'_, '_, '_>,
        store: &librairii_storage::profile::NativeProfileStore,
        generation: LibraryGeneration,
        action: Action,
        revision: u64,
    ) -> bool {
        self.snapshot.revision = revision;
        self.snapshot.error = None;
        let mut effects = false;
        let result = (|| -> Result<()> {
            if !runtime.accepts(&generation) || !action.bounded() {
                return Err(StorageError::SnapshotChanged);
            }
            // Drain the previous admission receipt before offering another job.
            effects |= self.poll(runtime, true)?.1;
            match action {
                Action::Sync => {
                    if self.snapshot.sync_preparing
                        || self.snapshot.cards.iter().any(|c| {
                            c.operation.kind == OperationKind::MetadataSync
                                && !is_terminal(c.operation.state)
                        })
                    {
                        return Err(StorageError::QueueFull);
                    }
                    let id = librairii_adapters::identities::job_id()
                        .map_err(|source| StorageError::Io {
                            operation: "create metadata job identity",
                            source,
                        })?
                        .to_string();
                    let start = librairii_app::native_jobs::Start::new(
                        librairii_app::ports::creation::NewOperation {
                            id: id.clone(),
                            kind: OperationKind::MetadataSync,
                            source: None,
                            destination: None,
                            items: vec![librairii_app::ports::creation::NewItem {
                                source_name: "en-GB".into(),
                                ..Default::default()
                            }],
                        },
                    )
                    .map_err(|_| StorageError::CompatibilityInvariant)?;
                    runtime.offer_metadata(start)?;
                    self.recent.push_front(id.clone());
                    self.recent.truncate(8);
                    self.snapshot.preparing = Some(id.clone());
                    self.snapshot.sync_preparing = true;
                    self.snapshot.details = None;
                    self.selected = Some((id, 0));
                }
                Action::HideSync(hidden) => {
                    if let Some(preferences) = &self.preferences {
                        let mut next = preferences.clone();
                        next.sync_panel_hidden = hidden;
                        if let Err(error) = store.save_interface(&next) {
                            self.snapshot.preference_error = true;
                            self.snapshot.recovery_required |=
                                matches!(store.load(), Err(StorageError::ProfileRestartRequired));
                            return Err(error);
                        }
                        self.preferences = Some(next);
                        self.snapshot.preference_error = false;
                    }
                    self.snapshot.sync_hidden = hidden;
                }
                Action::Import(paths) => {
                    let id = librairii_adapters::identities::job_id().map_err(|error| {
                        StorageError::Io {
                            operation: "generate operation identity",
                            source: error,
                        }
                    })?;
                    let prepared = PreparedImport::new(id, paths)
                        .map_err(|_| StorageError::CompatibilityInvariant)?;
                    let id = prepared.operation_id().to_owned();
                    runtime
                        .offer_import(prepared)
                        .map_err(|_| StorageError::QueueFull)?;
                    self.recent.retain(|old| old != &id);
                    self.recent.push_front(id.clone());
                    self.recent.truncate(8);
                    self.snapshot.preparing = Some(id.clone());
                    self.snapshot.details = None;
                    self.selected = Some((id, 0));
                }
                Action::Cancel(id) => {
                    if !self
                        .snapshot
                        .cards
                        .iter()
                        .any(|c| c.operation.identity.id == id && !is_terminal(c.operation.state))
                    {
                        return Err(StorageError::SnapshotChanged);
                    }
                    runtime.cancel_operation(id)?;
                }
                Action::Inspect { id, page } => {
                    // Only an observed operation can be selected from the feed.
                    if !self
                        .snapshot
                        .cards
                        .iter()
                        .any(|c| c.operation.identity.id == id)
                    {
                        return Err(StorageError::SnapshotChanged);
                    }
                    self.selected = Some((id, page));
                }
                Action::Refresh => {}
            }
            self.dirty = true;
            effects |= self.poll(runtime, true)?.1;
            Ok(())
        })();
        if let Err(error) = result {
            self.snapshot.recovery_required |= matches!(
                error,
                StorageError::RuntimeFenced
                    | StorageError::CustodyRecoveryRequired
                    | StorageError::ProfileRestartRequired
            );
            self.snapshot.error = Some(error.to_string());
        }
        effects
    }
    /// Returns (presentation changed, successful import effects changed).
    /// Progress never enters the command FIFO or creates another job runtime.
    pub fn poll(&mut self, runtime: &Runtime<'_, '_, '_>, force: bool) -> Result<(bool, bool)> {
        let observed = runtime.observe_jobs();
        if observed.status.failed {
            return Err(StorageError::RuntimeFenced);
        }
        self.dirty |= observed.dirty
            || !observed.progress.is_empty()
            || self.snapshot.critical != observed.critical
            || self.snapshot.status != Some(observed.status);
        self.snapshot.status = Some(observed.status);
        self.snapshot.critical = observed.critical;
        for receipt in observed.admissions {
            if self.snapshot.preparing.as_ref() == Some(&receipt.id) {
                self.snapshot.preparing = None;
                self.snapshot.sync_preparing = false;
                if !receipt.accepted {
                    self.snapshot.error = Some("operation.admission_failed".into());
                }
            }
            self.dirty = true;
        }
        for update in observed.progress {
            self.snapshot.progress.retain(|p| p.item != update.item);
            self.snapshot.progress.push(update);
        }
        if !force
            && (!self.dirty
                || self
                    .last
                    .is_some_and(|at| at.elapsed() < Duration::from_millis(100)))
        {
            return Ok((false, false));
        }
        let cards = runtime.command(|db, _, _| {
            db.native_operation_cards(self.recent.iter().cloned().collect())?
                .wait()
        })?;
        let (catalog, catalog_revision) = runtime.command(|db, _, _| {
            db.native_catalog_status(librairii_core::metadata::Locale::parse("en-GB")?)?
                .wait()
        })?;
        let mut effects = self.initialized
            && catalog_revision.is_some()
            && self.snapshot.catalog_revision != catalog_revision;
        self.snapshot.catalog_revision = catalog_revision;
        self.snapshot.last_success_local = catalog
            .activated_at
            .as_ref()
            .and_then(|at| librairii_adapters::datetime::local(at));
        self.snapshot.catalog = Some(catalog);
        let mut succeeded = BTreeMap::new();
        for card in &cards {
            if card.operation.kind == OperationKind::Import {
                effects |= self.initialized
                    && card.succeeded
                        > self
                            .succeeded
                            .get(&card.operation.identity.id)
                            .copied()
                            .unwrap_or(0);
                succeeded.insert(card.operation.identity.id.clone(), card.succeeded);
            }
        }
        self.succeeded = succeeded;
        self.snapshot.observation = self
            .snapshot
            .observation
            .checked_add(1)
            .ok_or(StorageError::RuntimeFenced)?;
        if effects {
            self.snapshot.effects_revision = self
                .snapshot
                .effects_revision
                .checked_add(1)
                .ok_or(StorageError::RuntimeFenced)?;
        }
        self.snapshot.progress.retain(|p| {
            cards.iter().any(|c| {
                c.operation.state == OperationState::Running
                    && c.running.iter().any(|(id, _)| *id == p.item)
            })
        });
        self.snapshot.progress.truncate(8);
        self.snapshot.cards = cards;
        if let Some((id, page)) = &self.selected {
            let exists = self
                .snapshot
                .cards
                .iter()
                .any(|c| c.operation.identity.id == *id);
            if exists {
                self.snapshot.details =
                    Some(runtime.command(|db, _, _| {
                        db.native_operation_items(id.clone(), *page)?.wait()
                    })?);
            } else if self.snapshot.preparing.as_ref() != Some(id) {
                self.snapshot.details = None;
                self.selected = None;
            }
        }
        self.last = Some(Instant::now());
        self.dirty = false;
        self.initialized = true;
        Ok((true, effects))
    }
}

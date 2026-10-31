//! Scoped composition for one active root. No self-referential owners, detached
//! database/artwork workers, or long-lived preparation cache crosses a move.
use crate::imports::{ImportExecutor, PrepareError, PreparedImport};
use librairii_adapters::thumbnails::ThumbnailCache;
use librairii_app::{
    lifecycle::{LibraryGeneration, LibraryUse, LifecycleError, LifecycleGate, Phase, Switch},
    native_jobs::{self, Configuration, Handle},
};
use librairii_storage::{
    IsolatedCloseFailure, IsolatedLibrary, LibraryLayout, Result, StorageError,
    database::{Database, WorkCancellation},
    diagnostics::Logger,
    native::{
        CleanupReport, NativeCloseFailure, NativeLibrary, RelocatedLibrary, RelocationFailure,
    },
    operations_repository::TerminalRepository,
    platform::unix::LibraryFiles,
    profile::ProfileLocations,
    settings::{RelocationPreview, TargetPreview},
};
use std::{path::Path, sync::Mutex, thread, time::Duration};

#[derive(Debug)]
pub enum SessionFailure<'profile> {
    Storage(StorageError),
    Lifecycle(LifecycleError),
    Jobs(native_jobs::SessionError),
    Close(Box<NativeCloseFailure<'profile>>),
    IsolatedClose(Box<IsolatedCloseFailure>),
    Move(Box<RelocationFailure<'profile>>),
}
impl std::fmt::Display for SessionFailure<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Storage(e) => e.fmt(f),
            Self::Lifecycle(e) => e.fmt(f),
            Self::Jobs(e) => e.fmt(f),
            Self::Close(e) => e.fmt(f),
            Self::IsolatedClose(e) => e.fmt(f),
            Self::Move(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for SessionFailure<'_> {}
impl From<StorageError> for SessionFailure<'_> {
    fn from(e: StorageError) -> Self {
        Self::Storage(e)
    }
}
impl From<LifecycleError> for SessionFailure<'_> {
    fn from(e: LifecycleError) -> Self {
        Self::Lifecycle(e)
    }
}
#[derive(Debug)]
pub struct LibrarySession<'profile> {
    library: Option<Owner<'profile>>,
    gate: LifecycleGate,
    cleanup: CleanupReport,
    cleanup_paths: Vec<std::path::PathBuf>,
    move_notice: Option<MoveNotice>,
    operation_generation: u64,
    catalog_client: Option<librairii_adapters::lunii::CatalogClient>,
}
#[derive(Debug)]
enum Owner<'p> {
    Native(Box<NativeLibrary<'p>>),
    Isolated(Box<IsolatedLibrary>),
}
impl Owner<'_> {
    fn layout(&self) -> &LibraryLayout {
        match self {
            Self::Native(n) => n.layout(),
            Self::Isolated(n) => n.layout(),
        }
    }
}
#[derive(Debug)]
pub struct MoveNotice {
    pub message_key: &'static str,
    pub technical_detail: String,
    pub retained_destination: Option<std::path::PathBuf>,
}
#[derive(Debug)]
pub struct Action<'gate, T>(ActionKind<'gate, T>);
#[derive(Debug)]
enum ActionKind<'gate, T> {
    Finish(T),
    Move {
        switch: Switch<'gate>,
        target: Box<TargetPreview>,
        id: String,
        control: Option<crate::maintenance::Control>,
    },
}
impl<T> Action<'_, T> {
    pub fn finish(value: T) -> Self {
        Self(ActionKind::Finish(value))
    }
}
/// UI/event-loop callbacks live inside this scope. Async readers/artwork retain
/// use_library() until completion; ordinary commands execute synchronously.
/// Preferences live outside this context and remain usable during relocation.
#[derive(Debug)]
pub struct Runtime<'gate, 'db, 'library> {
    artwork: crate::artwork::Handle,
    gate: &'gate LifecycleGate,
    database: &'db Database<'library>,
    files: &'db LibraryFiles<'library>,
    imports: &'db ImportExecutor<'db, 'library>,
    export_jobs: &'db librairii_storage::export::ExportExecutor<'db, 'library>,
    export_preparations: crate::export_preparation::Handle,
    jobs: Handle,
    thumbnails: &'db ThumbnailCache,
    root: &'db Path,
    exports: &'db Mutex<librairii_storage::export::Exports<'db, 'library>>,
    move_notice: Option<&'db MoveNotice>,
    cleanup_paths: &'db [std::path::PathBuf],
    library: &'db Owner<'library>,
    locations: Option<&'db ProfileLocations>,
    diagnostics: &'db Mutex<librairii_storage::diagnostics::Diagnostics<'db, 'db, 'db, 'library>>,
}
#[derive(Debug)]
pub struct JobObservation {
    pub critical: bool,
    pub status: native_jobs::RuntimeStatus,
    pub dirty: bool,
    pub progress: Vec<native_jobs::ProgressUpdate>,
    pub admissions: Vec<native_jobs::Admission>,
}
impl<'gate> Runtime<'gate, '_, '_> {
    pub(crate) fn request_artwork(&self, id: librairii_core::Checksum) {
        self.artwork.request(id);
    }
    pub(crate) fn observe_artwork(&self) -> Result<bool> {
        if self.artwork.recovery_required() {
            return Err(StorageError::CustodyRecoveryRequired);
        }
        Ok(self.artwork.take_dirty())
    }
    pub fn observe_jobs(&self) -> JobObservation {
        let mut admissions = Vec::new();
        while let Some(admission) = self.jobs.take_admission() {
            if !admission.accepted {
                self.export_jobs.reject_admission(&admission.id);
            }
            admissions.push(admission);
        }
        JobObservation {
            critical: self.jobs.has_critical_work()
                || self.export_preparations.recovery_required()
                || self.diagnostics_recovery_required(),
            status: self.jobs.status(),
            dirty: self.jobs.take_dirty(),
            progress: self.jobs.take_progress(),
            admissions,
        }
    }
    pub fn cancel_operation(&self, id: String) -> Result<()> {
        let _use = self
            .gate
            .enter_mutation()
            .map_err(|_| StorageError::RuntimeInUse)?;
        self.jobs.cancel(id).map_err(|_| StorageError::QueueFull)
    }
    pub fn offer_metadata(&self, start: native_jobs::Start) -> Result<()> {
        let _use = self
            .gate
            .enter_mutation()
            .map_err(|_| StorageError::RuntimeInUse)?;
        if start.input().kind != librairii_core::OperationKind::MetadataSync {
            return Err(StorageError::CompatibilityInvariant);
        }
        self.jobs
            .try_start(start)
            .map_err(|_| StorageError::QueueFull)
    }
    pub fn prepare_export(
        &self,
        scope: librairii_core::export::Scope,
        path: std::path::PathBuf,
    ) -> Result<u64> {
        let _use = self
            .gate
            .enter_mutation()
            .map_err(|_| StorageError::RuntimeInUse)?;
        self.export_preparations.prepare(scope, path)
    }
    pub fn cancel_export_preparation(&self) {
        self.export_preparations.cancel();
    }
    pub fn discard_export_preparation(
        &self,
        id: librairii_core::export::PreparationId,
    ) -> Result<()> {
        self.export_preparations.discard(id)
    }
    pub fn take_export_preparation(
        &self,
    ) -> Result<Option<(u64, Result<librairii_core::export::Preview>)>> {
        self.export_preparations.take_result()
    }
    pub fn offer_export(
        &self,
        preparation: &librairii_core::export::PreparationId,
        id: String,
    ) -> Result<()> {
        self.export_command(|exports| self.export_jobs.offer(&self.jobs, exports, preparation, id))
    }
    pub fn preview_relocation(
        &self,
        selected: &Path,
        cancel: &WorkCancellation,
    ) -> Result<RelocationPreview> {
        if !self.jobs.idle_for_relocation() {
            return Err(StorageError::RuntimeInUse);
        }
        let _use = self.use_library().map_err(|_| StorageError::RuntimeInUse)?;
        match (self.library, self.locations) {
            (Owner::Native(library), Some(locations)) => {
                library.preview_relocation(selected, locations, cancel)
            }
            _ => Err(StorageError::SettingsDestination),
        }
    }
    pub fn relocation_summary(
        &self,
        cancel: &WorkCancellation,
    ) -> Result<librairii_storage::settings::ManifestSummary> {
        let _use = self.use_library().map_err(|_| StorageError::RuntimeInUse)?;
        match self.library {
            Owner::Native(library) => library.relocation_preview_summary(cancel),
            Owner::Isolated(_) => Err(StorageError::SettingsDestination),
        }
    }
    pub fn diagnostics(
        &self,
        selected: &Path,
        cancel: &WorkCancellation,
    ) -> Result<librairii_storage::diagnostics::Report> {
        let _use = self
            .gate
            .enter_mutation()
            .map_err(|_| StorageError::RuntimeInUse)?;
        self.diagnostics
            .lock()
            .map_err(|_| StorageError::RuntimeFenced)?
            .export_named(selected, cancel)
            .map_err(|failure| failure.into_storage_error())
    }
    pub fn diagnostics_recovery_required(&self) -> bool {
        self.diagnostics
            .lock()
            .map(|d| d.retained_outputs() > 0)
            .unwrap_or(true)
    }
    pub fn is_override(&self) -> bool {
        matches!(self.library, Owner::Isolated(_))
    }
    pub fn root(&self) -> &Path {
        self.root
    }
    pub fn move_notice(&self) -> Option<&MoveNotice> {
        self.move_notice
    }
    pub fn pending_cleanup_paths(&self) -> &[std::path::PathBuf] {
        self.cleanup_paths
    }
    pub fn use_library(&self) -> std::result::Result<LibraryUse<'gate>, LifecycleError> {
        self.gate.enter()
    }
    pub fn generation(&self) -> std::result::Result<LibraryGeneration, LifecycleError> {
        Ok(self.use_library()?.generation().clone())
    }
    pub fn accepts(&self, generation: &LibraryGeneration) -> bool {
        self.gate.accepts(generation)
    }
    pub fn command<T>(
        &self,
        work: impl FnOnce(&Database<'_>, &LibraryFiles<'_>, &ThumbnailCache) -> Result<T>,
    ) -> Result<T> {
        let _use = self
            .gate
            .enter_mutation()
            .map_err(|_| StorageError::RuntimeInUse)?;
        work(self.database, self.files, self.thumbnails)
    }
    pub fn offer_import(
        &self,
        prepared: PreparedImport,
    ) -> std::result::Result<(), (PrepareError, PreparedImport)> {
        let Ok(_use) = self.gate.enter_mutation() else {
            return Err((PrepareError::Busy, prepared));
        };
        self.imports.offer(&self.jobs, prepared)
    }
    pub fn export_command<T>(
        &self,
        work: impl FnOnce(&mut librairii_storage::export::Exports<'_, '_>) -> Result<T>,
    ) -> Result<T> {
        let _use = self
            .gate
            .enter_mutation()
            .map_err(|_| StorageError::RuntimeInUse)?;
        let mut exports = self.exports.try_lock().map_err(|e| match e {
            std::sync::TryLockError::WouldBlock => StorageError::QueueFull,
            std::sync::TryLockError::Poisoned(_) => StorageError::RuntimeFenced,
        })?;
        work(&mut exports)
    }
    pub fn move_library<T>(
        &self,
        target: TargetPreview,
        id: String,
    ) -> std::result::Result<Action<'gate, T>, LifecycleError> {
        self.prepare_move(target, id, None)
    }
    pub fn move_library_controlled<T>(
        &self,
        target: TargetPreview,
        id: String,
        control: crate::maintenance::Control,
    ) -> std::result::Result<Action<'gate, T>, LifecycleError> {
        self.prepare_move(target, id, Some(control))
    }
    fn prepare_move<T>(
        &self,
        target: TargetPreview,
        id: String,
        control: Option<crate::maintenance::Control>,
    ) -> std::result::Result<Action<'gate, T>, LifecycleError> {
        if self.is_override() {
            return Err(LifecycleError::Busy);
        }
        self.artwork.cancel();
        let switch=self.gate.begin_switch(|| {
            self.jobs.idle_for_relocation() && self.database.read(|c|c.query_row("SELECT NOT EXISTS(SELECT 1 FROM file_operations WHERE status IN ('queued','running'))",[],|r|r.get::<_,bool>(0)).map_err(StorageError::from)).and_then(|reply|reply.wait()).unwrap_or(false)
        })?;
        Ok(Action(ActionKind::Move {
            switch,
            target: Box::new(target),
            id,
            control,
        }))
    }
}
impl<'profile> LibrarySession<'profile> {
    pub fn new(library: NativeLibrary<'profile>) -> Self {
        Self {
            library: Some(Owner::Native(Box::new(library))),
            gate: LifecycleGate::new(),
            cleanup: CleanupReport::default(),
            cleanup_paths: Vec::new(),
            move_notice: None,
            operation_generation: 1,
            catalog_client: None,
        }
    }
    pub fn isolated(library: IsolatedLibrary) -> Self {
        Self {
            library: Some(Owner::Isolated(Box::new(library))),
            gate: LifecycleGate::new(),
            cleanup: CleanupReport::default(),
            cleanup_paths: Vec::new(),
            move_notice: None,
            operation_generation: 1,
            catalog_client: None,
        }
    }
    /// Presentation only. Cleanup itself still requires qualified storage
    /// ownership and the real relocation journal/manifest.
    pub(crate) fn with_cleanup_notice(mut self, paths: Vec<std::path::PathBuf>) -> Self {
        self.cleanup_paths = paths;
        self
    }
    pub fn with_catalog_client(
        mut self,
        client: Option<librairii_adapters::lunii::CatalogClient>,
    ) -> Self {
        self.catalog_client = client;
        self
    }
    pub(crate) fn retryable(&self) -> bool {
        match self.library.as_ref() {
            Some(Owner::Native(n)) => n.runtime_reusable(),
            Some(Owner::Isolated(n)) => n.runtime_reusable(),
            None => false,
        }
    }
    pub fn cleanup(&self) -> CleanupReport {
        self.cleanup
    }
    pub fn move_notice(&self) -> Option<&MoveNotice> {
        self.move_notice.as_ref()
    }
    /// Event loop driver on its owned blocking context. A move request stops
    /// intake/drains uses, joins jobs, invalidates artwork, flushes logs, closes
    /// DB/readers, then consumes the actual closed owner. The next iteration
    /// creates every service before activating the next generation.
    pub fn drive<T>(
        &mut self,
        locations: &ProfileLocations,
        cancel: &WorkCancellation,
        run: impl for<'g, 'a, 'b> FnMut(&Runtime<'g, 'a, 'b>) -> Result<Action<'g, T>>,
    ) -> std::result::Result<T, SessionFailure<'profile>> {
        self.drive_inner(Some(locations), cancel, |_, _| {}, run)
    }
    pub fn drive_override<T>(
        &mut self,
        cancel: &WorkCancellation,
        run: impl for<'g, 'a, 'b> FnMut(&Runtime<'g, 'a, 'b>) -> Result<Action<'g, T>>,
    ) -> std::result::Result<T, SessionFailure<'profile>> {
        self.drive_inner(None, cancel, |_, _| {}, run)
    }
    pub(crate) fn drive_inner<T>(
        &mut self,
        locations: Option<&ProfileLocations>,
        cancel: &WorkCancellation,
        mut unclosed: impl FnMut(&StorageError, &Path),
        mut run: impl for<'g, 'a, 'b> FnMut(&Runtime<'g, 'a, 'b>) -> Result<Action<'g, T>>,
    ) -> std::result::Result<T, SessionFailure<'profile>> {
        let mut activation = if self.gate.phase()? == Phase::Recovery {
            Some(self.gate.begin_recovery()?)
        } else {
            None
        };
        loop {
            let generation = self.operation_generation;
            self.operation_generation = self
                .operation_generation
                .checked_add(1)
                .ok_or(StorageError::RuntimeFenced)?;
            let native = self.library.as_ref().ok_or(StorageError::RuntimeFenced)?;
            let mut database = match native {
                Owner::Native(n) => Database::open_native(n)?,
                Owner::Isolated(n) => Database::open_isolated(n)?,
            };
            let action_result =
                (|| -> std::result::Result<Action<'_, T>, SessionFailure<'profile>> {
                    match database
                        .seed_age_facets(Default::default(), cancel.clone())?
                        .wait()?
                    {
                        librairii_storage::database::TransactionOutcome::Committed(_) => {}
                        librairii_storage::database::TransactionOutcome::ProvenNotCommitted(
                            error,
                        ) => {
                            return Err(error.into());
                        }
                        librairii_storage::database::TransactionOutcome::CommitUncertain(_) => {
                            return Err(StorageError::RuntimeFenced.into());
                        }
                    }

                    let action = {
                        let files = match native {
                            Owner::Native(_) => LibraryFiles::open_native(&database)?,
                            Owner::Isolated(n) => LibraryFiles::open(n)?,
                        };
                        let logger = Logger::new(&database, &files)?;
                        let thumbnails = ThumbnailCache::default();
                        let artwork_store = crate::artwork::Repository::new(&database);
                        let artwork_service = self
                            .catalog_client
                            .clone()
                            .map(Ok)
                            .unwrap_or_else(librairii_adapters::lunii::CatalogClient::production)
                            .ok()
                            .map(|client| {
                                librairii_adapters::lunii::ArtworkService::new(
                                    &artwork_store,
                                    client,
                                )
                            });
                        let imports = ImportExecutor::new(&database);
                        let metadata = crate::metadata_jobs::MetadataExecutor::new(
                            &database,
                            self.catalog_client.clone(),
                        );
                        let export_jobs =
                            librairii_storage::export::ExportExecutor::new(&database, &files);
                        let executors = crate::metadata_jobs::Executors {
                            imports: &imports,
                            metadata: &metadata,
                            exports: &export_jobs,
                        };
                        let exports =
                            Mutex::new(librairii_storage::export::Exports::new(&database));
                        let diagnostics = Mutex::new(
                            librairii_storage::diagnostics::Diagnostics::new(&database, &logger)?,
                        );
                        let repository = TerminalRepository::new(&database, generation);
                        let action = native_jobs::with_runtime(
                            &repository,
                            &executors,
                            Configuration::new(2).ok_or(StorageError::CompatibilityInvariant)?,
                            |jobs| -> Result<Action<'_, T>> {
                                while jobs.status().starting {
                                    thread::sleep(Duration::from_millis(2));
                                }
                                if jobs.status().failed {
                                    return Err(StorageError::RuntimeFenced);
                                }
                                if let Some(switch) = activation.take() {
                                    switch
                                        .activated()
                                        .map_err(|_| StorageError::RuntimeFenced)?;
                                }
                                crate::artwork::with_worker(
                                    &self.gate,
                                    artwork_service.as_ref(),
                                    &artwork_store,
                                    &thumbnails,
                                    |artwork| {
                                        crate::export_preparation::with_worker(
                                            &exports,
                                            |export_preparations| {
                                                let runtime = Runtime {
                                                    artwork,
                                                    gate: &self.gate,
                                                    database: &database,
                                                    files: &files,
                                                    imports: &imports,
                                                    export_jobs: &export_jobs,
                                                    export_preparations,
                                                    jobs,
                                                    thumbnails: &thumbnails,
                                                    root: native.layout().root(),
                                                    exports: &exports,
                                                    move_notice: self.move_notice.as_ref(),
                                                    cleanup_paths: &self.cleanup_paths,
                                                    library: native,
                                                    locations,
                                                    diagnostics: &diagnostics,
                                                };
                                                run(&runtime)
                                            },
                                        )?
                                    },
                                )
                            },
                        );
                        // A failed publication keeps the exact metadata store,
                        // file receipts and custody live after the worker joins.
                        while artwork_store.recovery_required() {
                            unclosed(
                                &StorageError::CustodyRecoveryRequired,
                                native.layout().root(),
                            );
                            thread::sleep(Duration::from_millis(100));
                        }
                        let action = action.map_err(SessionFailure::Jobs)??;
                        // with_runtime has positively joined its coordinator/workers.
                        thumbnails
                            .invalidate()
                            .map_err(|_| StorageError::RuntimeFenced)?;
                        exports
                            .lock()
                            .map_err(|_| StorageError::RuntimeFenced)?
                            .invalidate();
                        if let Err(error) = logger.flush() {
                            loop {
                                unclosed(&error, native.layout().root());
                                thread::sleep(Duration::from_millis(100));
                            }
                        }
                        // A stopped UI/runtime is not permission to release uncertain
                        // native output ownership or report a successful close.
                        while diagnostics
                            .lock()
                            .map_err(|_| StorageError::RuntimeFenced)?
                            .retained_outputs()
                            > 0
                        {
                            unclosed(
                                &StorageError::CustodyRecoveryRequired,
                                native.layout().root(),
                            );
                            thread::sleep(Duration::from_millis(100));
                        }
                        action
                    };
                    Ok(action)
                })();
            if let Err(error) = database.close() {
                // close_failed is a permanent fence. Retrying a reconstructed
                // Database cannot settle this actual owner's missing proof.
                // Keep it, its root and every close witness alive; publish an
                // honest nonretryable recovery while language remains usable.
                loop {
                    unclosed(&error, native.layout().root());
                    thread::sleep(Duration::from_millis(100));
                }
            }
            drop(database);
            let action = action_result?;
            match action.0 {
                ActionKind::Finish(value) => return Ok(value),
                ActionKind::Move {
                    switch,
                    target,
                    id,
                    control,
                } => {
                    let id = id.parse().map_err(|_| StorageError::InvalidProfile)?;
                    let native = self.library.take().ok_or(StorageError::RuntimeFenced)?;
                    let Owner::Native(native) = native else {
                        return Err(StorageError::SettingsDestination.into());
                    };
                    let locations = locations.ok_or(StorageError::InvalidProfile)?;
                    let previous_root = native.layout().root().to_path_buf();
                    let closed = native
                        .close_for_relocation()
                        .map_err(SessionFailure::Close)?;
                    let moved = match &control {
                        Some(control) => closed.relocate_observed(
                            *target,
                            locations,
                            id,
                            &control.cancellation(),
                            control,
                        ),
                        None => closed.relocate(*target, locations, id, cancel),
                    };
                    let outcome = match moved {
                        Ok(outcome) => {
                            self.move_notice = None;
                            outcome
                        }
                        Err(failure)
                            if failure.commit_status()
                                == librairii_storage::native::CommitStatus::BeforeCommit =>
                        {
                            let notice = MoveNotice {
                                message_key: "settings.move_failed",
                                technical_detail: failure.storage_error().to_string(),
                                retained_destination: failure
                                    .retained_destination()
                                    .map(Path::to_path_buf),
                            };
                            let (library, cleanup) =
                                failure.resume_source().map_err(SessionFailure::Move)?;
                            self.move_notice = Some(notice);
                            RelocatedLibrary { library, cleanup }
                        }
                        Err(failure) => return Err(SessionFailure::Move(failure)),
                    };
                    self.cleanup_paths = if outcome.cleanup.pending
                        && outcome.library.layout().root() != previous_root
                    {
                        vec![previous_root]
                    } else {
                        Vec::new()
                    };
                    self.cleanup = outcome.cleanup;
                    self.library = Some(Owner::Native(Box::new(outcome.library)));
                    activation = Some(switch);
                }
            }
        }
    }
    pub fn accept_recovered_move(&mut self, outcome: RelocatedLibrary<'profile>) -> Result<()> {
        if self.library.is_some()
            || self.gate.phase().map_err(|_| StorageError::RuntimeFenced)? != Phase::Recovery
        {
            return Err(StorageError::RuntimeInUse);
        }
        self.library = Some(Owner::Native(Box::new(outcome.library)));
        self.cleanup = outcome.cleanup;
        Ok(())
    }
    pub fn finish(mut self) -> std::result::Result<(), SessionFailure<'profile>> {
        match self.library.take().ok_or(StorageError::RuntimeFenced)? {
            Owner::Native(n) => n.finish().map_err(SessionFailure::Close),
            Owner::Isolated(n) => n.finish().map_err(SessionFailure::IsolatedClose),
        }
    }
}

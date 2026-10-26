//! Scoped composition for one active root. No self-referential owners, detached
//! database/artwork workers, or long-lived preparation cache crosses a move.
use crate::imports::{ImportExecutor, PrepareError, PreparedImport};
use librairii_adapters::thumbnails::ThumbnailCache;
use librairii_app::{
    lifecycle::{LibraryGeneration, LibraryUse, LifecycleError, LifecycleGate, Phase, Switch},
    native_jobs::{self, Configuration, Handle},
};
use librairii_storage::{
    Result, StorageError,
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
    Move(Box<RelocationFailure<'profile>>),
}
impl std::fmt::Display for SessionFailure<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Storage(e) => e.fmt(f),
            Self::Lifecycle(e) => e.fmt(f),
            Self::Jobs(e) => e.fmt(f),
            Self::Close(e) => e.fmt(f),
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
    library: Option<NativeLibrary<'profile>>,
    gate: LifecycleGate,
    cleanup: CleanupReport,
    move_notice: Option<MoveNotice>,
    operation_generation: u64,
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
    gate: &'gate LifecycleGate,
    database: &'db Database<'library>,
    files: &'db LibraryFiles<'library>,
    imports: &'db ImportExecutor<'db, 'library>,
    jobs: Handle,
    thumbnails: &'db ThumbnailCache,
    root: &'db Path,
    exports: &'db Mutex<librairii_storage::export::Exports<'db, 'library>>,
    move_notice: Option<&'db MoveNotice>,
    library: &'db NativeLibrary<'library>,
    locations: &'db ProfileLocations,
}
impl<'gate> Runtime<'gate, '_, '_> {
    pub fn preview_relocation(
        &self,
        selected: &Path,
        cancel: &WorkCancellation,
    ) -> Result<RelocationPreview> {
        let _use = self.use_library().map_err(|_| StorageError::RuntimeInUse)?;
        self.library
            .preview_relocation(selected, self.locations, cancel)
    }
    pub fn root(&self) -> &Path {
        self.root
    }
    pub fn move_notice(&self) -> Option<&MoveNotice> {
        self.move_notice
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
        let _use = self.use_library().map_err(|_| StorageError::RuntimeInUse)?;
        work(self.database, self.files, self.thumbnails)
    }
    pub fn offer_import(
        &self,
        prepared: PreparedImport,
    ) -> std::result::Result<(), (PrepareError, PreparedImport)> {
        let Ok(_use) = self.use_library() else {
            return Err((PrepareError::Busy, prepared));
        };
        self.imports.offer(&self.jobs, prepared)
    }
    pub fn export_command<T>(
        &self,
        work: impl FnOnce(&mut librairii_storage::export::Exports<'_, '_>) -> Result<T>,
    ) -> Result<T> {
        let _use = self.use_library().map_err(|_| StorageError::RuntimeInUse)?;
        let mut exports = self
            .exports
            .lock()
            .map_err(|_| StorageError::RuntimeFenced)?;
        work(&mut exports)
    }
    pub fn move_library<T>(
        &self,
        target: TargetPreview,
        id: String,
    ) -> std::result::Result<Action<'gate, T>, LifecycleError> {
        let switch=self.gate.begin_switch(|| {
            self.jobs.idle_for_relocation() && self.database.read(|c|c.query_row("SELECT NOT EXISTS(SELECT 1 FROM file_operations WHERE status IN ('queued','running'))",[],|r|r.get::<_,bool>(0)).map_err(StorageError::from)).and_then(|reply|reply.wait()).unwrap_or(false)
        })?;
        Ok(Action(ActionKind::Move {
            switch,
            target: Box::new(target),
            id,
        }))
    }
}
impl<'profile> LibrarySession<'profile> {
    pub fn new(library: NativeLibrary<'profile>) -> Self {
        Self {
            library: Some(library),
            gate: LifecycleGate::new(),
            cleanup: CleanupReport::default(),
            move_notice: None,
            operation_generation: 1,
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
            let mut database = Database::open_native(native)?;
            let action = {
                let files = LibraryFiles::open_native(&database)?;
                let logger = Logger::new(&database, &files)?;
                let thumbnails = ThumbnailCache::default();
                let imports = ImportExecutor::new(&database);
                let exports = Mutex::new(librairii_storage::export::Exports::new(&database));
                let repository = TerminalRepository::new(&database, generation);
                let action = native_jobs::with_runtime(
                    &repository,
                    &imports,
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
                        let runtime = Runtime {
                            gate: &self.gate,
                            database: &database,
                            files: &files,
                            imports: &imports,
                            jobs,
                            thumbnails: &thumbnails,
                            root: native.layout().root(),
                            exports: &exports,
                            move_notice: self.move_notice.as_ref(),
                            library: native,
                            locations,
                        };
                        run(&runtime)
                    },
                )
                .map_err(SessionFailure::Jobs)??;
                // with_runtime has positively joined its coordinator/workers.
                thumbnails
                    .invalidate()
                    .map_err(|_| StorageError::RuntimeFenced)?;
                exports
                    .lock()
                    .map_err(|_| StorageError::RuntimeFenced)?
                    .invalidate();
                logger.flush()?;
                action
            };
            database.close()?;
            drop(database);
            match action.0 {
                ActionKind::Finish(value) => return Ok(value),
                ActionKind::Move { switch, target, id } => {
                    let id = id.parse().map_err(|_| StorageError::InvalidProfile)?;
                    let native = self.library.take().ok_or(StorageError::RuntimeFenced)?;
                    let closed = native
                        .close_for_relocation()
                        .map_err(SessionFailure::Close)?;
                    let outcome = match closed.relocate(*target, locations, id, cancel) {
                        Ok(outcome) => outcome,
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
                    self.cleanup = outcome.cleanup;
                    self.library = Some(outcome.library);
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
        self.library = Some(outcome.library);
        self.cleanup = outcome.cleanup;
        Ok(())
    }
    pub fn finish(mut self) -> std::result::Result<(), SessionFailure<'profile>> {
        self.library
            .take()
            .ok_or(StorageError::RuntimeFenced)?
            .finish()
            .map_err(SessionFailure::Close)
    }
}

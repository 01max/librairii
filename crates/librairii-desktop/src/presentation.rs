//! One owned controller worker. Only bounded, owned presentation values cross
//! the native event-loop boundary; storage owners never enter a UI callback.
use crate::startup::{NativeEnvironment, StartupSelection, select_startup_with_progress};
use librairii_app::localization::{Catalogs, Language};
use librairii_storage::{database::WorkCancellation, settings::Startup};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, JoinHandle};

pub(crate) mod intake;
use intake::Intake;
pub use intake::OfferError;
pub const COMMAND_CAPACITY: usize = 16;
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Command {
    Refresh,
    Back,
    Forward,
    Navigate(librairii_core::collection_codec::url::Scope),
    Presentation(librairii_core::navigation::Presentation),
    Request(librairii_core::query::QueryRequest),
    Sort {
        request: librairii_core::query::QueryRequest,
        manual_order: bool,
    },
    ChangePage(i32),
    Select {
        id: i64,
        query: librairii_core::navigation::QueryStamp,
    },
    Language(Language),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Phase {
    Opening,
    Setup,
    Adoption,
    Recovery,
    Library,
}
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub facets: crate::browsing::Facets,
    pub groups: Vec<crate::browsing::Group>,
    pub revision: u64,
    /// Last scope/history command revision, retained across superseding snapshots.
    pub restore_revision: u64,
    pub generation: Option<librairii_app::lifecycle::LibraryGeneration>,
    pub phase: Phase,
    pub language: Language,
    pub status: String,
    pub navigation: Option<librairii_core::navigation::Navigation>,
    pub page: Option<crate::browsing::Page>,
}
#[derive(Debug)]
pub struct Worker {
    commands: Arc<Intake>,
    latest: Arc<Mutex<Option<Snapshot>>>,
    stop: Arc<AtomicBool>,
    startup_cancel: WorkCancellation,
    join: Option<JoinHandle<Result<(), String>>>,
}
impl Worker {
    pub fn start() -> Result<Self, std::io::Error> {
        Self::start_with(NativeEnvironment)
    }
    pub fn start_with(
        environment: impl crate::startup::StartupEnvironment + Send + 'static,
    ) -> Result<Self, std::io::Error> {
        let commands = Arc::new(Intake::default());
        let receiver = commands.clone();
        let latest = Arc::new(Mutex::new(None));
        let stop = Arc::new(AtomicBool::new(false));
        let mailbox = latest.clone();
        let cancelled = stop.clone();
        let startup_cancel = WorkCancellation::default();
        let probe_cancel = startup_cancel.clone();
        let join = thread::Builder::new()
            .name("native-controller".into())
            .spawn(move || run(environment, receiver, mailbox, cancelled, probe_cancel))?;
        Ok(Self {
            commands,
            latest,
            stop,
            startup_cancel,
            join: Some(join),
        })
    }
    pub fn offer(&self, command: Command) -> Result<u64, OfferError> {
        if self.stop.load(Ordering::Acquire) {
            return Err(OfferError::Closed);
        }
        self.commands.offer(command)
    }
    pub fn revision(&self) -> u64 {
        self.commands.latest()
    }
    pub fn take_snapshot(&self) -> Option<Snapshot> {
        self.latest.lock().ok()?.take().filter(|snapshot| {
            snapshot.phase == Phase::Recovery || snapshot.revision == self.commands.latest()
        })
    }
    pub fn request_shutdown(&self) {
        self.stop.store(true, Ordering::Release);
        self.startup_cancel.cancel();
    }
    pub fn finished(&self) -> bool {
        self.join.as_ref().is_none_or(JoinHandle::is_finished)
    }
    pub fn finish(&mut self) -> Result<(), String> {
        self.request_shutdown();
        match self.join.take() {
            Some(join) => join
                .join()
                .map_err(|_| "native controller failed".to_owned())?,
            None => Ok(()),
        }
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        let _closed = self.finish();
    }
}
fn run(
    environment: impl crate::startup::StartupEnvironment,
    receiver: Arc<Intake>,
    mailbox: Arc<Mutex<Option<Snapshot>>>,
    stop: Arc<AtomicBool>,
    startup_cancel: WorkCancellation,
) -> Result<(), String> {
    let catalogs = Catalogs::embedded().map_err(|e| e.to_string())?;
    let mut selection = select_startup_with_progress(&environment, &startup_cancel, || {
        *mailbox
            .lock()
            .map_err(|_| librairii_storage::StorageError::RuntimeFenced)? = Some(Snapshot {
            facets: Default::default(),
            revision: 0,
            restore_revision: 0,
            generation: None,
            phase: Phase::Opening,
            language: Language::English,
            status: "Librairii".into(),
            groups: Vec::new(),
            navigation: None,
            page: None,
        });
        Ok(())
    })
    .map_err(|e| e.to_string())?;
    if stop.load(Ordering::Acquire) {
        return Ok(());
    }
    if let StartupSelection::Production {
        locations,
        decision,
    } = &selection
        && let Startup::Native(document) = decision.as_ref()
    {
        let language =
            Language::explicit(&document.profile().language).map_err(|e| e.to_string())?;
        if crate::browsing::run(locations, language, &receiver, &mailbox, &stop).is_ok() {
            return Ok(());
        }
        return recovery(&receiver, &mailbox, &stop, language, |requested| {
            let store =
                librairii_storage::profile::NativeProfileStore::open_native(&locations.native)?;
            let mut settings = crate::settings::restore_language(&store, language)?;
            crate::settings::change_language(&store, &mut settings, requested.code())
        })
        .map_err(|e| e.to_string());
    }
    let (phase, language, key) = match &selection {
        StartupSelection::Override(settings) => {
            if settings.root().exists() {
                return Err("development library requires confirmation; existing override launch is not implemented".into());
            }
            (Phase::Setup, Language::English, "settings.setup_required")
        }
        StartupSelection::Production {
            decision,
            locations,
        } => match decision.as_ref() {
            Startup::ConfirmFolder { .. } => {
                let saved = librairii_storage::profile::read_legacy(&locations.native)
                    .ok()
                    .flatten()
                    .or_else(|| {
                        locations
                            .legacy
                            .as_ref()
                            .and_then(|p| librairii_storage::profile::read_legacy(p).ok().flatten())
                    });
                let language = saved
                    .and_then(|d| Language::explicit(&d.profile().language).ok())
                    .unwrap_or_else(crate::settings::system_language);
                (Phase::Setup, language, "settings.setup_required")
            }
            Startup::AdoptIndependent(input) => (
                Phase::Adoption,
                Language::explicit(&input.profile().profile().language)
                    .map_err(|e| e.to_string())?,
                "Choose library folder",
            ),
            Startup::Recovery { configured, .. } => (
                Phase::Recovery,
                configured
                    .as_ref()
                    .and_then(|p| Language::explicit(&p.language).ok())
                    .unwrap_or_default(),
                "Local library needs recovery",
            ),
            Startup::Native(document) => (
                Phase::Library,
                Language::explicit(&document.profile().language).map_err(|e| e.to_string())?,
                "Opening your local library",
            ),
        },
    };
    let mut snapshot = Snapshot {
        facets: Default::default(),
        groups: Vec::new(),
        revision: 0,
        restore_revision: 0,
        generation: None,
        navigation: None,
        page: None,
        phase,
        language,
        status: catalogs
            .text(language, key)
            .map_err(|e| e.to_string())?
            .into(),
    };
    while !stop.load(Ordering::Acquire) {
        *mailbox
            .lock()
            .map_err(|_| "native presentation mailbox failed")? = Some(snapshot.clone());
        loop {
            if stop.load(Ordering::Acquire) {
                return Ok(());
            }
            let Some(envelope) = receiver.receive().map_err(|e| e.to_string())? else {
                continue;
            };
            snapshot.revision = envelope.revision;
            match envelope.command {
                Command::Language(language) => {
                    let result = match &mut selection {
                        StartupSelection::Override(settings) => {
                            settings.change_language(language.code())
                        }
                        StartupSelection::Production { locations, .. } => {
                            librairii_storage::settings::prepare_native_profile(
                                locations,
                                &Default::default(),
                            )
                            .and_then(|store| {
                                let mut presentation =
                                    crate::settings::restore_language(&store, snapshot.language)?;
                                crate::settings::change_language(
                                    &store,
                                    &mut presentation,
                                    language.code(),
                                )
                            })
                        }
                    };
                    if result.is_ok() {
                        snapshot.language = language;
                        snapshot.status = catalogs
                            .text(language, key)
                            .map_err(|e| e.to_string())?
                            .into();
                    } else {
                        snapshot.status = catalogs
                            .text(snapshot.language, "settings.restart_required")
                            .map_err(|e| e.to_string())?
                            .into();
                    }
                    break;
                }
                _ => break,
            }
        }
    }
    Ok(())
}

/// Keep failure reachable while retained session/profile owners remain in scope.
/// Ordinary queries/history are fenced; language changes retain independent
/// settings authority. Recovery never initializes/replaces a library.
pub(crate) fn recovery(
    intake: &Intake,
    mailbox: &Mutex<Option<Snapshot>>,
    stop: &AtomicBool,
    initial_language: Language,
    mut change_language: impl FnMut(Language) -> librairii_storage::Result<()>,
) -> librairii_storage::Result<()> {
    intake
        .fence()
        .map_err(|_| librairii_storage::StorageError::RuntimeFenced)?;
    let catalogs =
        Catalogs::embedded().map_err(|_| librairii_storage::StorageError::InvalidProfile)?;
    let mut language = initial_language;
    let mut key = "Local library needs recovery";
    while !stop.load(Ordering::Acquire) {
        *mailbox
            .lock()
            .map_err(|_| librairii_storage::StorageError::RuntimeFenced)? = Some(Snapshot {
            facets: Default::default(),
            revision: intake.latest(),
            restore_revision: 0,
            generation: None,
            phase: Phase::Recovery,
            language,
            status: catalogs
                .text(language, key)
                .map_err(|_| librairii_storage::StorageError::InvalidProfile)?
                .into(),
            groups: Vec::new(),
            navigation: None,
            page: None,
        });
        loop {
            if stop.load(Ordering::Acquire) {
                return Ok(());
            }
            let Some(envelope) = intake
                .receive()
                .map_err(|_| librairii_storage::StorageError::RuntimeFenced)?
            else {
                continue;
            };
            if let Command::Language(requested) = envelope.command {
                if change_language(requested).is_ok() {
                    language = requested;
                    key = "Local library needs recovery";
                } else {
                    key = "settings.restart_required";
                }
            }
            break;
        }
    }
    Ok(())
}

//! One owned controller worker. Only bounded, owned presentation values cross
//! the native event-loop boundary; storage owners never enter a UI callback.
use crate::startup::{NativeEnvironment, select_startup_with_progress};
use librairii_app::localization::{Catalogs, Language};
use librairii_storage::database::WorkCancellation;
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
    Maintenance {
        generation: Option<librairii_app::lifecycle::LibraryGeneration>,
        action: crate::maintenance::Action,
    },
    Exports {
        generation: librairii_app::lifecycle::LibraryGeneration,
        action: crate::workflows::exports::Action,
    },
    Operations {
        generation: librairii_app::lifecycle::LibraryGeneration,
        action: crate::workflows::operations::Action,
    },
    ShelfReview {
        generation: librairii_app::lifecycle::LibraryGeneration,
        action: crate::workflows::shelf_reviews::Action,
    },
    Shelves {
        generation: librairii_app::lifecycle::LibraryGeneration,
        action: crate::workflows::shelves::Action,
    },
    SelectUnavailable {
        uuid: String,
        query: librairii_core::navigation::QueryStamp,
    },
    Collections {
        generation: librairii_app::lifecycle::LibraryGeneration,
        action: crate::workflows::collections::Action,
    },
    Flags {
        generation: librairii_app::lifecycle::LibraryGeneration,
        action: crate::workflows::flags::Action,
    },
    Tags {
        generation: librairii_app::lifecycle::LibraryGeneration,
        action: crate::workflows::tags::Action,
    },
    Details(crate::workflows::StoryTarget),
    Remove(crate::workflows::StoryTarget),
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
    GroupPage(i32),
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
    pub workflows: crate::workflows::Snapshot,
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
    maintenance: crate::maintenance::Control,
    join: Option<JoinHandle<Result<(), String>>>,
}
impl Worker {
    pub fn start() -> Result<Self, std::io::Error> {
        Self::start_with(NativeEnvironment)
    }
    pub fn start_with(
        environment: impl crate::startup::StartupEnvironment + Send + 'static,
    ) -> Result<Self, std::io::Error> {
        let commands = Arc::new(Intake::with_scope(environment.acceptance_scope()));
        let receiver = commands.clone();
        let latest = Arc::new(Mutex::new(None));
        let stop = Arc::new(AtomicBool::new(false));
        let mailbox = latest.clone();
        let cancelled = stop.clone();
        let startup_cancel = WorkCancellation::default();
        let probe_cancel = startup_cancel.clone();
        let maintenance = crate::maintenance::Control::default();
        let maintenance_worker = maintenance.clone();
        let join = thread::Builder::new()
            .name("native-controller".into())
            .spawn(move || {
                run(
                    environment,
                    receiver,
                    mailbox,
                    cancelled,
                    probe_cancel,
                    maintenance_worker,
                )
            })?;
        Ok(Self {
            commands,
            latest,
            stop,
            startup_cancel,
            maintenance,
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
    pub fn maintenance_control(&self) -> crate::maintenance::Control {
        self.maintenance.clone()
    }
    pub fn take_snapshot(&self) -> Option<Snapshot> {
        self.latest.lock().ok()?.take().filter(|snapshot| {
            snapshot.phase == Phase::Recovery || snapshot.revision == self.commands.latest()
        })
    }
    pub fn request_shutdown(&self) {
        self.stop.store(true, Ordering::Release);
        self.startup_cancel.cancel();
        self.maintenance.cancel();
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
    maintenance: crate::maintenance::Control,
) -> Result<(), String> {
    let catalog_client = environment.catalog_client();
    let selection = select_startup_with_progress(&environment, &startup_cancel, || {
        *mailbox
            .lock()
            .map_err(|_| librairii_storage::StorageError::RuntimeFenced)? = Some(Snapshot {
            workflows: Default::default(),
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
    crate::startup_flow::run(
        selection,
        &receiver,
        &mailbox,
        &stop,
        catalog_client,
        maintenance,
    )
    .map_err(|e| e.to_string())
}

/// Keep failure reachable while retained session/profile owners remain in scope.
/// Ordinary queries/history are fenced; language changes retain independent
/// settings authority. Recovery never initializes/replaces a library.
pub(crate) fn recovery_with_workflows(
    intake: &Intake,
    mailbox: &Mutex<Option<Snapshot>>,
    stop: &AtomicBool,
    initial_language: Language,
    mut workflows: crate::workflows::Snapshot,
    mut change_language: impl FnMut(Language) -> librairii_storage::Result<()>,
) -> librairii_storage::Result<()> {
    if workflows.settings.mode != crate::maintenance::Mode::Override {
        workflows.settings.mode = crate::maintenance::Mode::Recovery;
    }
    workflows.settings.can_confirm = false;
    workflows.settings.can_retry = false;
    workflows.settings.can_locate = false;
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
            workflows: workflows.clone(),
            facets: Default::default(),
            revision: intake.latest(),
            restore_revision: 0,
            generation: None,
            phase: Phase::Recovery,
            language,
            status: {
                let status = catalogs
                    .text(language, key)
                    .map_err(|_| librairii_storage::StorageError::InvalidProfile)?;
                let mut confirmed = String::new();
                if workflows.removal.as_ref().is_some_and(|r| r.is_ok()) {
                    confirmed.push_str(if language == Language::French {
                        "L’histoire a été retirée. "
                    } else {
                        "The story was removed. "
                    });
                }
                if matches!(
                    workflows.shelves.feedback,
                    Some(crate::workflows::shelves::Feedback::Saved(_))
                ) {
                    confirmed.push_str(if language == Language::French {
                        "L’Étagère a été enregistrée. "
                    } else {
                        "The Shelf was saved. "
                    });
                }
                if let Some(crate::workflows::shelf_reviews::Feedback::Committed(outcome)) =
                    &workflows.shelf_reviews.feedback
                {
                    use crate::workflows::shelf_reviews::Outcome;
                    confirmed.push_str(match (outcome, language == Language::French) {
                        (Outcome::ShelfSaved { changed: true, .. }, true) => "Les décisions approuvées ont été enregistrées. ",
                        (Outcome::ShelfSaved { changed: true, .. }, false) => "Approved decisions were saved. ",
                        (Outcome::ShelfSaved { changed: false, .. }, _) => "",
                        (Outcome::ShelfDeleted(_), true) => "L’Étagère a été supprimée ; les histoires ont été conservées. ",
                        (Outcome::ShelfDeleted(_), false) => "The Shelf was deleted; stories were retained. ",
                        (Outcome::CollectionDeleted(_), true) => "La Collection a été supprimée et les décisions approuvées ont été enregistrées. ",
                        (Outcome::CollectionDeleted(_), false) => "The Collection was deleted and approved decisions were saved. ",
                    });
                }
                if workflows.shelf_reviews.warning_preference_error {
                    confirmed.push_str(if language == Language::French {"L’avertissement de correspondance n’a pas pu être masqué durablement. Les membres approuvés sont conservés. "} else {"The mismatch warning could not be saved as dismissed. Approved membership is retained. "});
                }
                format!("{confirmed}{status}")
            },
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

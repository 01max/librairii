//! One owned controller worker. Only bounded, owned presentation values cross
//! the native event-loop boundary; storage owners never enter a UI callback.
use crate::startup::{NativeEnvironment, StartupSelection, select_startup};
use librairii_app::localization::{Catalogs, Language};
use librairii_storage::{database::WorkCancellation, settings::Startup};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::thread::{self, JoinHandle};

pub const COMMAND_CAPACITY: usize = 16;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Command {
    Refresh,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Phase {
    Setup,
    Adoption,
    Recovery,
    Library,
}
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub phase: Phase,
    pub language: Language,
    pub status: String,
}
#[derive(Debug)]
pub struct Worker {
    pub commands: mpsc::SyncSender<Command>,
    latest: Arc<Mutex<Option<Snapshot>>>,
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<Result<(), String>>>,
}
impl Worker {
    pub fn start() -> Result<Self, std::io::Error> {
        let (commands, receiver) = mpsc::sync_channel(COMMAND_CAPACITY);
        let latest = Arc::new(Mutex::new(None));
        let stop = Arc::new(AtomicBool::new(false));
        let mailbox = latest.clone();
        let cancelled = stop.clone();
        let join = thread::Builder::new()
            .name("native-controller".into())
            .spawn(move || run(receiver, mailbox, cancelled))?;
        Ok(Self {
            commands,
            latest,
            stop,
            join: Some(join),
        })
    }
    pub fn take_snapshot(&self) -> Option<Snapshot> {
        self.latest.lock().ok()?.take()
    }
    pub fn request_shutdown(&self) {
        self.stop.store(true, Ordering::Release);
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
    receiver: mpsc::Receiver<Command>,
    mailbox: Arc<Mutex<Option<Snapshot>>>,
    stop: Arc<AtomicBool>,
) -> Result<(), String> {
    let catalogs = Catalogs::embedded().map_err(|e| e.to_string())?;
    let selection = select_startup(&NativeEnvironment, &WorkCancellation::default())
        .map_err(|e| e.to_string())?;
    let (phase, language, key) = match &selection {
        StartupSelection::Override(settings) => {
            if settings.root().exists() {
                return Err("development library requires confirmation; existing override launch is not implemented".into());
            }
            (Phase::Setup, Language::English, "settings.setup_required")
        }
        StartupSelection::Production { decision, .. } => match decision.as_ref() {
            Startup::ConfirmFolder { .. } => {
                (Phase::Setup, Language::English, "settings.setup_required")
            }
            Startup::AdoptIndependent(_) => {
                (Phase::Adoption, Language::English, "Choose library folder")
            }
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
    let snapshot = Snapshot {
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
            match receiver.recv_timeout(std::time::Duration::from_millis(20)) {
                Ok(Command::Refresh) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
            }
        }
    }
    Ok(())
}

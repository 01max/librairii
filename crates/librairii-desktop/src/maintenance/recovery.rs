//! Retry the exact retained session/failure owner, never a reconstructed DTO.
use super::{Action, Control, Mode, Notice};
use crate::{
    library::{LibrarySession, SessionFailure},
    presentation::{Command, Phase, Snapshot, intake::Intake},
};
use librairii_app::localization::Language;
use librairii_core::navigation::Navigation;
use librairii_storage::{
    Result, StorageError,
    database::WorkCancellation,
    native::{CommitStatus, NativeCloseFailure, NativeLibrary, RelocationFailure},
    profile::{NativeProfileStore, ProfileLocations},
};
use librairii_ui::settings::SettingsPresentation;
use std::{
    cell::Cell,
    path::PathBuf,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
pub(crate) struct CloseContext<'a> {
    pub preferences: &'a dyn crate::preferences::Preferences,
    pub receiver: &'a Intake,
    pub mailbox: &'a Mutex<Option<Snapshot>>,
    pub stop: &'a AtomicBool,
    pub control: &'a Control,
    pub language: &'a Cell<Language>,
    pub revision: &'a Cell<u64>,
}
impl CloseContext<'_> {
    pub fn observe(&self, error: &StorageError, root: &std::path::Path) {
        self.control.retain();
        let _fenced = self.receiver.fence();
        let mut detail = error.to_string();
        if !self.stop.load(Ordering::Acquire)
            && let Ok(Some(envelope)) = self.receiver.receive()
        {
            self.revision.set(envelope.revision);
            if let Command::Language(language) = envelope.command {
                let changed = SettingsPresentation::new(self.language.get())
                    .map_err(|_| StorageError::InvalidProfile)
                    .and_then(|mut p| self.preferences.save_language(&mut p, language));
                match changed {
                    Ok(()) => self.language.set(language),
                    Err(error) => detail = error.to_string(),
                }
            }
        }
        let language = self.language.get();
        let settings = super::Snapshot {
            mode: Mode::Recovery,
            current: Some(root.into()),
            revision: self.revision.get(),
            error: Some(format!(
                "{} [{detail}]",
                if language == Language::French {
                    "La fermeture n’a pas pu être confirmée. Les ressources restent conservées ; la récupération dans cette session est indisponible."
                } else {
                    "Closure could not be confirmed. Resources remain retained; recovery in this session is unavailable."
                }
            )),
            recovery_required: true,
            ..Default::default()
        };
        if let Ok(mut mailbox) = self.mailbox.lock() {
            *mailbox = Some(Snapshot {
                workflows: crate::workflows::Snapshot {
                    settings,
                    ..Default::default()
                },
                facets: Default::default(),
                groups: vec![],
                revision: self.revision.get(),
                restore_revision: 0,
                generation: None,
                phase: Phase::Recovery,
                language,
                status: String::new(),
                navigation: None,
                page: None,
            });
        }
    }
}
pub(crate) struct Context<'a, 'p> {
    pub production: Option<(&'a ProfileLocations, &'p NativeProfileStore)>,
    pub preferences: &'a dyn crate::preferences::Preferences,
    pub receiver: &'a Intake,
    pub mailbox: &'a Mutex<Option<Snapshot>>,
    pub stop: &'a AtomicBool,
    pub control: &'a Control,
    pub language: &'a mut Language,
    pub settings: &'a mut SettingsPresentation,
    pub workflows: &'a mut crate::workflows::Snapshot,
    pub navigation: &'a Navigation,
    pub revision: &'a mut u64,
    pub catalog: Option<librairii_adapters::lunii::CatalogClient>,
}
enum Owner<'p> {
    IsolatedClose(Box<librairii_storage::IsolatedCloseFailure>),
    Move {
        failure: Box<RelocationFailure<'p>>,
        session: Option<LibrarySession<'p>>,
    },
    Close {
        failure: Box<NativeCloseFailure<'p>>,
        session: Option<LibrarySession<'p>>,
    },
    Session(LibrarySession<'p>),
}
impl Owner<'_> {
    fn retryable(&self) -> bool {
        match self {
            Self::Move { failure, .. } => failure.commit_status() != CommitStatus::Uncertain,
            Self::Close { failure, .. } => {
                matches!(failure.storage_error(), StorageError::ProfileBusy)
            }
            Self::Session(session) => session.retryable(),
            Self::IsolatedClose(_) => false,
        }
    }
}
fn owner<'p>(
    failure: SessionFailure<'p>,
    session: Option<LibrarySession<'p>>,
) -> Result<Owner<'p>> {
    match failure {
        SessionFailure::IsolatedClose(failure) => Ok(Owner::IsolatedClose(failure)),
        SessionFailure::Move(failure) => Ok(Owner::Move { failure, session }),
        SessionFailure::Close(failure) => Ok(Owner::Close { failure, session }),
        _ => session
            .map(Owner::Session)
            .ok_or(StorageError::RuntimeFenced),
    }
}
/// None certifies actual closure after a stop. Some contains the real resumed
/// owner; source/destination commitment remains determined solely by storage.
pub(crate) fn settle<'p>(
    failure: SessionFailure<'p>,
    session: Option<LibrarySession<'p>>,
    context: &mut Context<'_, 'p>,
) -> Result<Option<LibrarySession<'p>>> {
    let mut detail = failure.to_string();
    let mut held = Some(owner(failure, session)?);
    let source = context.workflows.settings.current.clone();
    let mut attempt = false;
    let _fenced = context.receiver.fence();
    loop {
        let stopping = context.stop.load(Ordering::Acquire);
        if attempt || stopping && held.as_ref().is_some_and(Owner::retryable) {
            context.control.retrying();
            let current = held.take().ok_or(StorageError::RuntimeFenced)?;
            let mut resumed = None;
            let mut closed = false;
            let mut resumed_session = None;
            match current {
                Owner::IsolatedClose(failure) => {
                    let library = failure.into_library();
                    if stopping {
                        match library.finish() {
                            Ok(()) => closed = true,
                            Err(failure) => {
                                detail = failure.to_string();
                                held = Some(Owner::IsolatedClose(failure));
                            }
                        }
                    } else {
                        resumed_session = Some(LibrarySession::isolated(library));
                    }
                }
                Owner::Move { failure, session } => {
                    let result = if stopping
                        && failure.commit_status() == CommitStatus::BeforeCommit
                    {
                        failure.resume_source().map(|(library, cleanup)| {
                            librairii_storage::native::RelocatedLibrary { library, cleanup }
                        })
                    } else {
                        if failure.commit_status() == CommitStatus::BeforeCommit {
                            let cancel = context.control.begin_retry_move();
                            failure.retry_observed(&cancel, context.control)
                        } else {
                            failure.retry_observed(&WorkCancellation::default(), context.control)
                        }
                    };
                    match result {
                        Ok(outcome) => {
                            if outcome.cleanup.pending
                                && source.as_deref() != Some(outcome.library.layout().root())
                            {
                                context.workflows.settings.cleanup_pending = true;
                                context.workflows.settings.retained =
                                    source.iter().cloned().collect();
                            }
                            // The old session has no root: the qualified move
                            // failure held it until this successful resolution.
                            drop(session);
                            resumed = Some(outcome.library);
                        }
                        Err(failure) => {
                            detail = failure.to_string();
                            held = Some(Owner::Move { failure, session });
                        }
                    }
                }
                Owner::Close { failure, session } => match failure.into_library().finish() {
                    Ok(()) => {
                        drop(session);
                        closed = true;
                    }
                    Err(failure) => {
                        detail = failure.to_string();
                        held = Some(Owner::Close { failure, session });
                    }
                },
                Owner::Session(session) => {
                    if !stopping {
                        resumed_session = Some(session);
                    } else {
                        match session.finish() {
                            Ok(()) => closed = true,
                            Err(failure) => {
                                detail = failure.to_string();
                                held = Some(owner(failure, None)?);
                            }
                        }
                    }
                }
            }
            if let Some(native) = resumed {
                if stopping {
                    let mut session =
                        LibrarySession::new(native).with_catalog_client(context.catalog.clone());
                    let language = Cell::new(*context.language);
                    let revision = Cell::new(*context.revision);
                    let close_context = CloseContext {
                        preferences: context.preferences,
                        receiver: context.receiver,
                        mailbox: context.mailbox,
                        stop: context.stop,
                        control: context.control,
                        language: &language,
                        revision: &revision,
                    };
                    let drained = session.drive_inner(
                        context.production.map(|(locations, _)| locations),
                        &Default::default(),
                        |error, root| close_context.observe(error, root),
                        |_| Ok(crate::library::Action::finish(())),
                    );
                    match drained {
                        Ok(()) => match session.finish() {
                            Ok(()) => closed = true,
                            Err(failure) => {
                                detail = failure.to_string();
                                held = Some(owner(failure, None)?);
                            }
                        },
                        Err(failure) => {
                            detail = failure.to_string();
                            held = Some(owner(failure, Some(session))?);
                        }
                    }
                } else {
                    resumed_session = Some(
                        LibrarySession::new(native)
                            .with_catalog_client(context.catalog.clone())
                            .with_cleanup_notice(if context.workflows.settings.cleanup_pending {
                                context.workflows.settings.retained.clone()
                            } else {
                                Vec::new()
                            }),
                    );
                }
            }
            if let Some(session) = resumed_session {
                match context.receiver.resume() {
                    Ok(()) => {
                        context.control.finish();
                        return Ok(Some(session));
                    }
                    Err(_) => {
                        detail = StorageError::RuntimeFenced.to_string();
                        held = Some(Owner::Session(session));
                    }
                }
            }
            if closed {
                context.control.finish();
                if stopping {
                    return Ok(None);
                }
                context
                    .receiver
                    .resume()
                    .map_err(|_| StorageError::RuntimeFenced)?;
                let (locations, store) = context.production.ok_or(StorageError::RuntimeFenced)?;
                let native = NativeLibrary::reopen(store, locations, &Default::default())?;
                return Ok(Some(
                    LibrarySession::new(native)
                        .with_catalog_client(context.catalog.clone())
                        .with_cleanup_notice(if context.workflows.settings.cleanup_pending {
                            context.workflows.settings.retained.clone()
                        } else {
                            Vec::new()
                        }),
                ));
            }
            attempt = false;
        }
        context.control.retain();
        context.workflows.settings.mode = Mode::Recovery;
        context.workflows.settings.can_confirm = false;
        context.workflows.settings.can_retry = held.as_ref().is_some_and(Owner::retryable);
        context.workflows.settings.can_locate = false;
        context.workflows.settings.error = Some(detail.clone());
        if context.workflows.settings.cleanup_pending {
            context.workflows.settings.notice = Some(Notice::CleanupPending);
        }
        if let Some(Owner::Move { failure, .. }) = &held {
            context.workflows.settings.current = failure.authoritative_root().map(PathBuf::from);
            if failure.commit_status() == CommitStatus::Uncertain {
                context.workflows.settings.error = Some(format!(
                    "{} [{}]",
                    if *context.language == Language::French {
                        "L’emplacement de référence attend la résolution de l’enregistrement."
                    } else {
                        "The authoritative location is awaiting commitment resolution."
                    },
                    detail
                ));
            }
        }
        if let Some(Owner::Close { failure, .. }) = &held {
            context.workflows.settings.current =
                Some(failure.retained_library().layout().root().into());
        }
        if let Some(Owner::Move { failure, .. }) = &held
            && let Some(path) = failure.retained_destination()
        {
            let path = PathBuf::from(path);
            if !context.workflows.settings.retained.contains(&path) {
                context.workflows.settings.retained.push(path);
            }
        }
        if let Ok(mut mailbox) = context.mailbox.lock() {
            *mailbox=Some(Snapshot{workflows:context.workflows.clone(),facets:Default::default(),groups:vec![],revision:*context.revision,restore_revision:0,generation:None,phase:Phase::Recovery,language:*context.language,status:if *context.language==Language::French{"Les ressources de la bibliothèque sont conservées. Réessayez après avoir rétabli son emplacement."}else{"Library resources are retained. Restore its location and retry."}.into(),navigation:Some(context.navigation.clone()),page:None});
        }
        if stopping {
            std::thread::sleep(Duration::from_millis(100));
            continue;
        }
        match context.receiver.receive() {
            Ok(Some(envelope)) => {
                *context.revision = envelope.revision;
                match envelope.command {
                    Command::Maintenance {
                        action: Action::Retry,
                        ..
                    } if held.as_ref().is_some_and(Owner::retryable) => attempt = true,
                    Command::Language(language) => match context
                        .preferences
                        .save_language(context.settings, language)
                    {
                        Ok(()) => *context.language = language,
                        Err(error) => detail = error.to_string(),
                    },
                    _ => {}
                }
            }
            Ok(None) => {}
            Err(_) => std::thread::sleep(Duration::from_millis(100)),
        }
    }
}

//! Explicit native setup/recovery. Actual failed attempts remain in this scope.
use crate::{
    maintenance::{Action, Control, Mode, Notice, Snapshot as Settings, Stage},
    presentation::{Command, Phase, Snapshot, intake::Intake},
    startup::StartupSelection,
};
use librairii_app::localization::Language;
use librairii_storage::{
    Result, StorageError,
    database::WorkCancellation,
    native::{NativeCloseFailure, NativeLibrary, NewInstallFailure},
    profile::{NativeProfileStore, ProfileLocations},
    settings::{AdoptionSetupFailure, Startup, TargetPreview},
};
use std::{
    cell::OnceCell,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

enum Held<'p> {
    Schema(Box<librairii_storage::native::SchemaRecovery<'p>>),
    SchemaFailed(Box<librairii_storage::native::SchemaRecoveryFailure<'p>>),
    Active {
        native: Box<NativeLibrary<'p>>,
        error: StorageError,
    },
    Recovery {
        library: Box<librairii_storage::native::RecoveryLibrary<'p>>,
        error: StorageError,
    },
    Setup(Box<NewInstallFailure<'p>>),
    Close(Box<NativeCloseFailure<'p>>),
    Adoption(AdoptionSetupFailure<'p>),
    Move(Box<librairii_storage::native::RelocationFailure<'p>>),
}
impl Held<'_> {
    fn detail(&self) -> String {
        match self {
            Self::Schema(recovery) => recovery.error().unwrap_or("storage.foreign_schema").into(),
            Self::SchemaFailed(failure) => failure.to_string(),
            Self::Active { error, .. } | Self::Recovery { error, .. } => error.to_string(),
            Self::Setup(f) => f.to_string(),
            Self::Close(f) => f.to_string(),
            Self::Adoption(f) => f.to_string(),
            Self::Move(f) => f.to_string(),
        }
    }
    fn retryable(&self) -> bool {
        match self {
            Self::Schema(_) | Self::SchemaFailed(_) => false,
            Self::Active { native, .. } => native.runtime_reusable(),
            Self::Close(f) => matches!(f.storage_error(), StorageError::ProfileBusy),
            Self::Recovery { .. } | Self::Adoption(_) => false,
            Self::Move(f) => {
                f.commit_status() != librairii_storage::native::CommitStatus::Uncertain
            }
            Self::Setup(_) => true,
        }
    }
}
pub(crate) fn run(
    selection: StartupSelection,
    receiver: &Arc<Intake>,
    mailbox: &Arc<Mutex<Option<Snapshot>>>,
    stop: &Arc<AtomicBool>,
    catalog: Option<librairii_adapters::lunii::CatalogClient>,
    control: Control,
) -> Result<()> {
    let StartupSelection::Production {
        locations,
        decision,
    } = selection
    else {
        let StartupSelection::Override(settings) = selection else {
            unreachable!()
        };
        let mut snapshot = empty(Mode::Override, Language::English);
        snapshot.workflows.settings.current = Some(settings.root().into());
        let result = crate::browsing::run_override(
            settings,
            crate::browsing::Context {
                receiver,
                mailbox,
                stop,
                catalog_client: catalog,
                control,
                initial_revision: 0,
                initial_settings: &snapshot.workflows.settings,
            },
        );
        return match result {
            Ok(()) => Ok(()),
            Err(error) => {
                snapshot.workflows.settings.error = Some(error.to_string());
                let preferences = crate::preferences::Memory::default();
                let mut language =
                    librairii_ui::settings::SettingsPresentation::new(Language::English)
                        .map_err(|_| StorageError::InvalidProfile)?;
                crate::presentation::recovery_with_workflows(
                    receiver,
                    mailbox,
                    stop,
                    Language::English,
                    snapshot.workflows,
                    |requested| {
                        crate::preferences::Preferences::save_language(
                            &preferences,
                            &mut language,
                            requested,
                        )
                    },
                )
            }
        };
    };
    let mut input = None;
    let (mode, language, current, proposed, error) = match *decision {
        Startup::ConfirmFolder { suggested } => {
            let saved = librairii_storage::profile::read_legacy(&locations.native)
                .ok()
                .flatten()
                .or_else(|| {
                    locations
                        .legacy
                        .as_ref()
                        .and_then(|p| librairii_storage::profile::read_legacy(p).ok().flatten())
                });
            (
                Mode::Setup,
                saved
                    .and_then(|d| Language::explicit(&d.profile().language).ok())
                    .unwrap_or_else(crate::settings::system_language),
                None,
                Some(suggested),
                None,
            )
        }
        Startup::AdoptIndependent(source) => {
            let language = librairii_storage::profile::read_legacy(&locations.native)
                .ok()
                .flatten()
                .and_then(|d| Language::explicit(&d.profile().language).ok())
                .unwrap_or(
                    Language::explicit(&source.profile().profile().language)
                        .map_err(|_| StorageError::InvalidProfile)?,
                );
            let path = Some(source.root().into());
            input = Some(source);
            (
                Mode::Adoption,
                language,
                path,
                Some(locations.data.join("library")),
                None,
            )
        }
        Startup::Native(document) => (
            Mode::Library,
            Language::explicit(&document.profile().language)
                .map_err(|_| StorageError::InvalidProfile)?,
            Some(PathBuf::from(&document.profile().root)),
            None,
            None,
        ),
        Startup::Recovery { configured, cause } => (
            Mode::Recovery,
            configured
                .as_ref()
                .and_then(|p| Language::explicit(&p.language).ok())
                .unwrap_or_default(),
            configured.map(|p| PathBuf::from(p.root)),
            None,
            Some(cause.to_string()),
        ),
    };
    receiver.fence().map_err(|_| StorageError::RuntimeFenced)?;
    let channels = Channels {
        receiver,
        mailbox,
        stop,
        control: &control,
    };
    let store_cell = OnceCell::new();
    let mut held: Option<Held<'_>> = None;
    let mut target: Option<TargetPreview> = None;
    let mut snapshot = empty(mode, language);
    snapshot.workflows.settings.current = current;
    snapshot.workflows.settings.source = input.as_ref().map(|i| i.root().into());
    snapshot.workflows.settings.proposed = proposed;
    snapshot.workflows.settings.error = error;
    snapshot.workflows.settings.can_confirm = matches!(mode, Mode::Setup | Mode::Adoption);
    snapshot.workflows.settings.can_retry = matches!(mode, Mode::Recovery);
    snapshot.workflows.settings.can_locate = mode == Mode::Recovery;
    let mut open = mode == Mode::Library;
    while !stop.load(Ordering::Acquire)
        || held.is_some()
        || target
            .as_ref()
            .is_some_and(TargetPreview::has_retained_probes)
    {
        if stop.load(Ordering::Acquire) {
            if matches!(held, Some(Held::Schema(_))) {
                if let Some(Held::Schema(observation)) = held.take() {
                    observation.abandon_observation();
                }
                continue;
            }
            // Graceful shutdown is not a successful close of an unsettled
            // setup/copy/move/close owner. Keep every actual resource alive.
            control.retain();
            std::thread::sleep(std::time::Duration::from_millis(100));
            continue;
        }
        if open {
            open = false;
            let entered = existing_store(&store_cell, &locations)
                .map_err(Failure::Storage)
                .and_then(|store| {
                    recover_closed(store, &locations, &channels, &mut snapshot)?;
                    Ok(store)
                });
            match entered {
                Ok(store) => {
                    receiver.resume().map_err(|_| StorageError::RuntimeFenced)?;
                    match crate::browsing::run(
                        &locations,
                        store,
                        crate::browsing::Context {
                            receiver,
                            mailbox,
                            stop,
                            catalog_client: catalog.clone(),
                            control: control.clone(),
                            initial_revision: snapshot.revision,
                            initial_settings: &snapshot.workflows.settings,
                        },
                    ) {
                        Ok(()) => return Ok(()),
                        Err(error) => snapshot.workflows.settings.error = Some(error.to_string()),
                    }
                }
                Err(Failure::Held(failure)) => {
                    snapshot.workflows.settings.error = Some(failure.detail());
                    held = Some(failure);
                }
                Err(Failure::Storage(error)) => {
                    snapshot.workflows.settings.error = Some(error.to_string())
                }
            }
            snapshot.workflows.settings.mode = Mode::Recovery;
            snapshot.workflows.settings.can_confirm = false;
            snapshot.workflows.settings.can_retry = held.as_ref().is_none_or(Held::retryable);
            snapshot.workflows.settings.can_locate = held.is_none();
            snapshot.phase = Phase::Recovery;
            receiver.fence().map_err(|_| StorageError::RuntimeFenced)?;
        }
        project_held(&held, &mut snapshot, &control);
        publish(mailbox, &snapshot)?;
        let Some(envelope) = receiver
            .receive()
            .map_err(|_| StorageError::RuntimeFenced)?
        else {
            continue;
        };
        snapshot.revision = envelope.revision;
        snapshot.workflows.settings.revision = envelope.revision;
        let result = (|| -> Result<()> {
            match envelope.command {
                Command::Language(language) => {
                    let store = prepare_store(&store_cell, &locations)?;
                    let mut presentation =
                        crate::settings::restore_language(store, snapshot.language)?;
                    crate::settings::change_language(store, &mut presentation, language.code())?;
                    snapshot.language = language;
                }
                Command::Refresh => {
                    if held.is_none() && snapshot.workflows.settings.mode == Mode::Recovery {
                        open = true;
                    }
                }
                Command::Maintenance {
                    generation: None,
                    action,
                } if action.bounded() => {
                    snapshot.workflows.settings.error = None;
                    match action {
                        Action::PreserveSchema if matches!(&held,Some(Held::Schema(s)) if s.preview().is_some()) =>
                        {
                            control.begin_schema()?;
                            let Some(Held::Schema(recovery)) = held.take() else {
                                return Err(StorageError::RecoveryBindingRequired);
                            };
                            let store = existing_store(&store_cell, &locations)?;
                            let recovered = recovery.confirm(|error, root| {
                                retain_close(error, root, store, &channels, &snapshot)
                            });
                            control.finish();
                            snapshot.workflows.settings.schema = None;
                            match recovered {
                                Ok((native, paths)) => {
                                    snapshot.workflows.settings.retained = paths;
                                    snapshot.workflows.settings.notice =
                                        Some(Notice::SchemaRecovered);
                                    match native.finish() {
                                        Ok(()) => open = true,
                                        Err(failure) => held = Some(Held::Close(failure)),
                                    }
                                }
                                Err(failure) => held = Some(Held::SchemaFailed(failure)),
                            }
                        }
                        Action::Select(path)
                            if held.is_none()
                                && matches!(
                                    snapshot.workflows.settings.mode,
                                    Mode::Setup | Mode::Adoption
                                ) =>
                        {
                            let cancel = control.begin(Stage::Checking)?;
                            snapshot.workflows.settings.can_confirm = false;
                            let result: Result<()> = (|| {
                                let mut preview = preview(
                                    &path,
                                    &locations,
                                    input.as_ref().map(|i| i.root()),
                                    &cancel,
                                )?;
                                let writable = preview.check_writable(&cancel);
                                snapshot.workflows.settings.recovery_required =
                                    preview.has_retained_probes();
                                snapshot.workflows.settings.proposed =
                                    Some(preview.resolved().into());
                                target = Some(preview);
                                writable?;
                                snapshot.workflows.settings.can_confirm = true;
                                snapshot.workflows.settings.notice = Some(Notice::Selected);
                                Ok(())
                            })();
                            if snapshot.workflows.settings.recovery_required {
                                control.retain();
                            } else {
                                control.finish();
                            }
                            result?;
                        }
                        Action::Confirm
                            if held.is_none() && snapshot.workflows.settings.can_confirm =>
                        {
                            let store = prepare_store(&store_cell, &locations)?;
                            let cancel = control.begin(if input.is_some() {
                                Stage::Adopting
                            } else {
                                Stage::Creating
                            })?;
                            let selected = match target.take() {
                                Some(target) => target,
                                None => preview(
                                    snapshot
                                        .workflows
                                        .settings
                                        .proposed
                                        .as_deref()
                                        .ok_or(StorageError::SettingsDestination)?,
                                    &locations,
                                    input.as_ref().map(|i| i.root()),
                                    &cancel,
                                )?,
                            };
                            let initialized = if let Some(input) = input.take() {
                                librairii_storage::settings::confirm_adoption(
                                    input, selected, store, &locations, &cancel,
                                )
                                .map_err(Held::Adoption)
                            } else {
                                let id = librairii_adapters::identities::job_id()
                                    .map_err(|source| StorageError::Io {
                                        operation: "generate library identity",
                                        source,
                                    })?
                                    .to_string()
                                    .parse()
                                    .map_err(|_| StorageError::InvalidIdentity)?;
                                NativeLibrary::initialize_confirmed(
                                    selected,
                                    store,
                                    &locations,
                                    id,
                                    snapshot.language,
                                    &cancel,
                                )
                                .map_err(Held::Setup)
                            };
                            control.finish();
                            snapshot.workflows.settings.can_confirm = false;
                            match initialized {
                                Ok(native) => {
                                    match close_native(native, store, &channels, &snapshot) {
                                        Ok(()) => open = true,
                                        Err(f) => held = Some(f),
                                    }
                                }
                                Err(Held::Adoption(AdoptionSetupFailure::Preparation {
                                    error,
                                    input: retained_input,
                                    target: retained_target,
                                })) => {
                                    snapshot.workflows.settings.error = Some(error.to_string());
                                    snapshot.workflows.settings.recovery_required =
                                        retained_target.has_retained_probes();
                                    input = Some(*retained_input);
                                    target = Some(*retained_target);
                                    snapshot.workflows.settings.mode = Mode::Adoption;
                                    snapshot.phase = Phase::Adoption;
                                    if snapshot.workflows.settings.recovery_required {
                                        control.retain();
                                    }
                                }
                                Err(f) => held = Some(f),
                            }
                        }
                        Action::Retry if snapshot.workflows.settings.can_retry => {
                            if let Some(failure) = held.take() {
                                let cancel = control.begin_retry()?;
                                let retried = match failure {
                                    Held::Setup(f) => f.retry(&cancel).map_err(Held::Setup),
                                    Held::Active { native, .. } => Ok(*native),
                                    Held::Close(f) => {
                                        match f.into_library().finish() {
                                            Ok(()) => open = true,
                                            Err(f) => held = Some(Held::Close(f)),
                                        };
                                        control.finish();
                                        return Ok(());
                                    }
                                    Held::Move(f) => {
                                        let cancel = if f.commit_status()
                                            == librairii_storage::native::CommitStatus::BeforeCommit
                                        {
                                            control.begin_retry_move()
                                        } else {
                                            cancel
                                        };
                                        f.retry_observed(&cancel, &control)
                                            .map(|o| o.library)
                                            .map_err(Held::Move)
                                    }
                                    other => {
                                        held = Some(other);
                                        control.finish();
                                        return Err(StorageError::RecoveryBindingRequired);
                                    }
                                };
                                control.finish();
                                match retried {
                                    Ok(native) => match close_native(
                                        native,
                                        existing_store(&store_cell, &locations)?,
                                        &channels,
                                        &snapshot,
                                    ) {
                                        Ok(()) => open = true,
                                        Err(f) => held = Some(f),
                                    },
                                    Err(f) => held = Some(f),
                                }
                            } else {
                                open = true;
                            }
                        }
                        Action::Locate(path)
                            if held.is_none() && snapshot.workflows.settings.can_locate =>
                        {
                            let store = existing_store(&store_cell, &locations)?;
                            let cancel = control.begin(Stage::Recovery)?;
                            let located = NativeLibrary::locate_same_uuid_with_report(
                                store, &locations, &path, &cancel,
                            );
                            control.finish();
                            let located = located?;
                            snapshot.workflows.settings.retained = located.retained_move_locations;
                            match close_native(located.library, store, &channels, &snapshot) {
                                Ok(()) => open = true,
                                Err(f) => held = Some(f),
                            }
                        }
                        Action::RetryMove
                            if held.is_none() && snapshot.workflows.settings.interrupted_move =>
                        {
                            let store = existing_store(&store_cell, &locations)?;
                            let cancel = control.begin(Stage::Copying)?;
                            let retried = librairii_storage::native::retry_relocation_observed(
                                store, &locations, &cancel, &control,
                            );
                            control.finish();
                            match retried {
                                Ok(outcome) => {
                                    match close_native(outcome.library, store, &channels, &snapshot)
                                    {
                                        Ok(()) => open = true,
                                        Err(f) => held = Some(f),
                                    }
                                }
                                Err(librairii_storage::native::RestartMoveFailure::Move(f)) => {
                                    held = Some(Held::Move(f))
                                }
                                Err(librairii_storage::native::RestartMoveFailure::Recovery(
                                    error,
                                )) => return Err(error),
                            }
                        }
                        Action::KeepSource
                            if held.is_none() && snapshot.workflows.settings.interrupted_move =>
                        {
                            let store = existing_store(&store_cell, &locations)?;
                            let (_, retained) = librairii_storage::native::cancel_relocation(
                                store,
                                &locations,
                                &Default::default(),
                            )?;
                            snapshot.workflows.settings.retained = vec![retained];
                            snapshot.workflows.settings.notice = Some(Notice::Retained);
                            open = true;
                        }
                        Action::Clear if held.is_none() => {
                            target = None;
                            snapshot.workflows.settings.can_confirm = false;
                        }
                        _ => return Err(StorageError::InvalidNavigation),
                    }
                }
                _ => return Err(StorageError::RuntimeInUse),
            }
            Ok(())
        })();
        if let Err(error) = result {
            snapshot.workflows.settings.error = Some(error.to_string());
            if control.progress().busy && !snapshot.workflows.settings.recovery_required {
                control.finish();
            }
        }
        project_held(&held, &mut snapshot, &control);
    }
    // `held`, target and profile stay alive through the complete interaction.
    Ok(())
}
fn project_held(held: &Option<Held<'_>>, snapshot: &mut Snapshot, control: &Control) {
    if let Some(failure) = held {
        snapshot.workflows.settings.schema = None;
        match failure {
            Held::Schema(recovery) => {
                snapshot.workflows.settings.current = Some(recovery.root().into());
                snapshot.workflows.settings.schema = recovery.preview();
                snapshot.workflows.settings.retained = recovery.retained_paths();
            }
            Held::SchemaFailed(failure) => {
                snapshot.workflows.settings.current = Some(failure.root().into());
                snapshot.workflows.settings.retained = failure.retained_paths();
            }
            Held::Move(f) => {
                snapshot.workflows.settings.current = f.authoritative_root().map(PathBuf::from)
            }
            Held::Close(f) => {
                snapshot.workflows.settings.current =
                    Some(f.retained_library().layout().root().into())
            }
            Held::Recovery { library, .. } => {
                snapshot.workflows.settings.current = Some(library.layout().root().into())
            }
            Held::Active { native, .. } => {
                snapshot.workflows.settings.current = Some(native.layout().root().into())
            }
            _ => {}
        }
        if matches!(failure, Held::Schema(_)) {
            control.finish();
        } else {
            control.retain();
        }
        snapshot
            .workflows
            .settings
            .error
            .get_or_insert_with(|| failure.detail());
        snapshot.workflows.settings.mode = Mode::Recovery;
        snapshot.workflows.settings.can_confirm = false;
        snapshot.workflows.settings.can_retry = failure.retryable();
        snapshot.workflows.settings.can_locate = false;
        snapshot.phase = Phase::Recovery;
    }
}
fn preview(
    path: &Path,
    locations: &ProfileLocations,
    source: Option<&Path>,
    cancel: &WorkCancellation,
) -> Result<TargetPreview> {
    let legacy = librairii_storage::settings::default_legacy_root(locations)?;
    let mut excluded = vec![locations.native.as_path(), legacy.as_path()];
    excluded.extend(locations.legacy.as_deref());
    excluded.extend(source);
    TargetPreview::inspect(path, &excluded, cancel)
}
fn existing_store<'a>(
    cell: &'a OnceCell<NativeProfileStore>,
    locations: &ProfileLocations,
) -> Result<&'a NativeProfileStore> {
    if cell.get().is_none() {
        cell.set(NativeProfileStore::open_native(&locations.native)?)
            .map_err(|_| StorageError::RuntimeFenced)?;
    }
    cell.get().ok_or(StorageError::RuntimeFenced)
}
fn prepare_store<'a>(
    cell: &'a OnceCell<NativeProfileStore>,
    locations: &ProfileLocations,
) -> Result<&'a NativeProfileStore> {
    if cell.get().is_none() {
        cell.set(librairii_storage::settings::prepare_native_profile(
            locations,
            &Default::default(),
        )?)
        .map_err(|_| StorageError::RuntimeFenced)?;
    }
    cell.get().ok_or(StorageError::RuntimeFenced)
}
enum Failure<'p> {
    Storage(StorageError),
    Held(Held<'p>),
}
impl From<StorageError> for Failure<'_> {
    fn from(e: StorageError) -> Self {
        Self::Storage(e)
    }
}
struct Channels<'a> {
    receiver: &'a Intake,
    mailbox: &'a Mutex<Option<Snapshot>>,
    stop: &'a AtomicBool,
    control: &'a Control,
}
fn retain_close(
    error: &StorageError,
    root: &Path,
    store: &NativeProfileStore,
    channels: &Channels<'_>,
    snapshot: &Snapshot,
) -> ! {
    let language = std::cell::Cell::new(snapshot.language);
    let revision = std::cell::Cell::new(snapshot.revision);
    let observer = crate::maintenance::recovery::CloseContext {
        preferences: store,
        receiver: channels.receiver,
        mailbox: channels.mailbox,
        stop: channels.stop,
        control: channels.control,
        language: &language,
        revision: &revision,
    };
    loop {
        observer.observe(error, root);
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}
fn close_native<'p>(
    native: NativeLibrary<'p>,
    store: &NativeProfileStore,
    channels: &Channels<'_>,
    snapshot: &Snapshot,
) -> std::result::Result<(), Held<'p>> {
    match native.has_pending_schema_recovery() {
        Ok(true) => return Err(Held::Schema(Box::new(native.into_schema_recovery()))),
        Err(error) => {
            return Err(Held::Active {
                native: Box::new(native),
                error,
            });
        }
        Ok(false) => {}
    }
    let opened = (|| -> Result<()> {
        let mut database = librairii_storage::database::Database::open_native(&native)?;
        if let Err(error) = database.close() {
            retain_close(&error, native.layout().root(), store, channels, snapshot);
        }
        Ok(())
    })();
    if let Err(error) = opened {
        return Err(Held::Active {
            native: Box::new(native),
            error,
        });
    }
    native.finish().map_err(Held::Close)
}
fn recover_closed<'p>(
    store: &'p NativeProfileStore,
    locations: &ProfileLocations,
    channels: &Channels<'_>,
    snapshot: &mut Snapshot,
) -> std::result::Result<(), Failure<'p>> {
    let control = channels.control;
    let cancel = control.begin(Stage::Recovery)?;
    let result = (|| {
        if let Some(recovery) =
            librairii_storage::native::recover_relocation(store, locations, &cancel)?
        {
            snapshot.workflows.settings.interrupted_move = recovery.authoritative
                == librairii_storage::native::RecoveredRoot::Source
                && recovery.cleanup.pending;
            if recovery.authoritative == librairii_storage::native::RecoveredRoot::Destination
                && recovery.cleanup.pending
            {
                snapshot.workflows.settings.cleanup_pending = true;
                snapshot.workflows.settings.retained = vec![recovery.previous_root];
                snapshot.workflows.settings.notice = Some(Notice::CleanupPending);
            }
            if snapshot.workflows.settings.interrupted_move {
                return Err(Failure::Storage(StorageError::RecoveryBindingRequired));
            }
        }
        match NativeLibrary::reopen(store, locations, &cancel) {
            Ok(native) => close_native(native, store, channels, snapshot).map_err(Failure::Held),
            Err(_) => {
                let recovery = match NativeLibrary::recover_interrupted(store, locations, &cancel) {
                    Ok(recovery) => recovery,
                    Err(StorageError::ForeignSchema) => {
                        let recovery = librairii_storage::native::SchemaRecovery::open(
                            store, locations, &cancel,
                        )?;
                        return Err(Failure::Held(Held::Schema(Box::new(recovery))));
                    }
                    Err(error) => return Err(error.into()),
                };
                match recovery.has_pending_schema_recovery() {
                    Ok(true) => {
                        return Err(Failure::Held(Held::Schema(Box::new(
                            recovery.into_schema_recovery(),
                        ))));
                    }
                    Err(error) => {
                        return Err(Failure::Held(Held::Recovery {
                            library: Box::new(recovery),
                            error,
                        }));
                    }
                    Ok(false) => {}
                }
                let opened = (|| -> Result<()> {
                    let mut database = recovery.open()?;
                    if let Err(error) = database.close() {
                        retain_close(&error, recovery.layout().root(), store, channels, snapshot);
                    }
                    Ok(())
                })();
                if let Err(error) = opened {
                    return Err(Failure::Held(Held::Recovery {
                        library: Box::new(recovery),
                        error,
                    }));
                }
                recovery.finish().map_err(|f| Failure::Held(Held::Close(f)))
            }
        }
    })();
    control.finish();
    if let Ok(Some(document)) = store.load() {
        snapshot.workflows.settings.current = Some(PathBuf::from(&document.profile().root));
    }
    result
}
fn empty(mode: Mode, language: Language) -> Snapshot {
    let phase = match mode {
        Mode::Setup | Mode::Override => Phase::Setup,
        Mode::Adoption => Phase::Adoption,
        _ => Phase::Recovery,
    };
    let workflows = crate::workflows::Snapshot {
        settings: Settings {
            mode,
            ..Default::default()
        },
        ..Default::default()
    };
    Snapshot {
        workflows,
        facets: Default::default(),
        groups: vec![],
        revision: 0,
        restore_revision: 0,
        generation: None,
        phase,
        language,
        status: String::new(),
        navigation: None,
        page: None,
    }
}
fn publish(mailbox: &Mutex<Option<Snapshot>>, snapshot: &Snapshot) -> Result<()> {
    *mailbox.lock().map_err(|_| StorageError::RuntimeFenced)? = Some(snapshot.clone());
    Ok(())
}

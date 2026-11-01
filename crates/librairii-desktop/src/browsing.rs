//! Bounded native query/artwork projection on the controller worker.
use crate::{
    library::{Action, LibrarySession, Runtime},
    presentation::{Command, Phase, Snapshot, intake::Intake},
};
use librairii_adapters::thumbnails::Thumbnail;
use librairii_app::localization::{Catalogs, Language};
use librairii_core::{
    browsing::StorySummary,
    collection_codec::url::{Location, Scope},
    navigation::{Frame, Navigation},
    query::QueryRequest,
};
use librairii_storage::{
    Result, StorageError,
    database::WorkCancellation,
    native::NativeLibrary,
    profile::{NativeProfileStore, ProfileLocations},
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
#[derive(Clone, Debug)]
pub struct Story {
    pub membership: Option<librairii_core::shelves::Member>,
    pub current_sources: Vec<librairii_core::shelves::Source>,
    pub coverage_incomplete: bool,
    pub dates: [Option<librairii_adapters::datetime::LocalDateTime>; 4],
    pub uuid: String,
    pub title: String,
    pub summary: Option<StorySummary>,
    pub thumbnail: Option<Thumbnail>,
    pub provenance: String,
    pub tags: Vec<librairii_core::tagging::DisplayTag>,
}
#[derive(Clone, Debug)]
pub struct Page {
    pub shelf: Option<ShelfContext>,
    pub stories: Vec<Story>,
    pub previews: Vec<GroupPreview>,
    pub preview_page: usize,
    pub preview_pages: usize,
    pub page: i64,
    pub pages: i64,
    pub total: i64,
}
#[derive(Clone, Debug)]
pub struct ShelfContext {
    pub shelf: librairii_core::shelves::Shelf,
    pub sources: Vec<(librairii_core::shelves::Source, bool)>,
    pub exclusion_count: usize,
    pub incomplete: bool,
}
#[derive(Clone, Debug)]
pub struct GroupPreview {
    pub group_index: usize,
    pub name: String,
    pub total: i64,
    pub stories: Vec<Story>,
    pub unavailable: bool,
    pub failed: bool,
}
impl Page {
    pub fn all_stories(&self) -> impl Iterator<Item = &Story> {
        self.stories
            .iter()
            .chain(self.previews.iter().flat_map(|p| p.stories.iter()))
    }
}
#[derive(Clone, Debug)]
pub struct Group {
    pub scope: Scope,
    pub name: String,
    pub count: i64,
    pub available: bool,
}
#[derive(Clone, Debug, Default)]
pub struct Facets {
    pub tags: Vec<librairii_core::tagging::CatalogEntry>,
    pub languages: Vec<String>,
}
fn facets(runtime: &Runtime<'_, '_, '_>) -> Result<Facets> {
    runtime.command(|db, _, _| {
        Ok(Facets {
            tags: db.tag_catalog()?.wait()?,
            languages: db.available_languages()?.wait()?,
        })
    })
}
fn groups(runtime: &Runtime<'_, '_, '_>) -> Result<Vec<Group>> {
    runtime.command(|db, _, _| {
        let summaries = match db
            .collection_summaries("en-GB".into(), Default::default())?
            .wait()?
        {
            librairii_storage::database::TransactionOutcome::Committed(s) => s,
            _ => return Err(StorageError::RuntimeFenced),
        };
        let mut groups = Vec::new();
        for summary in summaries {
            groups.push(Group {
                scope: Scope::Collection(
                    librairii_core::CollectionId::new(summary.inspection.collection.id)
                        .map_err(|_| StorageError::InvalidNavigation)?,
                ),
                name: String::from_utf8_lossy(&summary.inspection.collection.name).into_owned(),
                count: summary.count,
                available: summary.inspection.is_openable(),
            });
        }
        groups.extend(db.shelves()?.wait()?.into_iter().map(|s| Group {
            scope: Scope::Shelf {
                id: s.id,
                manual_order: true,
            },
            name: s.name,
            count: s.count,
            available: true,
        }));
        Ok(groups)
    })
}
fn page(runtime: &Runtime<'_, '_, '_>, location: &Location, stop: &AtomicBool) -> Result<Page> {
    runtime.command(|db, _, cache| {
        let mut shelf_context = None;
        let mut request = location.request.clone();
        let locale = "en-GB".to_owned();
        let (mut result, page, pages, total) = match location.scope {
            Scope::Library | Scope::Custom | Scope::Collection(_) => {
                let p = db.search(request, locale)?.wait()?;
                (
                    p.stories
                        .into_iter()
                        .map(|s| Story {
                            membership: None,
                            current_sources: Vec::new(),
                            coverage_incomplete: false,
                            uuid: s.uuid.clone(),
                            title: s.title.clone(),
                            dates: Default::default(),
                            summary: Some(s),
                            thumbnail: None,
                            provenance: String::new(),
                            tags: Vec::new(),
                        })
                        .collect::<Vec<_>>(),
                    p.page,
                    p.total_pages,
                    p.total_items,
                )
            }
            Scope::Shelf { id, manual_order } => {
                if manual_order {
                    request.sort.clear();
                }
                let p = db.shelf_page(id, request, locale)?.wait()?;
                let evaluated = db.shelf_source_evaluation(id, "en-GB".into())?.wait()?;
                let exclusions = db.shelf_exclusions_page(id, 0)?.wait()?;
                shelf_context = Some(ShelfContext {
                    shelf: p.shelf.clone(),
                    sources: evaluated
                        .sources
                        .iter()
                        .map(|s| (s.source.clone(), s.warning.is_some()))
                        .collect(),
                    exclusion_count: exclusions.total,
                    incomplete: evaluated.incomplete,
                });
                (
                    p.members
                        .into_iter()
                        .map(|m| Story {
                            membership: Some(m.member.clone()),
                            current_sources: evaluated
                                .matching
                                .get(&m.member.uuid)
                                .cloned()
                                .unwrap_or_default(),
                            coverage_incomplete: evaluated.incomplete,
                            uuid: m.member.uuid,
                            title: m
                                .story
                                .as_ref()
                                .map(|s| s.title.clone())
                                .unwrap_or(m.member.cached_title),
                            dates: Default::default(),
                            summary: m.story,
                            thumbnail: None,
                            tags: Vec::new(),
                            provenance: m
                                .member
                                .origins
                                .into_iter()
                                .map(|s| s.name)
                                .collect::<Vec<_>>()
                                .join(", "),
                        })
                        .collect(),
                    p.page as i64,
                    p.pages as i64,
                    p.total as i64,
                )
            }
        };
        let mut labels = db
            .story_tag_labels(
                result
                    .iter()
                    .filter_map(|s| s.summary.as_ref().map(|s| s.id))
                    .collect(),
            )?
            .wait()?;
        let generation = cache.generation()?;
        for story in &mut result {
            if stop.load(Ordering::Acquire) {
                return Err(StorageError::Cancelled);
            }
            if let Some(summary) = &mut story.summary {
                summary.description = librairii_ui::description::readable(&summary.description);
                story.dates[0] = librairii_adapters::datetime::local(&summary.imported_at);
                if let Some(info) = &summary.official {
                    for (index, value) in [
                        &info.fetched_at,
                        &info.activated_at,
                        &info.source_updated_at,
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        story.dates[index + 1] = librairii_adapters::datetime::local(value);
                    }
                }
                story.tags = labels.remove(&summary.id).unwrap_or_default();
                let official = summary
                    .artwork_id
                    .parse()
                    .ok()
                    .and_then(|id| db.read_registered_artwork(id).ok())
                    .and_then(|a| {
                        if a.cached.is_none() {
                            runtime.request_artwork(a.id);
                        }
                        a.cached
                    });
                // The same query snapshot resolved fallback only when both
                // artwork sources were absent. Retain the owned lookup for
                // embedded/official stories, including official-load fallback.
                let embedded =
                    if summary.artwork_id.is_empty() && summary.sources.artwork == "fallback" {
                        None
                    } else {
                        db.read_embedded_artwork(summary.id).ok().flatten()
                    };
                if let librairii_adapters::thumbnails::ArtworkModel::Ready { thumbnail, .. } =
                    cache.resolve(&generation, official.as_ref(), embedded.as_ref())?
                {
                    story.thumbnail = Some(thumbnail);
                }
            }
        }
        Ok(Page {
            shelf: shelf_context,
            stories: result,
            previews: Vec::new(),
            preview_page: 0,
            preview_pages: 0,
            page,
            pages,
            total,
        })
    })
}
fn overview(
    runtime: &Runtime<'_, '_, '_>,
    location: &Location,
    groups: &[Group],
    index: usize,
    stop: &AtomicBool,
) -> Result<Page> {
    let mut loaded = page(runtime, location, stop)?;
    if !matches!(location.scope, Scope::Library) || location.request.criteria != Default::default()
    {
        return Ok(loaded);
    }
    let collections = groups
        .iter()
        .enumerate()
        .filter(|(_, g)| matches!(g.scope, Scope::Collection(_)))
        .collect::<Vec<_>>();
    loaded.preview_pages = collections.len().div_ceil(10);
    loaded.preview_page = index.min(loaded.preview_pages.saturating_sub(1));
    for (group_index, group) in collections
        .into_iter()
        .skip(loaded.preview_page * 10)
        .take(10)
    {
        let mut preview = GroupPreview {
            group_index,
            name: group.name.clone(),
            total: group.count,
            stories: Vec::new(),
            unavailable: true,
            failed: false,
        };
        if let Scope::Collection(id) = group.scope {
            let evaluated = runtime.command(|db, _, _| {
                db.evaluate_collection(
                    id.get(),
                    location.request.clone(),
                    "en-GB".into(),
                    Default::default(),
                )?
                .wait()
            })?;
            use librairii_storage::{collections::Observation, database::TransactionOutcome};
            match evaluated {
                TransactionOutcome::Committed(Observation::Ready(evaluation)) => {
                    let mut request = location.request.clone();
                    request.criteria = evaluation.criteria;
                    request.page = 1;
                    request.page_size = 6;
                    match page(
                        runtime,
                        &Location {
                            scope: group.scope.clone(),
                            request,
                        },
                        stop,
                    ) {
                        Ok(page) => {
                            preview.total = page.total;
                            preview.stories = page.stories;
                            preview.unavailable = false;
                        }
                        Err(StorageError::Cancelled) => return Err(StorageError::Cancelled),
                        Err(StorageError::RuntimeFenced) => {
                            return Err(StorageError::RuntimeFenced);
                        }
                        Err(_) => {
                            preview.unavailable = false;
                            preview.failed = true;
                        }
                    }
                }
                TransactionOutcome::Committed(Observation::NeedsAttention(_)) => {}
                TransactionOutcome::ProvenNotCommitted(StorageError::RuntimeFenced) => {
                    return Err(StorageError::RuntimeFenced);
                }
                TransactionOutcome::ProvenNotCommitted(_) => {
                    preview.unavailable = false;
                    preview.failed = true;
                }
                TransactionOutcome::CommitUncertain(_) => return Err(StorageError::RuntimeFenced),
            }
        }
        loaded.previews.push(preview);
    }
    Ok(loaded)
}
fn push_query(navigation: &mut Navigation, request: QueryRequest) -> Result<()> {
    navigation
        .push(Frame {
            location: Location {
                scope: navigation.current().location.scope.clone(),
                request,
            },
            selection: Vec::new(),
        })
        .map_err(|_| StorageError::InvalidNavigation)
}
fn export_command(
    runtime: &Runtime<'_, '_, '_>,
    navigation: &Navigation,
    operations: &mut crate::workflows::operations::Controller,
    exports: &mut crate::workflows::exports::Controller,
    generation: librairii_app::lifecycle::LibraryGeneration,
    action: crate::workflows::exports::Action,
    revision: u64,
) -> Result<bool> {
    // Polling advances the operation baselines, so these effects cannot be
    // recovered by a later ordinary poll if this command discards them.
    let mut effects = operations.poll(runtime, true)?.1;
    if let Some(id) = exports.handle(runtime, navigation, generation, action, revision) {
        operations.watch(id, revision);
        effects |= operations.poll(runtime, true)?.1;
    }
    Ok(effects)
}
fn refresh_after_operations(
    runtime: &Runtime<'_, '_, '_>,
    navigation: &Navigation,
    workflows: &mut crate::workflows::Snapshot,
    group_rows: &mut Vec<Group>,
    query_facets: &mut Facets,
    collections: &mut crate::workflows::collections::Controller,
    reviews: &mut crate::workflows::shelf_reviews::Controller,
) -> Result<()> {
    crate::workflows::refresh_details(runtime, navigation, workflows);
    *group_rows = groups(runtime)?;
    *query_facets = facets(runtime)?;
    collections.refresh_union(runtime, group_rows)?;
    workflows.collections = collections.snapshot.clone();
    reviews.invalidate();
    workflows.shelf_reviews = reviews.snapshot.clone();
    Ok(())
}
pub(crate) struct Context<'a> {
    pub receiver: &'a Arc<Intake>,
    pub mailbox: &'a Arc<Mutex<Option<Snapshot>>>,
    pub stop: &'a AtomicBool,
    pub catalog_client: Option<librairii_adapters::lunii::CatalogClient>,
    pub control: crate::maintenance::Control,
    pub initial_revision: u64,
    pub initial_settings: &'a crate::maintenance::Snapshot,
}
pub(crate) fn run(
    locations: &ProfileLocations,
    store: &NativeProfileStore,
    context: Context<'_>,
) -> Result<()> {
    let profile = store.load()?.ok_or(StorageError::InvalidProfile)?;
    let language = Language::explicit(&profile.profile().language)
        .map_err(|_| StorageError::InvalidProfile)?;
    let id = librairii_storage::LibraryIdentity::from_bytes(
        profile.profile().library_id.as_bytes().to_vec(),
    )?
    .typed();
    run_session(
        || {
            NativeLibrary::reopen(store, locations, &WorkCancellation::default())
                .map(LibrarySession::new)
        },
        Some((locations, store)),
        store,
        id,
        language,
        context,
    )
}
pub(crate) fn run_override(
    settings: crate::startup::OverrideSettings,
    context: Context<'_>,
) -> Result<()> {
    let id = librairii_adapters::identities::job_id().map_err(|_| StorageError::InvalidIdentity)?;
    let library_id = id
        .to_string()
        .parse()
        .map_err(|_| StorageError::InvalidIdentity)?;
    let preferences = crate::preferences::Memory::default();
    run_session(
        || {
            settings
                .initialize(&id.to_string())
                .map(LibrarySession::isolated)
        },
        None,
        &preferences,
        library_id,
        Language::English,
        context,
    )
}
fn run_session<'p>(
    initialize: impl FnOnce() -> Result<LibrarySession<'p>>,
    production: Option<(&ProfileLocations, &'p NativeProfileStore)>,
    store: &dyn crate::preferences::Preferences,
    id: librairii_core::LibraryId,
    language: Language,
    context: Context<'_>,
) -> Result<()> {
    let Context {
        receiver,
        mailbox,
        stop,
        catalog_client,
        control,
        initial_revision,
        initial_settings,
    } = context;
    let mut navigation = store.load_navigation(id, 1)?.unwrap_or(
        Navigation::new(
            id,
            1,
            Location {
                scope: Scope::Library,
                request: QueryRequest::default(),
            },
        )
        .map_err(|_| StorageError::InvalidNavigation)?,
    );
    store.load_shelf_warnings(id)?;
    let mut language = language;
    let mut settings = librairii_ui::settings::SettingsPresentation::new(language)
        .map_err(|_| StorageError::InvalidProfile)?;
    let catalogs = Catalogs::embedded().map_err(|_| StorageError::InvalidProfile)?;
    let mut revision = initial_revision;
    let mut restore_revision = 0;
    let mut invalid_query = false;
    let mut preview_page = 0usize;
    let mut workflows = crate::workflows::Snapshot::default();
    let mut tag_controller = crate::workflows::tags::Controller::default();
    let mut flag_controller = crate::workflows::flags::Controller::default();
    let mut collection_controller = crate::workflows::collections::Controller::default();
    let mut shelf_controller = crate::workflows::shelves::Controller::default();
    let mut review_controller = crate::workflows::shelf_reviews::Controller::default();
    let mut export_controller = crate::workflows::exports::Controller::default();
    let mut operation_controller = crate::workflows::operations::Controller::default();
    let mut maintenance_controller = crate::maintenance::Controller::default();
    let mut activation = 0u64;
    operation_controller.restore_preferences(store, id)?;
    let close_language = std::cell::Cell::new(language);
    let close_revision = std::cell::Cell::new(revision);
    let close_context = crate::maintenance::recovery::CloseContext {
        preferences: store,
        receiver,
        mailbox,
        stop,
        control: &control,
        language: &close_language,
        revision: &close_revision,
    };
    let mut session = Some(
        initialize()?
            .with_catalog_client(catalog_client.clone())
            .with_cleanup_notice(if initial_settings.cleanup_pending {
                initial_settings.retained.clone()
            } else {
                Vec::new()
            }),
    );
    loop {
        let mut current = session.take().ok_or(StorageError::RuntimeFenced)?;
        let result = current.drive_inner(production.map(|(locations,_)|locations),&WorkCancellation::default(),|error,root|close_context.observe(error,root),|runtime| {
        let active = (|| -> Result<Option<Action<'_, ()>>> {
            let generation = runtime
                .generation()
                .map_err(|_| StorageError::RuntimeFenced)?;
            activation = activation.checked_add(1).ok_or(StorageError::RuntimeFenced)?;
            if activation > 1 {
                navigation = Navigation::decode(&navigation.encode().map_err(|_|StorageError::InvalidNavigation)?, id, activation).map_err(|_|StorageError::InvalidNavigation)?;
                workflows = Default::default();
                tag_controller = Default::default(); flag_controller = Default::default();
                collection_controller = Default::default(); shelf_controller = Default::default();
                review_controller = Default::default(); export_controller = Default::default();
                operation_controller = Default::default();
            }
            operation_controller.restore_preferences(store, id)?;
            maintenance_controller.activate(runtime, activation);
            if activation == 1 && initial_settings.notice == Some(crate::maintenance::Notice::SchemaRecovered) {
                maintenance_controller.snapshot.notice = initial_settings.notice;
                maintenance_controller.snapshot.retained = initial_settings.retained.clone();
            }
            control.finish();
            workflows.settings = maintenance_controller.snapshot.clone();
            let mut group_rows = groups(runtime)?;
            let mut query_facets = facets(runtime)?;
            operation_controller.poll(runtime,true)?;
            workflows.operations=operation_controller.snapshot.clone();
            let mut check_entry = true;
            while !stop.load(Ordering::Acquire) {
                close_language.set(language);close_revision.set(revision);
                // Job progress does not depend on the UI command FIFO becoming
                // empty while a user types or navigates continuously.
                let prepared=export_controller.poll(runtime);workflows.exports=export_controller.snapshot.clone();prepared?;
                let observation=operation_controller.poll(runtime,false);
                workflows.operations=operation_controller.snapshot.clone();let (_,effects)=observation?;
                if effects {
                    refresh_after_operations(runtime,&navigation,&mut workflows,&mut group_rows,&mut query_facets,&mut collection_controller,&mut review_controller)?;
                }
                if check_entry {
                    review_controller.enter(runtime, store, &navigation)?;
                    workflows.shelf_reviews = review_controller.snapshot.clone();
                }
                let loaded = if invalid_query || workflows.recovery_required {
                    Err(StorageError::InvalidNavigation)
                } else {
                    overview(runtime, &navigation.current().location, &group_rows, preview_page, stop)
                };
                let (page, status) = match loaded {
                    Ok(page) => (Some(page), String::new()),
                    Err(StorageError::RuntimeFenced) => return Err(StorageError::RuntimeFenced),
                    Err(StorageError::Cancelled) if stop.load(Ordering::Acquire) => {
                        return Ok(None);
                    }
                    Err(_) => (
                        None,
                        catalogs
                            .text(language, "The story collection could not be loaded.")
                            .map_err(|_| StorageError::InvalidProfile)?
                            .into(),
                    ),
                };
                if !runtime.accepts(&generation) {
                    return Err(StorageError::RuntimeFenced);
                }
                let current_page = page.as_ref().map_or(1, |page| page.page);
                let visible_targets = page
                    .as_ref()
                    .map(|p| {
                        p.all_stories()
                            .filter_map(|s| {
                                s.summary
                                    .as_ref()
                                    .map(|summary| (summary.id, s.uuid.clone()))
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                let unavailable=page.as_ref().map(|p|p.all_stories().filter(|s|s.summary.is_none()).map(|s|s.uuid.clone()).collect::<Vec<_>>()).unwrap_or_default();
                *mailbox.lock().map_err(|_| StorageError::RuntimeFenced)? = Some(Snapshot {
                    workflows: workflows.clone(),
                    facets: query_facets.clone(),
                    groups: group_rows.clone(),
                    revision,
                    restore_revision,
                    generation: Some(generation.clone()),
                    phase: if workflows.recovery_required { Phase::Recovery } else { Phase::Library },
                    language,
                    status: if workflows.recovery_required { if language == Language::French { "La bibliothèque attend une récupération. Reconnectez son volume puis redémarrez Librairii. Les effets confirmés sont conservés." } else { "The library needs recovery. Reconnect its volume, then restart Librairii. Confirmed effects are preserved." }.into() } else if workflows.group_error { if language == Language::French { "L’histoire a été retirée. Les groupes n’ont pas pu être actualisés ; réessayez l’actualisation." } else { "The story was removed. Groups could not be refreshed; retry refresh." }.into() } else { status },
                    navigation: Some(navigation.clone()),
                    page,
                });
                loop {
                    if stop.load(Ordering::Acquire) {
                        return Ok(None);
                    }
                    let Some(envelope) = receiver
                        .receive()
                        .map_err(|_| StorageError::RuntimeFenced)?
                    else {
                        let prepared=export_controller.poll(runtime);workflows.exports=export_controller.snapshot.clone();let export_changed=prepared?;
                        let observed=operation_controller.poll(runtime,false);workflows.operations=operation_controller.snapshot.clone();let (changed,effects)=observed?;
                        let artwork_changed=runtime.observe_artwork()?;
                        if artwork_changed {workflows.artwork_revision=workflows.artwork_revision.checked_add(1).ok_or(StorageError::RuntimeFenced)?;}
                        if changed || export_changed || artwork_changed {
                            workflows.operations=operation_controller.snapshot.clone();
                            if effects {
                                refresh_after_operations(runtime,&navigation,&mut workflows,&mut group_rows,&mut query_facets,&mut collection_controller,&mut review_controller)?;
                            }
                            check_entry=false;
                            break;
                        }
                        continue;
                    };
                    revision = envelope.revision;
                    let command = envelope.command;
                    check_entry = matches!(&command, Command::Navigate(_) | Command::Back | Command::Forward);
                    let refresh_union=matches!(&command,Command::Tags{..}|Command::Remove(_)|Command::Refresh|Command::ShelfReview{action:crate::workflows::shelf_reviews::Action::Apply(_),..})
                        || matches!(&command,Command::Collections{action,..} if !matches!(action,crate::workflows::collections::Action::PreviewUnion(_)|crate::workflows::collections::Action::OpenUnion(_)));
                    if matches!(
                        command,
                        Command::Back | Command::Forward | Command::Navigate(_)
                    ) {
                        restore_revision = revision;
                    }
                    let mut next = navigation.clone();
                    invalid_query = false;
                    let mut invalidates_review=matches!(&command,Command::Refresh);
                    let mut operation_effects=false;
                    match command {
                        Command::Maintenance{generation,action} => {
                            if let crate::maintenance::Action::ImportPreferences(path)=&action {
                                use crate::maintenance::Notice;
                                use librairii_storage::profile::HandoverImport;
                                maintenance_controller.snapshot.error=None;
                                maintenance_controller.snapshot.revision=revision;
                                maintenance_controller.snapshot.notice=Some(Notice::PreferencesRejected);
                                if generation.as_ref().is_some_and(|g|runtime.accepts(g)) && production.is_some() && action.bounded() {
                                    match store.import_handover_file(path,id) {
                                        Ok(result)=>{
                                            maintenance_controller.snapshot.notice=Some(match result {
                                                HandoverImport::Imported=>Notice::PreferencesImported,
                                                HandoverImport::ImportedPreservingNativeChoices=>Notice::PreferencesKept,
                                                HandoverImport::AlreadyImported=>Notice::PreferencesAlreadyImported,
                                            });
                                            if result!=HandoverImport::AlreadyImported {
                                                next=store.load_navigation(id,activation)?.unwrap_or(next);
                                                operation_controller.restore_preferences(store,id)?;
                                                workflows.operations=operation_controller.snapshot.clone();
                                                restore_revision=revision;
                                                check_entry=true;
                                                invalidates_review=true;
                                            }
                                        }
                                        Err(_)=>{store.require_alive()?;}
                                    }
                                }
                                workflows.settings=maintenance_controller.snapshot.clone();
                            } else {
                                let action = maintenance_controller.handle(runtime,&control,generation,action,revision)?;
                                workflows.settings = maintenance_controller.snapshot.clone();
                                if workflows.settings.recovery_required { workflows.recovery_required=true; return Err(StorageError::RuntimeFenced); }
                                if let Some(action) = action { return Ok(Some(action)); }
                            }
                        }
                        Command::Exports{generation,action}=>{
                            operation_effects=export_command(runtime,&next,&mut operation_controller,&mut export_controller,generation,action,revision)?;
                            workflows.operations=operation_controller.snapshot.clone();
                            workflows.exports=export_controller.snapshot.clone();
                            if workflows.exports.recovery_required {workflows.recovery_required=true;receiver.fence().map_err(|_|StorageError::RuntimeFenced)?;return Err(StorageError::RuntimeFenced);}
                        }

                        Command::Operations{generation,action}=>{
                            operation_effects=operation_controller.handle(runtime,store,generation,action,revision);
                            workflows.operations=operation_controller.snapshot.clone();
                            if workflows.operations.recovery_required {workflows.recovery_required=true;receiver.fence().map_err(|_|StorageError::RuntimeFenced)?;return Err(StorageError::RuntimeFenced);}
                        }
                        Command::ShelfReview{generation,action}=>{
                            review_controller.handle(runtime,store,crate::workflows::shelf_reviews::Request{generation,action},revision,&mut next);
                            workflows.shelf_reviews=review_controller.snapshot.clone();
                            if matches!(&workflows.shelf_reviews.feedback,Some(crate::workflows::shelf_reviews::Feedback::Error{uncertain:true,..})) {
                                workflows.recovery_required=true;receiver.fence().map_err(|_|StorageError::RuntimeFenced)?;return Err(StorageError::RuntimeFenced);
                            }
                            if next.current().location!=navigation.current().location {restore_revision=revision;}
                            group_rows=groups(runtime)?;
                        }
                        Command::SelectUnavailable{uuid,query}=>{
                            if next.accepts(&query) && unavailable.contains(&uuid) {
                                workflows.details=None;
                                next.select(Vec::new()).map_err(|_|StorageError::InvalidNavigation)?;
                            }
                        }
                        Command::Shelves{generation,action}=>{
                            shelf_controller.handle(runtime,generation,action,revision,&mut next);
                            workflows.shelves=shelf_controller.snapshot.clone();
                            invalidates_review=matches!(workflows.shelves.feedback,Some(crate::workflows::shelves::Feedback::Saved(_)));
                            if matches!(&workflows.shelves.feedback,Some(crate::workflows::shelves::Feedback::Error{uncertain:true,..})) {
                                workflows.recovery_required=true;receiver.fence().map_err(|_|StorageError::RuntimeFenced)?;return Err(StorageError::RuntimeFenced);
                            }
                            if next.current().location!=navigation.current().location {restore_revision=revision;}
                            group_rows=groups(runtime)?;
                        }
                        Command::GroupPage(delta) => {
                            preview_page = preview_page.saturating_add_signed(delta as isize);
                        }
                        Command::Collections { generation, action } => {
                            collection_controller.handle(runtime,generation,action,revision,&mut next);
                            workflows.collections=collection_controller.snapshot.clone();
                            invalidates_review=matches!(workflows.collections.feedback,Some(crate::workflows::collections::Feedback::Saved(_)|crate::workflows::collections::Feedback::Deleted));
                            if next.current().location != navigation.current().location { restore_revision=revision; }
                            group_rows=groups(runtime)?;
                        }
                        Command::Flags { generation, action } => {
                            flag_controller.handle(runtime, generation, action, revision);
                            workflows.flags = flag_controller.snapshot.clone();
                            if workflows.flags.result.as_ref().is_some_and(|result| result.as_ref().is_err_and(|failure| failure.uncertain)) {
                                workflows.recovery_required = true;
                                receiver.fence().map_err(|_| StorageError::RuntimeFenced)?;
                                // Publish through the recovery path at the
                                // latest revision, including commands drained
                                // by the fence. Retain the uncertain result.
                                return Err(StorageError::RuntimeFenced);
                            }
                            review_controller.observe_flags(&workflows.flags.display);workflows.shelf_reviews=review_controller.snapshot.clone();
                            if let Some(details) = &workflows.details {
                                let detail = crate::workflows::inspect(runtime, &next, details.target.clone());
                                workflows.detail_error = detail.is_err();
                                workflows.details = detail.ok();
                            }
                        }
                        Command::Tags { generation, action } => {
                            tag_controller.handle(runtime,generation,action,revision);
                            workflows.tags=tag_controller.snapshot.clone();
                            invalidates_review=matches!(workflows.tags.feedback,Some(crate::workflows::tags::Feedback::Saved|crate::workflows::tags::Feedback::Deleted|crate::workflows::tags::Feedback::Assignment(_)));
                            query_facets=facets(runtime)?;
                            group_rows=groups(runtime)?;
                        }
                        Command::Details(target) => {
                            let detail = crate::workflows::inspect(runtime, &next, target);
                            workflows.detail_error = detail.is_err();
                            workflows.details = detail.ok();
                            workflows.removal = None;
                        }
                        Command::Remove(target) => {
                            workflows.removal_recovery = false;
                            let mut recovery_required = false;
                            workflows.removal = Some(crate::workflows::remove(runtime, &next, &target, &WorkCancellation::default(), || {
                                recovery_required = true;
                                let _fenced = receiver.fence();
                                let mut recovery = workflows.clone();
                                recovery.removal_recovery = true;
                                if let Ok(mut mailbox) = mailbox.lock() {
                                    *mailbox = Some(Snapshot { workflows: recovery, facets: query_facets.clone(), groups: group_rows.clone(), revision, restore_revision, generation: Some(generation.clone()), phase: Phase::Recovery, language, status: if language == Language::French { "Le retrait attend la restauration de la bibliothèque. Reconnectez son volume ; les opérations restent suspendues." } else { "Removal is waiting for library recovery. Reconnect its volume; library commands remain suspended." }.into(), navigation: Some(next.clone()), page: None });
                                }
                            }).map_err(|e| e.to_string()));
                            workflows.recovery_required = recovery_required;
                            invalidates_review=workflows.removal.as_ref().is_some_and(|r|r.is_ok());
                            if workflows.removal.as_ref().is_some_and(|r| r.is_ok()) {
                                workflows.details = None;
                                group_rows = if recovery_required { Vec::new() } else { match groups(runtime) { Ok(groups) => groups, Err(_) => { workflows.group_error = true; Vec::new() } } };
                            }
                        }
                        Command::Refresh => {
                            if !workflows.recovery_required {
                                group_rows = groups(runtime)?;
                                workflows.group_error = false;
                                query_facets = facets(runtime)?;
                                if let Some(details) = &workflows.details {
                                    let target = details.target.clone();
                                    let detail = crate::workflows::inspect(runtime, &next, target);
                                    workflows.detail_error = detail.is_err();
                                    workflows.details = detail.ok();
                                }
                            }
                        }
                        Command::Back => {
                            next.back().map_err(|_| StorageError::InvalidNavigation)?;
                        }
                        Command::Forward => {
                            next.forward()
                                .map_err(|_| StorageError::InvalidNavigation)?;
                        }
                        Command::Navigate(scope) => {
                            let mut request = QueryRequest::default();
                            if let Scope::Collection(id) = scope {
                                let evaluation = runtime.command(|db, _, _| {
                                    db.evaluate_collection(
                                        id.get(),
                                        request.clone(),
                                        "en-GB".into(),
                                        WorkCancellation::default(),
                                    )?
                                    .wait()
                                })?;
                                let librairii_storage::database::TransactionOutcome::Committed(
                                    librairii_storage::collections::Observation::Ready(evaluation),
                                ) = evaluation
                                else {
                                    return Err(StorageError::InvalidNavigation);
                                };
                                request.criteria = evaluation.criteria;
                            }
                            next.push(Frame {
                                location: Location { scope, request },
                                selection: Vec::new(),
                            })
                            .map_err(|_| StorageError::InvalidNavigation)?;
                        }
                        Command::Language(requested) => {
                            store.save_language(&mut settings,requested)?;
                            language = requested;
                        }
                        Command::Presentation(mode) => next.set_presentation(mode),
                        Command::ChangePage(delta) => {
                            let mut request = next.current().location.request.clone();
                            request.page = current_page.saturating_add(i64::from(delta)).max(1);
                            invalid_query = push_query(&mut next, request).is_err();
                        }
                        Command::Request(request) => {
                            invalid_query = push_query(&mut next, request).is_err();
                        }
                        Command::Sort {
                            request,
                            manual_order,
                        } => {
                            let scope = match next.current().location.scope.clone() {
                                Scope::Shelf { id, .. } => Scope::Shelf { id, manual_order },
                                scope => scope,
                            };
                            invalid_query = next
                                .push(Frame {
                                    location: Location { scope, request },
                                    selection: Vec::new(),
                                })
                                .is_err();
                        }
                        Command::Select { id, query } => {
                            if next.accepts(&query) {
                                if let Some((_, uuid)) =
                                    visible_targets.iter().find(|(story_id, _)| *story_id == id)
                                {
                                    workflows.details = crate::workflows::inspect(
                                        runtime,
                                        &next,
                                        crate::workflows::StoryTarget {
                                            generation: generation.clone(),
                                            query: query.clone(),
                                            id,
                                            uuid: uuid.clone(),
                                        },
                                    )
                                    .ok();
                                }
                                next.select(vec![id])
                                    .map_err(|_| StorageError::InvalidNavigation)?;
                            }
                        }
                    }
                    if operation_effects {
                        refresh_after_operations(runtime,&next,&mut workflows,&mut group_rows,&mut query_facets,&mut collection_controller,&mut review_controller)?;
                    }
                    if invalidates_review {review_controller.invalidate();workflows.shelf_reviews=review_controller.snapshot.clone();}
                    if !operation_effects && (refresh_union || invalidates_review) && !workflows.recovery_required {
                        let refreshed=collection_controller.refresh_union(runtime,&group_rows);
                        workflows.collections=collection_controller.snapshot.clone();
                        refreshed?;
                    }
                    if !invalid_query {
                        if next != navigation { store.save_navigation(&next)?; }
                        navigation = next;
                    }
                    break;
                }
            }
            Ok(None)
        })();
        match active {
            Ok(Some(action)) => return Ok(action),
            Ok(None) => {},
            Err(error) => {
                // Keep the initiating cause in process diagnostics; the public
                // recovery presentation continues to use localized guidance.
                eprintln!("native_controller_recovery; revision={revision}; cause={error:?}");
                crate::presentation::recovery_with_workflows(receiver, mailbox, stop, language, workflows.clone(), |requested| {
                    store.save_language(&mut settings, requested)
                })?;
            }
        }
        Ok(Action::finish(()))
    });
        let (failure, retained) = match result {
            Ok(()) => match current.finish() {
                Ok(()) => return Ok(()),
                Err(failure) => (failure, None),
            },
            Err(failure) => (failure, Some(current)),
        };
        let mut recovery = crate::maintenance::recovery::Context {
            production,
            preferences: store,
            receiver,
            mailbox,
            stop,
            control: &control,
            language: &mut language,
            settings: &mut settings,
            workflows: &mut workflows,
            navigation: &navigation,
            revision: &mut revision,
            catalog: catalog_client.clone(),
        };
        match crate::maintenance::recovery::settle(failure, retained, &mut recovery)? {
            Some(resumed) => session = Some(resumed),
            None => return Ok(()),
        }
    }
}

#[cfg(test)]
mod tests;

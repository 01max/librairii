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
    pub stories: Vec<Story>,
    pub previews: Vec<GroupPreview>,
    pub preview_page: usize,
    pub preview_pages: usize,
    pub page: i64,
    pub pages: i64,
    pub total: i64,
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
        let mut request = location.request.clone();
        let locale = "en-GB".to_owned();
        let (mut result, page, pages, total) = match location.scope {
            Scope::Library | Scope::Custom | Scope::Collection(_) => {
                let p = db.search(request, locale)?.wait()?;
                (
                    p.stories
                        .into_iter()
                        .map(|s| Story {
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
                (
                    p.members
                        .into_iter()
                        .map(|m| Story {
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
                    .and_then(|a| a.cached);
                let embedded = db.read_embedded_artwork(summary.id).ok().flatten();
                if let librairii_adapters::thumbnails::ArtworkModel::Ready { thumbnail, .. } =
                    cache.resolve(&generation, official.as_ref(), embedded.as_ref())?
                {
                    story.thumbnail = Some(thumbnail);
                }
            }
        }
        Ok(Page {
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
pub(crate) fn run(
    locations: &ProfileLocations,
    language: Language,
    receiver: &Arc<Intake>,
    mailbox: &Arc<Mutex<Option<Snapshot>>>,
    stop: &AtomicBool,
) -> Result<()> {
    let store = NativeProfileStore::open_native(&locations.native)?;
    let profile = store.load()?.ok_or(StorageError::InvalidProfile)?;
    let id = librairii_storage::LibraryIdentity::from_bytes(
        profile.profile().library_id.as_bytes().to_vec(),
    )?
    .typed();
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
    let native = NativeLibrary::reopen(&store, locations, &WorkCancellation::default())?;
    let mut language = language;
    let mut settings = crate::settings::restore_language(&store, language)?;
    let catalogs = Catalogs::embedded().map_err(|_| StorageError::InvalidProfile)?;
    let mut session = LibrarySession::new(native);
    let mut revision = 0;
    let mut restore_revision = 0;
    let mut invalid_query = false;
    let mut preview_page = 0usize;
    let mut workflows = crate::workflows::Snapshot::default();
    let mut tag_controller = crate::workflows::tags::Controller::default();
    let mut flag_controller = crate::workflows::flags::Controller::default();
    let mut collection_controller = crate::workflows::collections::Controller::default();
    let result = session.drive(locations, &WorkCancellation::default(), |runtime| {
        let active = (|| -> Result<()> {
            let generation = runtime
                .generation()
                .map_err(|_| StorageError::RuntimeFenced)?;
            let mut group_rows = groups(runtime)?;
            let mut query_facets = facets(runtime)?;
            while !stop.load(Ordering::Acquire) {
                let loaded = if invalid_query || workflows.recovery_required {
                    Err(StorageError::InvalidNavigation)
                } else {
                    overview(runtime, &navigation.current().location, &group_rows, preview_page, stop)
                };
                let (page, status) = match loaded {
                    Ok(page) => (Some(page), String::new()),
                    Err(StorageError::RuntimeFenced) => return Err(StorageError::RuntimeFenced),
                    Err(StorageError::Cancelled) if stop.load(Ordering::Acquire) => {
                        return Ok(());
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
                        return Ok(());
                    }
                    let Some(envelope) = receiver
                        .receive()
                        .map_err(|_| StorageError::RuntimeFenced)?
                    else {
                        continue;
                    };
                    revision = envelope.revision;
                    let command = envelope.command;
                    let refresh_union=matches!(&command,Command::Tags{..}|Command::Remove(_)|Command::Refresh)
                        || matches!(&command,Command::Collections{action,..} if !matches!(action,crate::workflows::collections::Action::PreviewUnion(_)|crate::workflows::collections::Action::OpenUnion(_)));
                    if matches!(
                        command,
                        Command::Back | Command::Forward | Command::Navigate(_)
                    ) {
                        restore_revision = revision;
                    }
                    let mut next = navigation.clone();
                    invalid_query = false;
                    match command {
                        Command::GroupPage(delta) => {
                            preview_page = preview_page.saturating_add_signed(delta as isize);
                        }
                        Command::Collections { generation, action } => {
                            collection_controller.handle(runtime,generation,action,revision,&mut next);
                            workflows.collections=collection_controller.snapshot.clone();
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
                            if let Some(details) = &workflows.details {
                                let detail = crate::workflows::inspect(runtime, &next, details.target.clone());
                                workflows.detail_error = detail.is_err();
                                workflows.details = detail.ok();
                            }
                        }
                        Command::Tags { generation, action } => {
                            tag_controller.handle(runtime,generation,action,revision);
                            workflows.tags=tag_controller.snapshot.clone();
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
                            crate::settings::change_language(
                                &store,
                                &mut settings,
                                requested.code(),
                            )?;
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
                    if refresh_union && !workflows.recovery_required {
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
            Ok(())
        })();
        if active.is_err() {
            crate::presentation::recovery_with_workflows(receiver, mailbox, stop, language, workflows.clone(), |requested| {
                crate::settings::change_language(&store, &mut settings, requested.code())
            })?;
        }
        Ok(Action::finish(()))
    });
    let closed = session.finish();
    if let Err(retained) = closed {
        crate::presentation::recovery_with_workflows(
            receiver,
            mailbox,
            stop,
            language,
            workflows.clone(),
            |requested| crate::settings::change_language(&store, &mut settings, requested.code()),
        )?;
        drop(retained);
        return Err(StorageError::RuntimeFenced);
    }
    result.map_err(|_| StorageError::RuntimeFenced)?;
    Ok(())
}

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
    pub page: i64,
    pub pages: i64,
    pub total: i64,
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
    runtime.command(|db, _, _| Ok(Facets {
        tags: db.tag_catalog()?.wait()?,
        languages: db.read(|tx| {
            let mut statement=tx.prepare("SELECT DISTINCT m.language FROM official_story_metadata m JOIN catalog_snapshots c ON c.id=m.snapshot_id WHERE c.status='active' AND m.language<>'' ORDER BY m.language")?;
            let rows=statement.query_map([],|row|row.get::<_,String>(0))?.collect::<std::result::Result<Vec<_>,_>>()?;
            Ok(rows)
        })?.wait()?,
    }))
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
            if let Some(summary) = &story.summary {
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
            page,
            pages,
            total,
        })
    })
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
    let result = session.drive(locations, &WorkCancellation::default(), |runtime| {
        let active = (|| -> Result<()> {
            let generation = runtime
                .generation()
                .map_err(|_| StorageError::RuntimeFenced)?;
            let mut group_rows = groups(runtime)?;
            let mut query_facets = facets(runtime)?;
            while !stop.load(Ordering::Acquire) {
                let loaded = if invalid_query {
                    Err(StorageError::InvalidNavigation)
                } else {
                    page(runtime, &navigation.current().location, stop)
                };
                let (page, status) = match loaded {
                    Ok(page) => (Some(page), String::new()),
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
                *mailbox.lock().map_err(|_| StorageError::RuntimeFenced)? = Some(Snapshot {
                    facets: query_facets.clone(),
                    groups: group_rows.clone(),
                    revision,
                    restore_revision,
                    generation: Some(generation.clone()),
                    phase: Phase::Library,
                    language,
                    status,
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
                    if matches!(
                        command,
                        Command::Back | Command::Forward | Command::Navigate(_)
                    ) {
                        restore_revision = revision;
                    }
                    let mut next = navigation.clone();
                    invalid_query = false;
                    match command {
                        Command::Refresh => {
                            group_rows = groups(runtime)?;
                            query_facets = facets(runtime)?;
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
                        Command::Select { id, query } => {
                            if next.accepts(&query) {
                                next.select(vec![id])
                                    .map_err(|_| StorageError::InvalidNavigation)?;
                            }
                        }
                    }
                    if !invalid_query {
                        store.save_navigation(&next)?;
                        navigation = next;
                    }
                    break;
                }
            }
            Ok(())
        })();
        if active.is_err() {
            crate::presentation::recovery(receiver, mailbox, stop, language, |requested| {
                crate::settings::change_language(&store, &mut settings, requested.code())
            })?;
        }
        Ok(Action::finish(()))
    });
    let closed = session.finish();
    if let Err(retained) = closed {
        crate::presentation::recovery(receiver, mailbox, stop, language, |requested| {
            crate::settings::change_language(&store, &mut settings, requested.code())
        })?;
        drop(retained);
        return Err(StorageError::RuntimeFenced);
    }
    result.map_err(|_| StorageError::RuntimeFenced)?;
    Ok(())
}

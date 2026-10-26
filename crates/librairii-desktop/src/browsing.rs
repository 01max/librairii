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
        let request = location.request.clone();
        let locale = "en-GB".to_owned();
        let (mut result, page, pages, total) = match location.scope {
            Scope::Library | Scope::Custom => {
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
                        })
                        .collect::<Vec<_>>(),
                    p.page,
                    p.total_pages,
                    p.total_items,
                )
            }
            Scope::Collection(id) => {
                let p = match db
                    .evaluate_collection(id.get(), request, locale, WorkCancellation::default())?
                    .wait()?
                {
                    librairii_storage::database::TransactionOutcome::Committed(
                        librairii_storage::collections::Observation::Ready(e),
                    ) => e.page,
                    _ => return Err(StorageError::InvalidNavigation),
                };
                (
                    p.stories
                        .into_iter()
                        .map(|s| Story {
                            uuid: s.uuid.clone(),
                            title: s.title.clone(),
                            summary: Some(s),
                            thumbnail: None,
                            provenance: String::new(),
                        })
                        .collect(),
                    p.page,
                    p.total_pages,
                    p.total_items,
                )
            }
            Scope::Shelf { id, .. } => {
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
        let generation = cache.generation()?;
        for story in &mut result {
            if stop.load(Ordering::Acquire) {
                return Err(StorageError::Cancelled);
            }
            if let Some(summary) = &story.summary {
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
pub(crate) fn run(
    locations: &ProfileLocations,
    language: Language,
    receiver: &Arc<Intake>,
    mailbox: &Arc<Mutex<Option<Snapshot>>>,
    stop: &AtomicBool,
) -> Result<()> {
    let store = NativeProfileStore::open_native(&locations.native)?;
    let native = NativeLibrary::reopen(&store, locations, &WorkCancellation::default())?;
    let id = native.identity().typed();
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
    let mut language = language;
    let mut settings = crate::settings::restore_language(&store, language)?;
    let catalogs = Catalogs::embedded().map_err(|_| StorageError::InvalidProfile)?;
    let mut session = LibrarySession::new(native);
    let mut revision = 0;
    let mut restore_navigation = true;
    let result = session.drive(locations, &WorkCancellation::default(), |runtime| {
        let generation = runtime
            .generation()
            .map_err(|_| StorageError::RuntimeFenced)?;
        let mut group_rows = groups(runtime)?;
        while !stop.load(Ordering::Acquire) {
            let loaded = page(runtime, &navigation.current().location, stop);
            let (page, status) = match loaded {
                Ok(page) => (Some(page), String::new()),
                Err(StorageError::Cancelled) if stop.load(Ordering::Acquire) => {
                    return Ok(Action::finish(()));
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
            *mailbox.lock().map_err(|_| StorageError::RuntimeFenced)? = Some(Snapshot {
                groups: group_rows.clone(),
                revision,
                restore_navigation,
                generation: Some(generation.clone()),
                phase: Phase::Library,
                language,
                status,
                navigation: Some(navigation.clone()),
                page,
            });
            loop {
                if stop.load(Ordering::Acquire) {
                    return Ok(Action::finish(()));
                }
                let Some(envelope) = receiver
                    .receive()
                    .map_err(|_| StorageError::RuntimeFenced)?
                else {
                    continue;
                };
                revision = envelope.revision;
                let command = envelope.command;
                restore_navigation = matches!(
                    command,
                    Command::Back | Command::Forward | Command::Navigate(_)
                );
                let mut next = navigation.clone();
                match command {
                    Command::Refresh => {
                        group_rows = groups(runtime)?;
                    }
                    Command::Back => {
                        next.back().map_err(|_| StorageError::InvalidNavigation)?;
                    }
                    Command::Forward => {
                        next.forward()
                            .map_err(|_| StorageError::InvalidNavigation)?;
                    }
                    Command::Navigate(scope) => {
                        next.push(Frame {
                            location: Location {
                                scope,
                                request: Default::default(),
                            },
                            selection: Vec::new(),
                        })
                        .map_err(|_| StorageError::InvalidNavigation)?;
                    }
                    Command::Language(requested) => {
                        crate::settings::change_language(&store, &mut settings, requested.code())?;
                        language = requested;
                    }
                    Command::Presentation(mode) => next.set_presentation(mode),
                    Command::Request(request) => next
                        .push(Frame {
                            location: Location {
                                scope: next.current().location.scope.clone(),
                                request,
                            },
                            selection: Vec::new(),
                        })
                        .map_err(|_| StorageError::InvalidNavigation)?,
                    Command::Select { id, query } => {
                        if next.accepts(&query) {
                            next.select(vec![id])
                                .map_err(|_| StorageError::InvalidNavigation)?;
                        }
                    }
                }
                store.save_navigation(&next)?;
                navigation = next;
                break;
            }
        }
        Ok(Action::finish(()))
    });
    let closed = session.finish();
    result.map_err(|_| StorageError::RuntimeFenced)?;
    closed.map_err(|_| StorageError::RuntimeFenced)
}

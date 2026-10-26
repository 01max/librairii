//! Bounded native query/artwork projection on the controller worker.
use crate::{
    library::{Action, LibrarySession, Runtime},
    presentation::{Command, Phase, Snapshot},
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
    mpsc,
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
fn page(
    runtime: &Runtime<'_, '_, '_>,
    location: &Location,
    language: Language,
    stop: &AtomicBool,
) -> Result<Page> {
    runtime.command(|db, _, cache| {
        let request = location.request.clone();
        let locale = language.code().to_owned();
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
    receiver: &mpsc::Receiver<Command>,
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
    let catalogs = Catalogs::embedded().map_err(|_| StorageError::InvalidProfile)?;
    let mut session = LibrarySession::new(native);
    let result = session.drive(locations, &WorkCancellation::default(), |runtime| {
        while !stop.load(Ordering::Acquire) {
            let loaded = page(runtime, &navigation.current().location, language, stop);
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
            *mailbox.lock().map_err(|_| StorageError::RuntimeFenced)? = Some(Snapshot {
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
                let command = match receiver.recv_timeout(std::time::Duration::from_millis(20)) {
                    Ok(c) => c,
                    Err(mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(Action::finish(())),
                };
                let mut next = navigation.clone();
                match command {
                    Command::Refresh => {}
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
                    Command::Select(id) => next
                        .select(vec![id])
                        .map_err(|_| StorageError::InvalidNavigation)?,
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

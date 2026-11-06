//! Frozen native export intent, opaque preview tickets and bounded presentation.
//! Every path is a fresh picker input; persisted destination labels are reveal-only.
use crate::library::Runtime;
use librairii_app::lifecycle::LibraryGeneration;
use librairii_core::{
    CollectionId, ShelfId,
    browsing::StoryIdentity,
    collection_codec::url::{Location, Scope as NavigationScope},
    export::{Issue, PreparationId, Preview, PreviewItem, Scope},
    navigation::Navigation,
};
use librairii_storage::{Result, StorageError};
use std::{collections::BTreeSet, path::PathBuf, sync::Arc};
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Mode {
    #[default]
    Results,
    Selection,
    Collections,
    Shelves,
}
#[derive(Clone, Debug)]
pub struct Ticket(Arc<PreparationId>);
impl PartialEq for Ticket {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for Ticket {}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Action {
    Open(Vec<StoryIdentity>),
    Select { mode: Mode, groups: Vec<i64> },
    Destination(PathBuf),
    Refresh,
    Page(i32),
    Start(Ticket),
    Reveal(String),
    Close,
}
impl Action {
    pub fn bounded(&self) -> bool {
        match self {
            Self::Open(rows) => rows
                .iter()
                .try_fold(0usize, |n, r| n.checked_add(r.uuid.capacity() + 64))
                .is_some_and(|n| n <= 1024 * 1024),
            Self::Select { groups, .. } => groups.capacity().saturating_mul(8) <= 1024 * 1024,
            Self::Destination(path) => path.as_os_str().len() <= 16 * 1024,
            Self::Reveal(id) => !id.is_empty() && id.len() <= 256,
            _ => true,
        }
    }
}
#[derive(Clone, Debug)]
pub struct Page {
    pub ticket: Option<Ticket>,
    pub intended: usize,
    pub ready: usize,
    pub bytes: i64,
    pub destination: String,
    pub formats: Vec<String>,
    pub overlap: usize,
    pub issue: Option<Issue>,
    pub items: Vec<PreviewItem>,
    pub page: usize,
    pub pages: usize,
}
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub revision: u64,
    pub observation: u64,
    pub preparing: bool,
    pub mode: Mode,
    pub groups: Vec<i64>,
    pub preview: Option<Page>,
    pub has_destination: bool,
    pub has_selection: bool,
    pub error: Option<String>,
    pub started: Option<String>,
    pub revealed: bool,
    pub recovery_required: bool,
}
#[derive(Debug)]
struct Context {
    location: Location,
    selection: Vec<StoryIdentity>,
}
#[derive(Debug, Default)]
pub struct Controller {
    pub snapshot: Snapshot,
    context: Option<Context>,
    destination: Option<PathBuf>,
    preview: Option<Preview>,
    ticket: Option<Ticket>,
    page: usize,
    pending: Option<u64>,
}
impl Controller {
    fn discard(&mut self, runtime: &Runtime<'_, '_, '_>) -> Result<()> {
        runtime.cancel_export_preparation();
        self.pending = None;
        self.snapshot.preparing = false;
        if let Some(id) = self.preview.take().and_then(|p| p.preparation) {
            runtime.discard_export_preparation(id)?;
        }
        self.ticket = None;
        self.snapshot.preview = None;
        self.page = 0;
        Ok(())
    }
    fn scope(&self) -> Result<Scope> {
        let context = self.context.as_ref().ok_or(StorageError::SnapshotChanged)?;
        Ok(match self.snapshot.mode {
            Mode::Results => match context.location.scope {
                NavigationScope::Shelf { id, .. } => Scope::ShelfResult {
                    shelf: id,
                    query: context.location.request.clone(),
                },
                _ => Scope::CurrentQuery(context.location.request.clone()),
            },
            Mode::Selection => {
                if context.selection.is_empty() {
                    return Err(librairii_core::export::ExportError::InvalidScope.into());
                }
                match context.location.scope {
                    NavigationScope::Shelf { id, .. } => Scope::ShelfSelection {
                        shelf: id,
                        uuids: context.selection.iter().map(|s| s.uuid.clone()).collect(),
                    },
                    _ => Scope::Selection(context.selection.iter().map(|s| s.id).collect()),
                }
            }
            Mode::Collections => Scope::Collections(
                self.snapshot
                    .groups
                    .iter()
                    .map(|id| CollectionId::new(*id).map_err(|_| StorageError::SnapshotChanged))
                    .collect::<Result<Vec<_>>>()?,
            ),
            Mode::Shelves => Scope::Shelves(
                self.snapshot
                    .groups
                    .iter()
                    .map(|id| ShelfId::new(*id).map_err(|_| StorageError::SnapshotChanged))
                    .collect::<Result<Vec<_>>>()?,
            ),
        })
    }
    fn prepare(&mut self, runtime: &Runtime<'_, '_, '_>) -> Result<()> {
        self.discard(runtime)?;
        let scope = self.scope()?;
        let path = self
            .destination
            .clone()
            .ok_or(StorageError::SnapshotChanged)?;
        self.pending = Some(runtime.prepare_export(scope, path)?);
        self.snapshot.preparing = true;
        Ok(())
    }
    pub fn poll(&mut self, runtime: &Runtime<'_, '_, '_>) -> Result<bool> {
        let result = runtime.take_export_preparation();
        if let Err(error) = &result {
            self.snapshot.recovery_required = true;
            self.snapshot.preparing = false;
            self.snapshot.error = Some(error.code().into());
        }
        let Some((serial, result)) = result? else {
            return Ok(false);
        };
        if self.pending != Some(serial) {
            if let Ok(preview) = result
                && let Some(id) = preview.preparation
            {
                runtime.discard_export_preparation(id)?;
            }
            return Ok(false);
        }
        self.pending = None;
        self.snapshot.preparing = false;
        self.snapshot.observation = self
            .snapshot
            .observation
            .checked_add(1)
            .ok_or(StorageError::RuntimeFenced)?;
        let result = result.and_then(|preview| {
            if self.snapshot.mode == Mode::Selection {
                let expected = &self
                    .context
                    .as_ref()
                    .ok_or(StorageError::SnapshotChanged)?
                    .selection;
                if expected.len() != preview.items.len()
                    || expected
                        .iter()
                        .map(|s| s.uuid.as_str())
                        .collect::<BTreeSet<_>>()
                        != preview
                            .items
                            .iter()
                            .map(|s| s.uuid.as_str())
                            .collect::<BTreeSet<_>>()
                {
                    if let Some(id) = preview.preparation {
                        runtime.discard_export_preparation(id)?;
                    }
                    return Err(StorageError::SnapshotChanged);
                }
            }
            self.ticket = preview.preparation.clone().map(|p| Ticket(Arc::new(p)));
            self.preview = Some(preview);
            self.publish();
            Ok(())
        });
        if let Err(error) = result {
            self.snapshot.error = Some(error.code().into());
            if matches!(
                error,
                StorageError::RuntimeFenced | StorageError::CustodyRecoveryRequired
            ) {
                return Err(error);
            }
        }
        Ok(true)
    }
    fn publish(&mut self) {
        self.snapshot.preview = self.preview.as_ref().map(|p| {
            let pages = p.items.len().div_ceil(25).max(1);
            self.page = self.page.min(pages - 1);
            Page {
                ticket: self.ticket.clone(),
                intended: p.items.len(),
                ready: p.ready,
                bytes: p.bytes,
                destination: p.destination_label.to_string_lossy().into_owned(),
                formats: p.formats.clone(),
                overlap: p.collapsed_overlap,
                issue: p.issue,
                items: p
                    .items
                    .iter()
                    .skip(self.page * 25)
                    .take(25)
                    .cloned()
                    .collect(),
                page: self.page,
                pages,
            }
        });
    }
    pub fn handle(
        &mut self,
        runtime: &Runtime<'_, '_, '_>,
        navigation: &Navigation,
        generation: LibraryGeneration,
        action: Action,
        revision: u64,
    ) -> Option<String> {
        let start_attempt = matches!(&action, Action::Start(_));
        self.snapshot.revision = revision;
        self.snapshot.error = None;
        self.snapshot.started = None;
        self.snapshot.revealed = false;
        let result = (|| -> Result<Option<String>> {
            if !runtime.accepts(&generation) || !action.bounded() {
                return Err(StorageError::SnapshotChanged);
            }
            match action {
                Action::Open(selection) => {
                    self.discard(runtime)?;
                    self.snapshot = Snapshot {
                        revision,
                        has_selection: !selection.is_empty(),
                        ..Default::default()
                    };
                    self.context = Some(Context {
                        location: navigation.current().location.clone(),
                        selection,
                    });
                    self.destination = None;
                }
                Action::Select { mode, groups } => {
                    self.discard(runtime)?;
                    self.snapshot.mode = mode;
                    self.snapshot.groups = groups;
                    if self.destination.is_some() {
                        self.prepare(runtime)?;
                    }
                }
                Action::Destination(path) => {
                    self.destination = Some(path);
                    self.snapshot.has_destination = true;
                    self.prepare(runtime)?;
                }
                Action::Refresh => self.prepare(runtime)?,
                Action::Page(delta) => {
                    self.page = self.page.saturating_add_signed(delta as isize);
                    self.publish();
                }
                Action::Start(ticket) => {
                    let actual = self
                        .ticket
                        .as_ref()
                        .filter(|p| **p == ticket)
                        .ok_or(StorageError::SnapshotChanged)?;
                    let id = librairii_adapters::identities::job_id()
                        .map_err(|source| StorageError::Io {
                            operation: "create export identity",
                            source,
                        })?
                        .to_string();
                    runtime.offer_export(&actual.0, id.clone())?;
                    self.ticket = None;
                    self.preview = None;
                    self.snapshot.preview = None;
                    self.snapshot.started = Some(id.clone());
                    return Ok(Some(id));
                }
                Action::Reveal(id) => {
                    runtime.command(|db, _, _| {
                        let destination = db.export_destination(id)?;
                        let path = destination.path_for_reveal()?;
                        crate::host::reveal(path).map_err(|source| StorageError::Io {
                            operation: "reveal validated export destination",
                            source,
                        })
                    })?;
                    self.snapshot.revealed = true;
                }
                Action::Close => {
                    self.discard(runtime)?;
                    self.context = None;
                    self.destination = None;
                    self.snapshot.has_destination = false;
                }
            }
            Ok(None)
        })();
        match result {
            Ok(id) => id,
            Err(error) => {
                if start_attempt {
                    self.ticket = None;
                    if let Some(p) = &mut self.preview {
                        p.preparation = None;
                    }
                    self.publish();
                }
                self.snapshot.recovery_required |= matches!(
                    error,
                    StorageError::RuntimeFenced
                        | StorageError::CustodyRecoveryRequired
                        | StorageError::ProfileRestartRequired
                );
                self.snapshot.error = Some(error.code().into());
                None
            }
        }
    }
}

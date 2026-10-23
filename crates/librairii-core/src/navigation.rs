//! Bounded semantic navigation independent of presentation and toolkit state.
use crate::{
    CollectionId, LibraryId, ShelfId,
    collection_codec::url::{self, Location, Scope},
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, sync::Arc};

pub const MAX_HISTORY: usize = 100;
pub const MAX_STATE_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Presentation {
    #[default]
    List,
    SmallCovers,
    LargeCovers,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Frame {
    pub location: Location,
    pub selection: Vec<i64>,
}
#[derive(Clone, Debug)]
pub struct QueryStamp {
    library: LibraryId,
    generation: u64,
    revision: u64,
    identity: Arc<()>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("native navigation state is invalid or exceeds its bounds")]
pub struct InvalidNavigation;
type Result<T> = std::result::Result<T, InvalidNavigation>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Navigation {
    library: LibraryId,
    generation: u64,
    revision: u64,
    identity: Arc<()>,
    frames: Vec<Frame>,
    position: usize,
    presentation: Presentation,
}
impl PartialEq for QueryStamp {
    fn eq(&self, other: &Self) -> bool {
        self.library == other.library
            && self.generation == other.generation
            && self.revision == other.revision
            && Arc::ptr_eq(&self.identity, &other.identity)
    }
}
impl Eq for QueryStamp {}

impl Navigation {
    pub fn new(library: LibraryId, generation: u64, location: Location) -> Result<Self> {
        if generation == 0 {
            return Err(InvalidNavigation);
        }
        let nav = Self {
            library,
            generation,
            revision: 1,
            identity: Arc::new(()),
            frames: vec![Frame {
                location,
                selection: Vec::new(),
            }],
            position: 0,
            presentation: Presentation::List,
        };
        nav.encode()?;
        Ok(nav)
    }
    pub fn library(&self) -> LibraryId {
        self.library
    }
    pub fn current(&self) -> &Frame {
        &self.frames[self.position]
    }
    pub fn presentation(&self) -> Presentation {
        self.presentation
    }
    pub fn set_presentation(&mut self, mode: Presentation) {
        self.presentation = mode;
    }
    pub fn stamp(&self) -> QueryStamp {
        QueryStamp {
            library: self.library,
            generation: self.generation,
            revision: self.revision,
            identity: self.identity.clone(),
        }
    }
    pub fn accepts(&self, stamp: &QueryStamp) -> bool {
        self.stamp() == *stamp
    }
    pub fn push(&mut self, frame: Frame) -> Result<()> {
        let mut next = self.clone();
        next.frames.truncate(next.position + 1);
        next.frames.push(frame);
        if next.frames.len() > MAX_HISTORY {
            next.frames.remove(0);
        }
        next.position = next.frames.len() - 1;
        next.revision = next.revision.checked_add(1).ok_or(InvalidNavigation)?;
        next.identity = Arc::new(());
        next.encode()?;
        *self = next;
        Ok(())
    }
    pub fn replace(&mut self, frame: Frame) -> Result<()> {
        let mut next = self.clone();
        next.frames[next.position] = frame;
        next.revision = next.revision.checked_add(1).ok_or(InvalidNavigation)?;
        next.identity = Arc::new(());
        next.encode()?;
        *self = next;
        Ok(())
    }
    pub fn select(&mut self, selection: Vec<i64>) -> Result<()> {
        let mut next = self.clone();
        next.frames[next.position].selection = selection;
        next.encode()?;
        *self = next;
        Ok(())
    }
    pub fn back(&mut self) -> Result<bool> {
        if self.position == 0 {
            return Ok(false);
        }
        let revision = self.revision.checked_add(1).ok_or(InvalidNavigation)?;
        self.position -= 1;
        self.revision = revision;
        self.identity = Arc::new(());
        Ok(true)
    }
    pub fn forward(&mut self) -> Result<bool> {
        if self.position + 1 == self.frames.len() {
            return Ok(false);
        }
        let revision = self.revision.checked_add(1).ok_or(InvalidNavigation)?;
        self.position += 1;
        self.revision = revision;
        self.identity = Arc::new(());
        Ok(true)
    }
    pub fn encode(&self) -> Result<Vec<u8>> {
        if self.frames.is_empty()
            || self.frames.len() > MAX_HISTORY
            || self.position >= self.frames.len()
        {
            return Err(InvalidNavigation);
        }
        let frames = self
            .frames
            .iter()
            .map(|frame| {
                validate_selection(&frame.selection)?;
                let (kind, id, manual_order) = match frame.location.scope {
                    Scope::Library => ("library", None, None),
                    Scope::Custom => ("custom", None, None),
                    Scope::Collection(id) => ("collection", Some(id.get()), None),
                    Scope::Shelf { id, manual_order } => {
                        ("shelf", Some(id.get()), Some(manual_order))
                    }
                };
                Ok(WireFrame {
                    kind: kind.into(),
                    id,
                    manual_order,
                    request: serde_json::from_slice(
                        &url::request_bytes(&frame.location.request)
                            .map_err(|_| InvalidNavigation)?,
                    )
                    .map_err(|_| InvalidNavigation)?,
                    selection: frame.selection.clone(),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let bytes = serde_json::to_vec(&WireState {
            version: 1,
            library_id: self.library.to_string(),
            frames,
            position: self.position,
            presentation: self.presentation,
        })
        .map_err(|_| InvalidNavigation)?;
        if bytes.len() > MAX_STATE_BYTES {
            return Err(InvalidNavigation);
        }
        Ok(bytes)
    }
    pub fn decode(bytes: &[u8], expected: LibraryId, generation: u64) -> Result<Self> {
        if bytes.len() > MAX_STATE_BYTES || generation == 0 {
            return Err(InvalidNavigation);
        }
        let wire: WireState = serde_json::from_slice(bytes).map_err(|_| InvalidNavigation)?;
        if wire.version != 1
            || wire.library_id != expected.to_string()
            || wire.frames.is_empty()
            || wire.frames.len() > MAX_HISTORY
            || wire.position >= wire.frames.len()
        {
            return Err(InvalidNavigation);
        }
        let frames = wire
            .frames
            .into_iter()
            .map(|f| {
                validate_selection(&f.selection)?;
                let scope = match (f.kind.as_str(), f.id, f.manual_order) {
                    ("library", None, None) => Scope::Library,
                    ("custom", None, None) => Scope::Custom,
                    ("collection", Some(id), None) => {
                        Scope::Collection(CollectionId::new(id).map_err(|_| InvalidNavigation)?)
                    }
                    ("shelf", Some(id), Some(manual_order)) => Scope::Shelf {
                        id: ShelfId::new(id).map_err(|_| InvalidNavigation)?,
                        manual_order,
                    },
                    _ => return Err(InvalidNavigation),
                };
                let request = url::restore_request(
                    &serde_json::to_vec(&f.request).map_err(|_| InvalidNavigation)?,
                )
                .map_err(|_| InvalidNavigation)?;
                Ok(Frame {
                    location: Location { scope, request },
                    selection: f.selection,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            library: expected,
            generation,
            revision: 1,
            identity: Arc::new(()),
            frames,
            position: wire.position,
            presentation: wire.presentation,
        })
    }
}
fn validate_selection(ids: &[i64]) -> Result<()> {
    let mut seen = BTreeSet::new();
    if ids.iter().any(|id| *id <= 0 || !seen.insert(*id)) {
        return Err(InvalidNavigation);
    }
    Ok(())
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct WireState {
    version: i64,
    library_id: String,
    frames: Vec<WireFrame>,
    position: usize,
    presentation: Presentation,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct WireFrame {
    kind: String,
    id: Option<i64>,
    manual_order: Option<bool>,
    request: serde_json::Value,
    selection: Vec<i64>,
}

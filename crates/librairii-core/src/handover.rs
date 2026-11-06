//! Explicit preference transfer from the compatibility Go release. Browser
//! databases, paths, tokens and arbitrary history dictionaries are not inputs.
use crate::{
    CollectionId, JobId, LibraryId, ShelfId,
    collection_codec::url::{self, Location, Scope},
    navigation::{Frame, MAX_HISTORY, MAX_STATE_BYTES, Navigation, Presentation},
};
use serde::Deserialize;
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("preference transfer is invalid, belongs to another library or exceeds its limits")]
pub struct InvalidHandover;
type Result<T> = std::result::Result<T, InvalidHandover>;

#[derive(Clone, Debug)]
pub struct Preferences {
    pub navigation: Navigation,
    pub dismissed_operations: BTreeSet<String>,
    pub sync_panel_hidden: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Document {
    format: String,
    version: u64,
    library_id: String,
    legacy_broken_tag_id: i64,
    state: State,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct State {
    presentation: Presentation,
    dismissed_operations: Vec<String>,
    sync_panel_hidden: bool,
    frames: Vec<LegacyFrame>,
    position: usize,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct LegacyFrame {
    hash: String,
    kind: String,
    id: Option<i64>,
    manual_order: Option<bool>,
}
const MAX_SAFE_ID: i64 = 9_007_199_254_740_991;

pub fn decode(bytes: &[u8], library: LibraryId, generation: u64) -> Result<Preferences> {
    if bytes.len() > MAX_STATE_BYTES {
        return Err(InvalidHandover);
    }
    let document: Document = serde_json::from_slice(bytes).map_err(|_| InvalidHandover)?;
    let state = document.state;
    if document.format != "librairii.preference-handover"
        || document.version != 1
        || document.library_id != library.to_string()
        || !(0..=MAX_SAFE_ID).contains(&document.legacy_broken_tag_id)
        || state.frames.is_empty()
        || state.frames.len() > MAX_HISTORY
        || state.position >= state.frames.len()
        || state.dismissed_operations.len() > 4096
    {
        return Err(InvalidHandover);
    }
    let mut dismissed = BTreeSet::new();
    for id in state.dismissed_operations {
        let parsed: JobId = id.parse().map_err(|_| InvalidHandover)?;
        if parsed.to_string() != id || !dismissed.insert(id) {
            return Err(InvalidHandover);
        }
    }
    let mut navigation = None::<Navigation>;
    let count = state.frames.len();
    for frame in state.frames {
        let id = frame.id.filter(|id| *id > 0 && *id <= MAX_SAFE_ID);
        if id != frame.id {
            return Err(InvalidHandover);
        }
        let scope = match (frame.kind.as_str(), id, frame.manual_order) {
            ("library", None, None) => Scope::Library,
            ("custom", None, None) => Scope::Custom,
            ("collection", Some(id), None) => {
                Scope::Collection(CollectionId::new(id).map_err(|_| InvalidHandover)?)
            }
            ("shelf", Some(id), Some(manual_order)) => Scope::Shelf {
                id: ShelfId::new(id).map_err(|_| InvalidHandover)?,
                manual_order,
            },
            _ => return Err(InvalidHandover),
        };
        let request =
            url::decode(&frame.hash, document.legacy_broken_tag_id).map_err(|_| InvalidHandover)?;
        let location = Location { scope, request };
        match &mut navigation {
            None => {
                navigation = Some(
                    Navigation::new(library, generation, location).map_err(|_| InvalidHandover)?,
                );
            }
            Some(navigation) => navigation
                .push(Frame {
                    location,
                    selection: vec![],
                })
                .map_err(|_| InvalidHandover)?,
        }
    }
    let mut navigation = navigation.ok_or(InvalidHandover)?;
    for _ in state.position + 1..count {
        navigation.back().map_err(|_| InvalidHandover)?;
    }
    navigation.set_presentation(state.presentation);
    navigation.encode().map_err(|_| InvalidHandover)?;
    Ok(Preferences {
        navigation,
        dismissed_operations: dismissed,
        sync_panel_hidden: state.sync_panel_hidden,
    })
}

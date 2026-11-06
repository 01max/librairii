//! Dismissing a mismatch prompt is a profile preference, never a membership
//! decision or review authorization. A changed semantic snapshot prompts again.
use super::ShelfError;
use crate::LibraryId;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
const MAX_ENTRIES: usize = 1024;
const MAX_BYTES: usize = 1024 * 1024;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WarningKey([u8; 32]);
impl WarningKey {
    /// A digest of the semantic review snapshot, not its token or Broken flags.
    pub fn from_snapshot(digest: [u8; 32]) -> Self {
        Self(digest)
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WarningAcknowledgements {
    library: LibraryId,
    entries: BTreeMap<i64, WarningKey>,
    removals: BTreeMap<i64, BTreeSet<String>>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    version: u8,
    library: String,
    entries: Vec<Entry>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    shelf_id: i64,
    snapshot: [u8; 32],
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    removals: Vec<String>,
}
impl WarningAcknowledgements {
    pub fn new(library: LibraryId) -> Self {
        Self {
            library,
            entries: BTreeMap::new(),
            removals: BTreeMap::new(),
        }
    }
    pub fn library(&self) -> LibraryId {
        self.library
    }
    pub fn needs_prompt(&self, shelf_id: crate::ShelfId, key: WarningKey) -> bool {
        self.entries.get(&shelf_id.get()) != Some(&key)
    }
    /// Prompt only for newly affected UUIDs. Names, order, additions and flags
    /// do not turn already dismissed removals into new decisions.
    pub fn needs_prompt_for_members(
        &self,
        shelf_id: crate::ShelfId,
        key: WarningKey,
        removals: &[String],
    ) -> bool {
        self.removals.get(&shelf_id.get()).map_or_else(
            || self.needs_prompt(shelf_id, key),
            |known| removals.iter().any(|uuid| !known.contains(uuid)),
        )
    }
    pub fn acknowledge_members(
        &mut self,
        shelf_id: crate::ShelfId,
        key: WarningKey,
        removals: &[String],
    ) -> Result<(), ShelfError> {
        for uuid in removals {
            super::StoryReference::from_stored(uuid)?;
        }
        self.acknowledge(shelf_id, key)?;
        self.removals
            .insert(shelf_id.get(), removals.iter().cloned().collect());
        Ok(())
    }
    pub fn acknowledge(
        &mut self,
        shelf_id: crate::ShelfId,
        key: WarningKey,
    ) -> Result<(), ShelfError> {
        if !self.entries.contains_key(&shelf_id.get()) && self.entries.len() >= MAX_ENTRIES {
            return Err(ShelfError::InvalidInput);
        }
        self.entries.insert(shelf_id.get(), key);
        self.removals.remove(&shelf_id.get());
        Ok(())
    }
    pub fn forget(&mut self, shelf_id: crate::ShelfId) -> bool {
        self.removals.remove(&shelf_id.get());
        self.entries.remove(&shelf_id.get()).is_some()
    }
    pub fn encode(&self) -> Result<Vec<u8>, ShelfError> {
        let bytes = serde_json::to_vec(&Wire {
            version: 1,
            library: self.library.to_string(),
            entries: self
                .entries
                .iter()
                .map(|(id, k)| Entry {
                    shelf_id: *id,
                    snapshot: k.0,
                    removals: self
                        .removals
                        .get(id)
                        .map(|uuids| uuids.iter().cloned().collect())
                        .unwrap_or_default(),
                })
                .collect(),
        })
        .map_err(|_| ShelfError::InvalidInput)?;
        if bytes.len() > MAX_BYTES {
            return Err(ShelfError::InvalidInput);
        }
        Ok(bytes)
    }
    pub fn decode(bytes: &[u8], library: LibraryId) -> Result<Self, ShelfError> {
        if bytes.len() > MAX_BYTES {
            return Err(ShelfError::InvalidInput);
        }
        let wire: Wire = serde_json::from_slice(bytes).map_err(|_| ShelfError::InvalidInput)?;
        if wire.version != 1
            || wire.library != library.to_string()
            || wire.entries.len() > MAX_ENTRIES
        {
            return Err(ShelfError::InvalidInput);
        }
        let mut result = Self::new(library);
        for e in wire.entries {
            for uuid in &e.removals {
                super::StoryReference::from_stored(uuid)?;
            }
            if !e.removals.is_empty() {
                result
                    .removals
                    .insert(e.shelf_id, e.removals.into_iter().collect());
            }
            if e.shelf_id <= 0
                || result
                    .entries
                    .insert(e.shelf_id, WarningKey(e.snapshot))
                    .is_some()
            {
                return Err(ShelfError::InvalidInput);
            }
        }
        Ok(result)
    }
}

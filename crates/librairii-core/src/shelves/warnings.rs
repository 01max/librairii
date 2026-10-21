//! Dismissing a mismatch prompt is a profile preference, never a membership
//! decision or review authorization. A changed semantic snapshot prompts again.
use super::ShelfError;
use crate::LibraryId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
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
}
impl WarningAcknowledgements {
    pub fn new(library: LibraryId) -> Self {
        Self {
            library,
            entries: BTreeMap::new(),
        }
    }
    pub fn library(&self) -> LibraryId {
        self.library
    }
    pub fn needs_prompt(&self, shelf_id: crate::ShelfId, key: WarningKey) -> bool {
        self.entries.get(&shelf_id.get()) != Some(&key)
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
        Ok(())
    }
    pub fn forget(&mut self, shelf_id: crate::ShelfId) {
        self.entries.remove(&shelf_id.get());
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

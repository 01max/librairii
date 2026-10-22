//! Immutable creation inputs and legacy-compatible presentation envelopes.
use super::operations::*;
use librairii_core::{
    OperationKind,
    operations::{
        ExportSource, ExportSourceType, encode_kind, encode_state, text_present, valid_output_name,
    },
};
use serde_json::{Value, json};

/// Inputs retain exact strings. Path-shaped fields are observations, never
/// managed-file, destination, activation or cleanup authority.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct NewItem {
    pub story_id: i64,
    pub story_uuid: String,
    pub story_title: String,
    pub source_name: String,
    pub output_name: String,
    pub archive_relative_path: String,
    pub archive_sha256: String,
    pub export_skip_code: String,
    pub total_bytes: i64,
    pub planned: Option<PlannedItem>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlannedItem {
    pub state: librairii_core::operations::ItemState,
    pub outcome: librairii_core::operations::RawOutcome,
}
impl NewItem {
    // Preserve the immutable scope of preflight terminal rows verbatim. Only
    // ready rows pass copy-field validation and can enter the effect adapter.
    pub fn stored_output_name(&self) -> Option<&str> {
        (!self.output_name.is_empty()).then_some(self.output_name.as_str())
    }
    pub fn stored_archive_path(&self) -> Option<&str> {
        (!self.archive_relative_path.is_empty()).then_some(self.archive_relative_path.as_str())
    }
    pub fn stored_archive_hash(&self) -> Option<&str> {
        (!self.archive_sha256.is_empty()).then_some(self.archive_sha256.as_str())
    }
}
fn valid_relative(path: &str) -> bool {
    !path.is_empty()
        && path != "."
        && !path.starts_with('/')
        && !path.starts_with("../")
        && !path
            .split('/')
            .any(|p| p.is_empty() || p == "." || (p == ".." && path != ".."))
}
fn valid_hash(hash: &str) -> bool {
    hash.len() == 64
        && hash
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewOperation {
    pub id: String,
    pub kind: OperationKind,
    pub source: Option<ExportSource>,
    pub destination: Option<OperationPath>,
    pub items: Vec<NewItem>,
}
pub const MAX_COMMAND_BYTES: usize = 16 * 1024 * 1024;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputError {
    Invalid,
    ResourceLimit,
}
impl std::fmt::Display for InputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Invalid => "operations.invalid_request",
            Self::ResourceLimit => "operations.resource_limit",
        })
    }
}
impl std::error::Error for InputError {}
impl NewOperation {
    pub fn retained_bytes(&self) -> Result<usize, InputError> {
        let mut total = std::mem::size_of::<Self>();
        let mut add = |bytes: usize| -> Result<(), InputError> {
            total = total.checked_add(bytes).ok_or(InputError::ResourceLimit)?;
            if total > MAX_COMMAND_BYTES {
                return Err(InputError::ResourceLimit);
            }
            Ok(())
        };
        add(self.id.capacity())?;
        add(self.destination.as_ref().map_or(0, OperationPath::capacity))?;
        add(self
            .items
            .capacity()
            .checked_mul(std::mem::size_of::<NewItem>())
            .ok_or(InputError::ResourceLimit)?)?;
        for item in &self.items {
            if let Some(p) = &item.planned {
                add(p.outcome.code.as_ref().map_or(0, String::capacity))?;
                add(p.outcome.message.as_ref().map_or(0, String::capacity))?;
            }
            for value in [
                &item.story_uuid,
                &item.story_title,
                &item.source_name,
                &item.output_name,
                &item.archive_relative_path,
                &item.archive_sha256,
                &item.export_skip_code,
            ] {
                add(value.capacity())?;
            }
        }
        if let Some(source) = &self.source {
            for ids in [&source.collection_ids, &source.shelf_ids] {
                add(ids
                    .capacity()
                    .checked_mul(std::mem::size_of::<i64>())
                    .ok_or(InputError::ResourceLimit)?)?;
            }
            for names in [&source.collection_names, &source.shelf_names] {
                add(names
                    .capacity()
                    .checked_mul(std::mem::size_of::<String>())
                    .ok_or(InputError::ResourceLimit)?)?;
                for name in names {
                    add(name.capacity())?;
                }
            }
        }
        Ok(total)
    }
    pub fn validate(&self) -> Result<i64, InputError> {
        self.retained_bytes()?;
        // Match SQLite character length for new IDs. Existing
        // 36-character non-UUID identities remain valid and are not normalized.
        if self.id.contains('\0') || self.id.chars().count() != 36 || self.items.is_empty() {
            return Err(InputError::Invalid);
        }
        let mut total = 0i64;
        let mut ids = std::collections::BTreeSet::new();
        let mut uuids = std::collections::BTreeSet::new();
        for item in &self.items {
            if (!text_present(&item.source_name)
                && !(item.planned.is_some() && !item.source_name.is_empty()))
                || item.total_bytes < 0
            {
                return Err(InputError::Invalid);
            }
            total = total
                .checked_add(item.total_bytes)
                .ok_or(InputError::Invalid)?;
            if self.kind != OperationKind::Export {
                if item.planned.is_some() {
                    return Err(InputError::Invalid);
                }
                continue;
            }
            if item
                .story_uuid
                .parse::<librairii_core::StoryUuid>()
                .is_err()
                || !text_present(&item.story_title)
                || !uuids.insert(&item.story_uuid)
            {
                return Err(InputError::Invalid);
            }
            if let Some(plan) = &item.planned {
                if !matches!(
                    plan.state,
                    librairii_core::operations::ItemState::Skipped
                        | librairii_core::operations::ItemState::Conflicted
                ) || item.total_bytes != 0
                    || !plan.outcome.code.as_deref().is_some_and(|s| !s.is_empty())
                    || !plan
                        .outcome
                        .message
                        .as_deref()
                        .is_some_and(|s| !s.is_empty())
                    || (!item.export_skip_code.is_empty()
                        && (!matches!(
                            item.export_skip_code.as_str(),
                            "story_marked_broken" | "story_unavailable"
                        ) || plan.outcome.code.as_deref()
                            != Some(item.export_skip_code.as_str())))
                {
                    return Err(InputError::Invalid);
                }
                continue;
            }
            if !item.export_skip_code.is_empty() {
                if !matches!(
                    item.export_skip_code.as_str(),
                    "story_marked_broken" | "story_unavailable"
                ) || item.total_bytes != 0
                    || item.story_id < 0
                {
                    return Err(InputError::Invalid);
                }
                continue;
            }
            if item.story_id <= 0
                || !ids.insert(item.story_id)
                || !valid_output_name(&item.output_name)
                || !valid_relative(&item.archive_relative_path)
                || !valid_hash(&item.archive_sha256)
            {
                return Err(InputError::Invalid);
            }
        }
        if self.kind == OperationKind::Export {
            self.source
                .as_ref()
                .ok_or(InputError::Invalid)?
                .validate()
                .map_err(|_| InputError::Invalid)?;
            if !self.destination.as_deref().is_some_and(|p| {
                !p.as_os_str().as_encoded_bytes().contains(&0)
                    && p.to_str().map_or(!p.as_os_str().is_empty(), text_present)
            }) {
                return Err(InputError::Invalid);
            }
            Ok(total)
        } else if self.source.is_some() || self.destination.is_some() {
            Err(InputError::Invalid)
        } else {
            Ok(0)
        }
    }
}

fn insert_text(object: &mut serde_json::Map<String, Value>, key: &str, value: Option<&str>) {
    if let Some(value) = value.filter(|s| !s.is_empty()) {
        object.insert(key.into(), json!(value));
    }
}
impl OperationSnapshot {
    /// Legacy Wails JSON envelope with unchanged raw identities/outcome codes.
    /// Absolute destination and managed-path/hash internals are omitted.
    pub fn presentation(&self) -> Result<Value, InputError> {
        let h = &self.operation;
        let mut value = json!({"id":h.identity.id,"kind":encode_kind(h.kind),"status":encode_state(h.state),"completedItems":h.counts.completed,"totalItems":h.counts.total,"totalBytes":h.total_bytes,"cancelRequested":h.cancel_requested,"createdAt":h.identity.created_at,"items":[]});
        let map = value.as_object_mut().ok_or(InputError::Invalid)?;
        for (key, text) in [
            ("startedAt", h.identity.started_at.as_deref()),
            ("finishedAt", h.identity.finished_at.as_deref()),
            ("errorCode", h.outcome.code.as_deref()),
            ("errorMessage", h.outcome.message.as_deref()),
        ] {
            insert_text(map, key, text);
        }
        let decode_ids = |raw: Option<&str>| -> Result<Vec<i64>, InputError> {
            match raw.filter(|s| !s.is_empty()) {
                None => Ok(Vec::new()),
                Some(s) => serde_json::from_str::<Option<Vec<i64>>>(s)
                    .map(Option::unwrap_or_default)
                    .map_err(|_| InputError::Invalid),
            }
        };
        let decode_names = |raw: Option<&str>| -> Result<Vec<String>, InputError> {
            match raw.filter(|s| !s.is_empty()) {
                None => Ok(Vec::new()),
                Some(s) => serde_json::from_str::<Option<Vec<String>>>(s)
                    .map(Option::unwrap_or_default)
                    .map_err(|_| InputError::Invalid),
            }
        };
        let ids = decode_ids(h.export_scope.group_ids.as_deref())?;
        let names = decode_names(h.export_scope.group_names.as_deref())?;
        let ty = h
            .export_scope
            .discriminator
            .as_deref()
            .map(ExportSourceType::from_persisted)
            .transpose()
            .map_err(|_| InputError::Invalid)?;
        insert_text(
            map,
            "exportSourceType",
            h.export_scope.discriminator.as_deref(),
        );
        if !ids.is_empty() {
            map.insert("sourceShelfIds".into(), json!(ids));
        }
        if !names.is_empty() {
            map.insert("sourceShelfNames".into(), json!(names));
        }
        if ty.is_some_and(|ty| {
            matches!(
                ty,
                ExportSourceType::Collection
                    | ExportSourceType::Collections
                    | ExportSourceType::Shelf
                    | ExportSourceType::Shelves
            )
        }) {
            if !ids.is_empty() {
                map.insert("sourceCollectionIds".into(), json!(ids));
            }
            if !names.is_empty() {
                map.insert("sourceCollectionNames".into(), json!(names));
            }
        }
        if let Some(path) = h
            .destination
            .as_deref()
            .filter(|p| !p.as_os_str().is_empty())
        {
            let path = std::path::Path::new(path);
            let label = path
                .file_name()
                .unwrap_or(path.as_os_str())
                .to_string_lossy();
            insert_text(map, "destination", Some(&label));
        }
        let items=self.items.iter().map(|item|{
            let mut v=json!({"id":item.identity.id,"sourceName":item.source_name,"status":item.state.as_persisted(),"completedBytes":item.progress.completed,"totalBytes":item.progress.total});
            if let Some(m)=v.as_object_mut(){
                let story=item.identity.historical_story_id();if story!=0{m.insert("storyId".into(),json!(story));}
                for (key,text) in [("storyUuid",item.identity.story_uuid.as_deref()),("storyTitle",item.story_title.as_deref()),("outputName",item.output_name.as_deref()),("outcomeCode",item.outcome.code.as_deref()),("outcomeMessage",item.outcome.message.as_deref())]{insert_text(m,key,text);}
            }v
        }).collect::<Vec<_>>();
        map.insert(
            "items".into(),
            if items.is_empty() {
                Value::Null
            } else {
                Value::Array(items)
            },
        );
        Ok(value)
    }
}

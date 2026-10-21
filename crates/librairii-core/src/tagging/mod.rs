//! Tag identities and validation, independent of SQLite and presentation.
mod unicode15;
use crate::collection_codec::{tag_is_mark, tag_key_decomposition, tag_trim};

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum TagError {
    #[error("tagging.invalid_projection_config")]
    InvalidProjectionConfig,
    #[error("tagging.derived_facet_drift")]
    DerivedFacetDrift,
    #[error("tagging.delete_plan_stale")]
    DeletePlanStale,
    #[error("tagging.saved_collection_query_invalid")]
    SavedQueryInvalid,
    #[error("tagging.invalid_assignment")]
    InvalidAssignment,
    #[error("tagging.story_not_found")]
    StoryNotFound,
    #[error("tagging.assignment_kind")]
    AssignmentKind,
    #[error("tagging.derived_assignment")]
    DerivedAssignment,
    #[error("tagging.invalid_definition")]
    InvalidDefinition,
    #[error("tagging.duplicate_definition")]
    DuplicateDefinition,
    #[error("tagging.definition_not_found")]
    DefinitionNotFound,
    #[error("tagging.protected_definition")]
    ProtectedDefinition,
    #[error("tagging.invalid_order")]
    InvalidOrder,
    #[error("tagging.invalid_value")]
    InvalidValue,
    #[error("tagging.duplicate_value")]
    DuplicateValue,
    #[error("tagging.value_not_found")]
    ValueNotFound,
    #[error("tagging.values_not_allowed")]
    ValuesNotAllowed,
}
impl TagError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidProjectionConfig => "tagging.invalid_projection_config",
            Self::DerivedFacetDrift => "tagging.derived_facet_drift",
            Self::DeletePlanStale => "tagging.delete_plan_stale",
            Self::SavedQueryInvalid => "tagging.saved_collection_query_invalid",
            Self::InvalidAssignment => "tagging.invalid_assignment",
            Self::StoryNotFound => "tagging.story_not_found",
            Self::AssignmentKind => "tagging.assignment_kind",
            Self::DerivedAssignment => "tagging.derived_assignment",
            Self::InvalidDefinition => "tagging.invalid_definition",
            Self::DuplicateDefinition => "tagging.duplicate_definition",
            Self::DefinitionNotFound => "tagging.definition_not_found",
            Self::ProtectedDefinition => "tagging.protected_definition",
            Self::InvalidOrder => "tagging.invalid_order",
            Self::InvalidValue => "tagging.invalid_value",
            Self::DuplicateValue => "tagging.duplicate_value",
            Self::ValueNotFound => "tagging.value_not_found",
            Self::ValuesNotAllowed => "tagging.values_not_allowed",
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    Boolean,
    Choice,
}
impl Kind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Boolean => "boolean",
            Self::Choice => "choice",
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Definition {
    pub id: i64,
    pub key: String,
    pub normalized_key: String,
    pub label: String,
    pub color: String,
    pub kind: Kind,
    pub source: String,
    pub presentation: String,
    pub position: i64,
    pub protected: bool,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Value {
    pub id: i64,
    pub definition_id: i64,
    pub key: String,
    pub normalized_key: String,
    pub label: String,
    pub position: i64,
}
#[derive(Clone, Debug)]
pub struct CreateDefinition {
    pub key: String,
    pub label: String,
    pub color: String,
    pub kind: Kind,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogEntry {
    pub definition: Definition,
    pub values: Vec<Value>,
}

pub fn normalize_key(value: &str) -> Result<String, TagError> {
    let value = tag_key_decomposition(tag_trim(value)).map_err(|_| TagError::InvalidDefinition)?;
    let mut out = String::new();
    let mut separator = false;
    for c in value.chars() {
        if tag_is_mark(c) {
            continue;
        }
        let code = u32::from(c);
        let i = unicode15::LETTER_DIGIT.partition_point(|&(_, hi, _)| hi < code);
        if unicode15::LETTER_DIGIT
            .get(i)
            .is_some_and(|&(lo, _, stride)| code >= lo && (code - lo) % stride == 0)
        {
            if separator && !out.is_empty() {
                out.push('-');
            }
            separator = false;
            out.push(c);
        } else {
            separator = true;
        }
    }
    if out.is_empty() || out.chars().count() > 64 {
        return Err(TagError::InvalidDefinition);
    }
    Ok(out)
}
pub fn normalize_label(value: &str) -> Result<String, TagError> {
    let value = tag_trim(value);
    if value.is_empty() || value.chars().count() > 80 {
        return Err(TagError::InvalidDefinition);
    }
    Ok(value.into())
}
pub fn normalize_color(value: &str) -> Result<String, TagError> {
    let value = tag_trim(value).to_ascii_uppercase();
    let bytes = value.as_bytes();
    if bytes.len() != 7 || bytes[0] != b'#' || !bytes[1..].iter().all(u8::is_ascii_hexdigit) {
        return Err(TagError::InvalidDefinition);
    }
    let mut channels = [0.0; 3];
    for (i, c) in channels.iter_mut().enumerate() {
        let v = f64::from(
            u8::from_str_radix(&value[1 + i * 2..3 + i * 2], 16)
                .map_err(|_| TagError::InvalidDefinition)?,
        ) / 255.0;
        *c = if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        };
    }
    if 1.05 / (0.2126 * channels[0] + 0.7152 * channels[1] + 0.0722 * channels[2] + 0.05) < 3.0 {
        return Err(TagError::InvalidDefinition);
    }
    Ok(value)
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AssignmentResult {
    pub requested_stories: usize,
    pub changed_stories: usize,
    pub assignments_added: usize,
    pub assignments_removed: usize,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValueState {
    pub value_id: i64,
    pub assigned_stories: i64,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DefinitionState {
    pub definition_id: i64,
    pub assigned_stories: i64,
    pub values: Vec<ValueState>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssignmentWorkspace {
    pub catalog: Vec<CatalogEntry>,
    pub requested_stories: usize,
    pub states: Vec<DefinitionState>,
}
/// Raw selection cap applies before deduplication, matching the frozen API.
pub fn story_ids(ids: &[i64]) -> Result<Vec<i64>, TagError> {
    if ids.is_empty() || ids.len() > 1000 || ids.iter().any(|&id| id <= 0) {
        return Err(TagError::InvalidAssignment);
    }
    let mut ids = ids.to_vec();
    ids.sort_unstable();
    ids.dedup();
    Ok(ids)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgeBand {
    pub key: String,
    pub label: String,
    pub minimum: i64,
    pub maximum: i64,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgeConfig {
    pub bands: Vec<AgeBand>,
}
impl Default for AgeConfig {
    fn default() -> Self {
        Self {
            bands: vec![
                AgeBand {
                    key: "3-5".into(),
                    label: "3–5 years".into(),
                    minimum: 3,
                    maximum: 5,
                },
                AgeBand {
                    key: "6-8".into(),
                    label: "6–8 years".into(),
                    minimum: 6,
                    maximum: 8,
                },
            ],
        }
    }
}
impl AgeConfig {
    pub fn validate(&self) -> Result<(), TagError> {
        if self.bands.is_empty() || self.bands.len() > 32 {
            return Err(TagError::InvalidProjectionConfig);
        }
        for (i, b) in self.bands.iter().enumerate() {
            if b.key.is_empty()
                || b.key.chars().count() > 64
                || !b
                    .key
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
                || b.label.is_empty()
                || tag_trim(&b.label) != b.label
                || b.label.chars().count() > 80
                || b.minimum < 0
                || b.maximum < b.minimum
            {
                return Err(TagError::InvalidProjectionConfig);
            }
            if self.bands[..i]
                .iter()
                .any(|a| a.key == b.key || (b.minimum <= a.maximum && a.minimum <= b.maximum))
            {
                return Err(TagError::InvalidProjectionConfig);
            }
        }
        Ok(())
    }
    pub fn matching_band(&self, minimum: Option<i64>, maximum: Option<i64>) -> Option<usize> {
        let (minimum, maximum) = (minimum?, maximum?);
        if minimum > maximum {
            return None;
        }
        let mut matches = self
            .bands
            .iter()
            .enumerate()
            .filter(|(_, b)| minimum >= b.minimum && maximum <= b.maximum)
            .map(|(i, _)| i);
        let first = matches.next()?;
        if matches.next().is_some() {
            None
        } else {
            Some(first)
        }
    }
}
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProjectionResult {
    pub definition_id: i64,
    pub matched_stories: usize,
    pub assigned_stories: usize,
    pub unassigned: usize,
}

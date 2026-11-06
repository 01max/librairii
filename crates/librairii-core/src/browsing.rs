//! Owned query results. These values grant no mutation or export authority.
use serde::Serialize;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoryIdentity {
    pub id: i64,
    pub uuid: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplaySources {
    pub title: String,
    pub description: String,
    pub author: String,
    pub artwork: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OfficialInfo {
    pub locale: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub publisher: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub language: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_seconds: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimum_age: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maximum_age: Option<i64>,
    pub provenance: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub source_record_id: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub source_updated_at: String,
    pub fetched_at: String,
    pub activated_at: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorySummary {
    pub broken: bool,
    pub id: i64,
    pub uuid: String,
    pub title: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub author: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub artwork_id: String,
    pub sources: DisplaySources,
    pub detected_format: String,
    pub compatibility: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub compatibility_reason: String,
    pub byte_size: i64,
    pub imported_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub official: Option<OfficialInfo>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Page {
    pub stories: Vec<StorySummary>,
    pub page: i64,
    pub page_size: i64,
    pub total_items: i64,
    pub total_pages: i64,
    pub sort: String,
}

/// Snapshot data for later export planning, without custody or eligibility
/// authority. A copier must revalidate against its actual source and library.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct ExportStory {
    pub broken: bool,
    #[serde(rename = "ID")]
    pub id: i64,
    #[serde(rename = "UUID")]
    pub uuid: String,
    pub title: String,
    pub original_filename: String,
    /// Lossless SQL/native bytes. Invalid UTF-8 has an empty display string;
    /// a copier must consume these bytes or explicitly refuse representation.
    #[serde(skip)]
    pub original_filename_bytes: Vec<u8>,
    pub detected_format: String,
    #[serde(rename = "SHA256")]
    pub sha256: String,
    pub byte_size: i64,
    pub managed_relative_path: String,
    #[serde(skip)]
    pub managed_relative_path_bytes: Vec<u8>,
    pub verification: String,
}
impl ExportStory {
    /// Mandatory manual eligibility guard, independent of the export scope.
    /// A later copier must still revalidate the live record and owned file.
    pub const fn manual_export_skip(&self) -> Option<crate::broken::ExportSkip> {
        crate::broken::export_skip(self.id != 0, self.broken)
    }
}

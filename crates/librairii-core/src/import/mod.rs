//! Pure import policy over supplied data, not an archive inspector or custody API.
//!
//! Callers must separately establish decoder safety, actual entry/payload facts,
//! integrity and storage authority. None of these values proves a committed import.

mod dispatch;
mod entries;
mod limits;
mod pack;
mod report;

pub use dispatch::{
    ArchivePlan, ContainerKind, ContentFacts, IntegrityRequirement, PackStructure, dispatch,
};
pub use entries::{
    CompressionAccounting, EntryRecord, Inventory, InventoryBuilder, validate_entry_name,
};
pub use limits::{Budget, BudgetUsage, EntryInfo, Limits};
pub use pack::{
    CandidateData, ImportIdentity, PackRoot, UuidSource, validate_plain_entries,
    validate_uuid_root_entries,
};
pub use report::{
    CommitObservation, FailureCause, ProblemPhase, ReportedExecution, ReportedOutcome,
    ReportedProblem,
};

macro_rules! codes {
    ($(#[$meta:meta])* pub enum $name:ident { $($variant:ident => $code:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub enum $name { $($variant),+ }
        impl $name {
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];
            pub const fn as_str(self) -> &'static str {
                match self { $(Self::$variant => $code),+ }
            }
            pub fn from_code(code: &str) -> Option<Self> {
                match code { $($code => Some(Self::$variant),)+ _ => None }
            }
        }
        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.as_str())
            }
        }
    };
}

codes! {
    /// Frozen catalog format labels. A label is not a decoder acceptance receipt.
    pub enum ArchiveFormat {
        PlainPk => "plain_pk",
        V1Pk => "v1_pk",
        V2Pk => "v2_pk",
        GenericPk => "generic_pk",
        Zip => "zip",
        SevenZip => "seven_zip",
        StudioZip => "studio_zip",
    }
}

codes! {
    /// Frozen inspection rejection codes, independent of shared identity errors.
    pub enum ValidationCode {
        InvalidLimits => "invalid_limits",
        InvalidContainer => "invalid_container",
        UnsupportedFormat => "unsupported_format",
        AmbiguousStructure => "ambiguous_structure",
        UnsafePath => "unsafe_path",
        LinkEntry => "link_entry",
        EntryLimit => "entry_limit",
        ExpandedSizeLimit => "expanded_size_limit",
        CompressionRatioLimit => "compression_ratio_limit",
        NestedArchive => "nested_archive",
        MetadataLimit => "metadata_limit",
        ArtworkLimit => "artwork_limit",
        MissingEntry => "missing_entry",
        MissingAsset => "missing_asset",
        InvalidUuid => "invalid_uuid",
        MalformedMetadata => "malformed_metadata",
    }
}

codes! {
    /// Frozen reported import labels, not evidence that their named effects ran.
    pub enum OutcomeCode {
        Imported => "imported",
        DuplicateChecksum => "duplicate_checksum",
        UuidConflict => "uuid_conflict",
    }
}

codes! {
    /// Frozen importer failure codes; runtime cancellation mapping is external.
    pub enum ImportErrorCode {
        Stage => "stage_failed",
        Lookup => "lookup_failed",
        Inspection => "inspection_failed",
        Publish => "publish_failed",
        Artwork => "artwork_failed",
        Catalog => "catalog_failed",
        Compensation => "compensation_failed",
        Cleanup => "cleanup_failed",
        Cancelled => "cancelled",
    }
}

/// Structured pure rejection data. Raw entry bytes remain available even for
/// invalid UTF-8; diagnostic formatting does not publish their contents.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidationError {
    pub code: ValidationCode,
    pub entry: Option<Vec<u8>>,
    pub cause: Option<String>,
}
impl ValidationError {
    pub const fn new(code: ValidationCode) -> Self {
        Self {
            code,
            entry: None,
            cause: None,
        }
    }
    pub fn at(code: ValidationCode, entry: impl AsRef<[u8]>) -> Self {
        Self {
            code,
            entry: Some(entry.as_ref().to_vec()),
            cause: None,
        }
    }
    pub fn caused_by(mut self, cause: impl Into<String>) -> Self {
        self.cause = Some(cause.into());
        self
    }
}
impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} validation failed", self.code)
    }
}
impl std::error::Error for ValidationError {}

/// Match only an ASCII policy suffix under frozen Go Unicode15 simple-lower
/// behavior. Besides ASCII capitals, U+0130 -> i and U+212A -> k can contribute
/// to these suffixes. This is not Unicode folding or filename normalization.
/// Invalid UTF-8 elsewhere in the original filename remains untouched.
pub(super) fn suffix_matches(mut name: &[u8], suffix: &str) -> bool {
    for expected in suffix.bytes().rev() {
        if name
            .last()
            .is_some_and(|b| b.to_ascii_lowercase() == expected)
        {
            name = &name[..name.len() - 1];
        } else if expected == b'i' && name.ends_with(b"\xc4\xb0") {
            name = &name[..name.len() - 2];
        } else if expected == b'k' && name.ends_with(b"\xe2\x84\xaa") {
            name = &name[..name.len() - 3];
        } else {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests;

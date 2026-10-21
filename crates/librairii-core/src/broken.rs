//! Independent manual flags; tag assignment and archive verification are separate.
use std::collections::BTreeSet;
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum FlagError {
    #[error("library.invalid_flag_request")]
    InvalidRequest,
    #[error("library.flag_story_unavailable")]
    StoryUnavailable,
    #[error("library.flag_restore_stale")]
    RestoreStale,
}
impl FlagError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidRequest => "library.invalid_flag_request",
            Self::StoryUnavailable => "library.flag_story_unavailable",
            Self::RestoreStale => "library.flag_restore_stale",
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FlagChange {
    pub story_id: i64,
    pub previous: bool,
    pub current: bool,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FlagResult {
    pub requested_stories: usize,
    pub changed_stories: usize,
    pub changes: Vec<FlagChange>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FlagSummary {
    pub requested_stories: usize,
    pub flagged_stories: usize,
}
pub fn targets(ids: &[i64]) -> Result<Vec<i64>, FlagError> {
    if ids.is_empty() || ids.len() > 1000 || ids.iter().any(|&id| id <= 0) {
        return Err(FlagError::InvalidRequest);
    }
    let mut seen = BTreeSet::new();
    Ok(ids.iter().copied().filter(|id| seen.insert(*id)).collect())
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExportSkip {
    Unavailable,
    Broken,
}
impl ExportSkip {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Unavailable => "story_unavailable",
            Self::Broken => "story_marked_broken",
        }
    }
}
/// Shared by every scope, before any file eligibility or publication check.
/// None grants no export authority: custody/filename/queued revalidation follows.
pub const fn export_skip(available: bool, broken: bool) -> Option<ExportSkip> {
    if !available {
        Some(ExportSkip::Unavailable)
    } else if broken {
        Some(ExportSkip::Broken)
    } else {
        None
    }
}

use super::{ImportErrorCode, OutcomeCode, ValidationError};

/// Owned error observations with nested/joined structure and source order intact.
/// These are task-local data, not shared error ports or a persisted error codec.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FailureCause {
    Message(String),
    Inspection(ValidationError),
    Import {
        code: ImportErrorCode,
        cause: Option<Box<FailureCause>>,
    },
    Joined(Vec<FailureCause>),
}
impl FailureCause {
    /// Like errors.Join, omit absent errors but preserve nonempty cause order
    /// and an explicitly present empty message. No errors yields no cause.
    pub fn join(causes: impl IntoIterator<Item = Option<Self>>) -> Option<Self> {
        let present: Vec<_> = causes.into_iter().flatten().collect();
        if present.is_empty() {
            None
        } else {
            Some(Self::Joined(present))
        }
    }
    /// Frozen errors.As-style first matching inspection error, not a search
    /// for a preferred code that would hide an earlier validation failure.
    pub fn first_validation(&self) -> Option<&ValidationError> {
        match self {
            Self::Inspection(error) => Some(error),
            Self::Import {
                cause: Some(cause), ..
            } => cause.first_validation(),
            Self::Joined(causes) => causes.iter().find_map(Self::first_validation),
            Self::Message(_) | Self::Import { cause: None, .. } => None,
        }
    }
}

impl std::fmt::Display for FailureCause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Message(_) => f.write_str("reported error"),
            Self::Inspection(error) => error.fmt(f),
            Self::Import { code, .. } => code.fmt(f),
            Self::Joined(_) => f.write_str("reported joined errors"),
        }
    }
}
impl std::error::Error for FailureCause {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Inspection(error) => Some(error),
            Self::Import {
                cause: Some(cause), ..
            } => Some(cause.as_ref()),
            // std::error::Error has one source, not Go's joined-source list.
            // The owned Joined children and first_validation retain all branches.
            Self::Message(_) | Self::Joined(_) | Self::Import { cause: None, .. } => None,
        }
    }
}

/// Raw reported facts. Optional values distinguish unreported from reported0/
/// empty bytes. Consumers must validate the real storage-issued acknowledgement;
/// constructing this struct or selecting Imported creates NO commitment proof,
/// managed file ownership, lease, receipt or compensation/cleanup authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReportedOutcome {
    pub code: OutcomeCode,
    pub raw_uuid: Vec<u8>,
    pub story_id: Option<i64>,
    pub archive_id: Option<i64>,
    pub existing_story_id: Option<i64>,
    pub checksum: Option<Vec<u8>>,
}

/// Supplied observation only, not a storage commit acknowledgement or persisted
/// operation status. The real guarded commit/reconciliation port is unreleased.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommitObservation {
    NotReported,
    Committed,
    ProvenNotCommitted,
    Uncertain,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProblemPhase {
    Primary,
    StagingCleanup,
    Compensation,
    Reporting,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReportedProblem {
    pub phase: ProblemPhase,
    pub cause: FailureCause,
}

/// Orthogonal supplied fact/commit/error data, not Result-only failure. Recording
/// a late error cannot erase the captured fact or change commit truth. This type
/// owns no effect resources and cannot grant or transfer custody authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReportedExecution {
    outcome: Option<ReportedOutcome>,
    commit: CommitObservation,
    problems: Vec<ReportedProblem>,
}
impl ReportedExecution {
    pub fn from_supplied_observations(
        outcome: Option<ReportedOutcome>,
        commit: CommitObservation,
    ) -> Self {
        Self {
            outcome,
            commit,
            problems: Vec::new(),
        }
    }
    pub fn outcome(&self) -> Option<&ReportedOutcome> {
        self.outcome.as_ref()
    }
    pub const fn commit(&self) -> CommitObservation {
        self.commit
    }
    pub fn problems(&self) -> &[ReportedProblem] {
        &self.problems
    }
    pub fn record_problem(&mut self, problem: ReportedProblem) {
        self.problems.push(problem);
    }
}

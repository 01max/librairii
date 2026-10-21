//! An explicit refresh owns one repository borrow from start through report.
//! Drive this synchronous state machine on a blocking worker. Fetch between
//! `begin` and `finish`; construction/opening never performs network requests.
use super::*;
use librairii_core::{CatalogSnapshotId, JobId};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RepositoryFailure {
    Proven,
    Uncertain,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RefreshCode {
    FetchFailed,
    InvalidCatalog,
    StageFailed,
    PersistenceFailed,
    Cancelled,
    OutcomeUncertain,
}
impl RefreshCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FetchFailed => "catalog_fetch_failed",
            Self::InvalidCatalog => "catalog_invalid",
            Self::StageFailed => "catalog_stage_failed",
            Self::PersistenceFailed => "catalog_persist_failed",
            Self::Cancelled => "catalog_cancelled",
            Self::OutcomeUncertain => "catalog_outcome_uncertain",
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RefreshReport {
    pub sync: JobId,
    pub snapshot: CatalogSnapshotId,
    pub matched_stories: i64,
    pub activated_at: String,
}
pub trait CatalogRepository {
    fn begin(
        &self,
        id: JobId,
        locale: Locale,
        at: &str,
    ) -> std::result::Result<(), RepositoryFailure>;
    fn stage(
        &self,
        id: JobId,
        catalog: NormalizedCatalog,
        at: &str,
    ) -> std::result::Result<CatalogSnapshotId, RepositoryFailure>;
    /// Once called, ignore work cancellation through durable success/report.
    fn activate(
        &self,
        id: CatalogSnapshotId,
        at: &str,
    ) -> std::result::Result<RefreshReport, RepositoryFailure>;
    fn fail(
        &self,
        id: JobId,
        snapshot: Option<CatalogSnapshotId>,
        code: RefreshCode,
        at: &str,
    ) -> std::result::Result<(), RepositoryFailure>;
}
#[derive(Debug)]
pub struct RefreshSession<'a, R: CatalogRepository> {
    repository: &'a R,
    id: JobId,
    locale: Locale,
}
impl<'a, R: CatalogRepository> RefreshSession<'a, R> {
    pub fn begin(
        repository: &'a R,
        id: JobId,
        locale: &str,
        at: &str,
    ) -> std::result::Result<Self, RefreshCode> {
        let locale = Locale::parse(locale).map_err(|_| RefreshCode::InvalidCatalog)?;
        repository
            .begin(id, locale.clone(), at)
            .map_err(|e| failure(e, RefreshCode::PersistenceFailed))?;
        Ok(Self {
            repository,
            id,
            locale,
        })
    }
    pub fn finish(
        self,
        payload: Result<Vec<u8>>,
        at: &str,
        cancelled: impl Fn() -> bool,
    ) -> std::result::Result<RefreshReport, RefreshCode> {
        let raw = match payload {
            Ok(v) => v,
            Err(_) => {
                return self.failed(
                    None,
                    if cancelled() {
                        RefreshCode::Cancelled
                    } else {
                        RefreshCode::FetchFailed
                    },
                    at,
                );
            }
        };
        if cancelled() {
            return self.failed(None, RefreshCode::Cancelled, at);
        }
        let normalized = match normalize(raw, self.locale.as_str()) {
            Ok(v) => v,
            Err(_) => return self.failed(None, RefreshCode::InvalidCatalog, at),
        };
        if cancelled() {
            return self.failed(None, RefreshCode::Cancelled, at);
        }
        let snapshot = match self.repository.stage(self.id, normalized, at) {
            Ok(v) => v,
            Err(RepositoryFailure::Uncertain) => return Err(RefreshCode::OutcomeUncertain),
            Err(_) => return self.failed(None, RefreshCode::StageFailed, at),
        };
        if cancelled() {
            return self.failed(Some(snapshot), RefreshCode::Cancelled, at);
        }
        match self.repository.activate(snapshot, at) {
            Ok(report) => Ok(report),
            Err(RepositoryFailure::Uncertain) => Err(RefreshCode::OutcomeUncertain),
            Err(_) => self.failed(Some(snapshot), RefreshCode::PersistenceFailed, at),
        }
    }
    fn failed<T>(
        &self,
        snapshot: Option<CatalogSnapshotId>,
        code: RefreshCode,
        at: &str,
    ) -> std::result::Result<T, RefreshCode> {
        self.repository
            .fail(self.id, snapshot, code, at)
            .map_err(|e| failure(e, RefreshCode::PersistenceFailed))?;
        Err(code)
    }
}
fn failure(e: RepositoryFailure, otherwise: RefreshCode) -> RefreshCode {
    match e {
        RepositoryFailure::Proven => otherwise,
        RepositoryFailure::Uncertain => RefreshCode::OutcomeUncertain,
    }
}

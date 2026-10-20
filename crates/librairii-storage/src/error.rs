//! Storage errors carry an operation and cause without making a path authority.
use std::io;

pub type Result<T> = std::result::Result<T, StorageError>;

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("storage.explicit_override_required")]
    MissingOverride,
    #[error("storage.absolute_root_required")]
    RootNotAbsolute,
    #[error("storage.fresh_root_required")]
    RootNotFresh,
    #[error("storage.invalid_managed_path")]
    InvalidManagedPath,
    #[error("storage.path_escape")]
    PathEscape,
    #[error("settings.invalid")]
    InvalidIdentity,
    #[error("settings.library_missing")]
    IdentityMismatch,
    #[error("storage.foreign_schema")]
    ForeignSchema,
    #[error("storage.migration_conflict")]
    MigrationConflict,
    #[error("storage.sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("storage.runtime_in_use")]
    RuntimeInUse,
    #[error("storage.qualified_existing_open_required")]
    QualifiedExistingOpenRequired,
    #[error("storage.queue_full")]
    QueueFull,
    #[error("storage.intake_closed")]
    IntakeClosed,
    #[error("storage.worker_stopped_outcome_unavailable")]
    WorkerStopped,
    #[error("storage.runtime_fenced")]
    RuntimeFenced,
    #[error("storage.work_cancelled")]
    Cancelled,
    #[error("storage.rollback_uncertain: work={work}; rollback={rollback}")]
    RollbackUncertain {
        work: Box<StorageError>,
        rollback: rusqlite::Error,
    },
    #[error("{operation}: {source}")]
    Io {
        operation: &'static str,
        #[source]
        source: io::Error,
    },
}

impl StorageError {
    pub(crate) fn io(operation: &'static str, source: io::Error) -> Self {
        Self::Io { operation, source }
    }

    pub const fn code(&self) -> &'static str {
        match self {
            Self::MissingOverride => "storage.explicit_override_required",
            Self::RootNotAbsolute => "storage.absolute_root_required",
            Self::RootNotFresh => "storage.fresh_root_required",
            Self::InvalidManagedPath => "storage.invalid_managed_path",
            Self::PathEscape => "storage.path_escape",
            Self::InvalidIdentity => "settings.invalid",
            Self::IdentityMismatch => "settings.library_missing",
            Self::ForeignSchema => "storage.foreign_schema",
            Self::MigrationConflict => "storage.migration_conflict",
            Self::Sqlite(_) => "storage.sqlite",
            Self::RuntimeInUse => "storage.runtime_in_use",
            Self::QualifiedExistingOpenRequired => "storage.qualified_existing_open_required",
            Self::QueueFull => "storage.queue_full",
            Self::IntakeClosed => "storage.intake_closed",
            Self::WorkerStopped => "storage.worker_stopped_outcome_unavailable",
            Self::RuntimeFenced => "storage.runtime_fenced",
            Self::Cancelled => "storage.work_cancelled",
            Self::RollbackUncertain { .. } => "storage.rollback_uncertain",
            Self::Io { .. } => "storage.io_failed",
        }
    }
}

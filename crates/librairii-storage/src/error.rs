//! Storage errors carry an operation and cause without making a path authority.
use std::io;

pub type Result<T> = std::result::Result<T, StorageError>;

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("{0}")]
    Flag(#[from] librairii_core::broken::FlagError),
    #[error("{0}")]
    Tag(#[from] librairii_core::tagging::TagError),
    #[error("navigation.invalid_state")]
    InvalidNavigation,
    #[error("{0}")]
    QueryInput(#[from] librairii_core::query::QueryInputError),
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
    #[error("settings.invalid")]
    InvalidProfile,
    #[error("settings.restart_required")]
    ProfileRestartRequired,
    #[error("settings.restart_required: {cause}")]
    ProfilePublicationUncertain { cause: Box<StorageError> },
    #[error("settings.profile_in_use")]
    ProfileInUse,
    #[error("settings.profile_changed")]
    ProfileChanged,
    #[error("settings.active_library_binding")]
    ProfileBindingActive,
    #[error("settings.update_in_progress")]
    ProfileBusy,
    #[error("settings.recovery_binding_required")]
    RecoveryBindingRequired,
    #[error("settings.library_missing")]
    IdentityMismatch,
    #[error("storage.foreign_schema")]
    ForeignSchema,
    #[error("storage.migration_conflict")]
    MigrationConflict,
    #[error("collection.not_found")]
    CollectionNotFound,
    #[error("storage.frozen_compatibility_invariant")]
    CompatibilityInvariant,
    #[error("storage.sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("storage.runtime_in_use")]
    RuntimeInUse,
    #[error("storage.library_in_use")]
    LibraryInUse,
    #[error("storage.library_lease_changed")]
    LibraryLeaseChanged,
    #[error("storage.legacy_lifetime_exclusion_unproven")]
    LegacyLifetimeExclusionUnproven,
    #[error("storage.adoption_input_invalid")]
    AdoptionInputInvalid,
    #[error("storage.adoption_input_changed")]
    AdoptionInputChanged,
    #[error("storage.closed_snapshot_required")]
    ClosedSnapshotRequired,
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
    #[error("storage.integrity_failed")]
    IntegrityFailed,
    #[error("storage.snapshot_changed")]
    SnapshotChanged,
    #[error("storage.backup_busy")]
    BackupBusy,
    #[error("storage.hash_size_limit")]
    HashSizeLimit,
    #[error("storage.custody_busy")]
    CustodyBusy,
    #[error("storage.custody_recovery_required")]
    CustodyRecoveryRequired,
    #[error("storage.destination_collision")]
    DestinationCollision,
    #[error("storage.owned_temp_cleanup_failed: work={work}; cleanup={cleanup}")]
    OwnedTempCleanupFailed {
        work: Box<StorageError>,
        cleanup: Box<StorageError>,
    },
    #[error("storage.backup_uncertain: backup={backup:?}: {cause}")]
    BackupUncertain {
        backup: Box<crate::backup::BackupReceipt>,
        cause: Box<StorageError>,
    },
    #[error("storage.directory_durability_unavailable")]
    DirectoryDurabilityUnavailable,
    #[error("storage.no_schema_conflict")]
    NoSchemaConflict,
    #[error("storage.preservation_failed: directory={directory:?}: {cause}")]
    PreservationFailed {
        directory: std::path::PathBuf,
        cause: Box<StorageError>,
    },
    #[error(
        "storage.migration_failed: version={version}, restored={restored}, backup={backup:?}, candidate={candidate:?}: {cause}"
    )]
    MigrationFailed {
        version: u32,
        backup: std::path::PathBuf,
        candidate: std::path::PathBuf,
        restored: bool,
        cause: Box<StorageError>,
    },
    #[error("storage.initialization_failed: candidate={candidate:?}, cleared={cleared}: {cause}")]
    InitializationFailed {
        candidate: std::path::PathBuf,
        cleared: bool,
        cause: Box<StorageError>,
    },
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
            Self::Flag(error) => error.code(),
            Self::Tag(error) => error.code(),
            Self::InvalidNavigation => "navigation.invalid_state",
            Self::QueryInput(librairii_core::query::QueryInputError::InvalidQuery) => {
                "library.invalid_query"
            }
            Self::QueryInput(librairii_core::query::QueryInputError::FrozenData) => {
                "storage.frozen_compatibility_invariant"
            }
            Self::QueryInput(librairii_core::query::QueryInputError::ResourceOverflow) => {
                "library.query_resource_overflow"
            }
            Self::MissingOverride => "storage.explicit_override_required",
            Self::RootNotAbsolute => "storage.absolute_root_required",
            Self::RootNotFresh => "storage.fresh_root_required",
            Self::InvalidManagedPath => "storage.invalid_managed_path",
            Self::PathEscape => "storage.path_escape",
            Self::InvalidIdentity => "settings.invalid",
            Self::InvalidProfile => "settings.invalid",
            Self::ProfileRestartRequired => "settings.restart_required",
            Self::ProfilePublicationUncertain { .. } => "settings.restart_required",
            Self::ProfileInUse => "settings.profile_in_use",
            Self::ProfileChanged => "settings.profile_changed",
            Self::ProfileBindingActive => "settings.active_library_binding",
            Self::ProfileBusy => "settings.update_in_progress",
            Self::RecoveryBindingRequired => "settings.recovery_binding_required",
            Self::IdentityMismatch => "settings.library_missing",
            Self::ForeignSchema => "storage.foreign_schema",
            Self::MigrationConflict => "storage.migration_conflict",
            Self::CollectionNotFound => "collection.not_found",
            Self::CompatibilityInvariant => "storage.frozen_compatibility_invariant",
            Self::Sqlite(_) => "storage.sqlite",
            Self::RuntimeInUse => "storage.runtime_in_use",
            Self::LibraryInUse => "storage.library_in_use",
            Self::LibraryLeaseChanged => "storage.library_lease_changed",
            Self::LegacyLifetimeExclusionUnproven => "storage.legacy_lifetime_exclusion_unproven",
            Self::AdoptionInputInvalid => "storage.adoption_input_invalid",
            Self::AdoptionInputChanged => "storage.adoption_input_changed",
            Self::ClosedSnapshotRequired => "storage.closed_snapshot_required",
            Self::QueueFull => "storage.queue_full",
            Self::IntakeClosed => "storage.intake_closed",
            Self::WorkerStopped => "storage.worker_stopped_outcome_unavailable",
            Self::RuntimeFenced => "storage.runtime_fenced",
            Self::Cancelled => "storage.work_cancelled",
            Self::IntegrityFailed => "storage.integrity_failed",
            Self::SnapshotChanged => "storage.snapshot_changed",
            Self::BackupBusy => "storage.backup_busy",
            Self::HashSizeLimit => "storage.hash_size_limit",
            Self::CustodyBusy => "storage.custody_busy",
            Self::CustodyRecoveryRequired => "storage.custody_recovery_required",
            Self::DestinationCollision => "storage.destination_collision",
            Self::OwnedTempCleanupFailed { .. } => "storage.owned_temp_cleanup_failed",
            Self::BackupUncertain { .. } => "storage.backup_uncertain",
            Self::DirectoryDurabilityUnavailable => "storage.directory_durability_unavailable",
            Self::NoSchemaConflict => "storage.no_schema_conflict",
            Self::PreservationFailed { .. } => "storage.preservation_failed",
            Self::MigrationFailed { .. } => "storage.migration_failed",
            Self::InitializationFailed { .. } => "storage.initialization_failed",
            Self::RollbackUncertain { .. } => "storage.rollback_uncertain",
            Self::Io { .. } => "storage.io_failed",
        }
    }
}

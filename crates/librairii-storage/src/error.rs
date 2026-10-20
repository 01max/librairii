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
            Self::Io { .. } => "storage.io_failed",
        }
    }
}

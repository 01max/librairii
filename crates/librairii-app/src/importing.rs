//! Typed inspection port. Its result is metadata, never file or SQL authority.
use crate::native_jobs::Cancellation;
use librairii_core::import::{ArchiveFormat, ImportIdentity, MetadataText, ValidationError};
pub use librairii_core::{JobId, OperationKind, OperationState};
use std::io::{Read, Seek};
pub trait ReadSeek: Read + Seek {}
impl<T: Read + Seek + ?Sized> ReadSeek for T {}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Inspection {
    pub identity: ImportIdentity,
    pub format: ArchiveFormat,
    pub text: MetadataText,
    pub artwork: Option<Vec<u8>>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InspectionError {
    Validation(ValidationError),
    Cancelled,
}
impl std::fmt::Display for InspectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Validation(e) => e.fmt(f),
            Self::Cancelled => f.write_str("import.cancelled"),
        }
    }
}
impl std::error::Error for InspectionError {}
/// Runs on the existing bounded blocking worker over the caller's retained
/// stage reader. Implementations cannot publish or clean up that stage.
pub trait Inspector: Sync {
    fn inspect(
        &self,
        source: &mut dyn ReadSeek,
        original: &[u8],
        cancel: &Cancellation,
    ) -> Result<Inspection, InspectionError>;
}

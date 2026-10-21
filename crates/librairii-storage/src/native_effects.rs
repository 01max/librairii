//! Native control-record registrations, created only from actual retained file
//! owners. Parsing this wire data alone cannot open a library or delete a path.
use crate::{Result, StorageError, platform::unix::FileIdentity};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Temporary {
    pub operation: String,
    pub item: i64,
    pub scope_sha256: String,
    /// None is the original managed staging descriptor. Some is a validated
    /// export destination, never an arbitrary final-file cleanup path.
    pub destination: Option<String>,
    pub parent: FileIdentity,
    pub name: String,
    pub directory: Option<FileIdentity>,
    pub payload: Option<FileIdentity>,
}
impl Temporary {
    pub(crate) fn validate(&self) -> Result<()> {
        if self.payload.is_some() && self.directory.is_none() {
            return Err(StorageError::RecoveryBindingRequired);
        }
        let prefix = if self.destination.is_some() {
            ".librairii-export-"
        } else {
            ".librairii-stage-"
        };
        if self.operation.is_empty()
            || self.operation.len() > 256
            || self.scope_sha256.len() != 64
            || !self
                .scope_sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || !self.name.starts_with(prefix)
            || self.name.len() > 160
            || self.name.contains(['/', '\\', '\0'])
        {
            return Err(StorageError::RecoveryBindingRequired);
        }
        if let Some(path) = &self.destination
            && (!std::path::Path::new(path).is_absolute()
                || path.len() > 16 * 1024
                || path.contains('\0'))
        {
            return Err(StorageError::RecoveryBindingRequired);
        }
        Ok(())
    }
}

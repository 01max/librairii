//! Read-only reveal handover from an actual stored terminal export and retained
//! directory object. It grants no write/publication/deletion authority.
use super::rows;
use crate::{Result, StorageError, database::Database, platform::unix::Directory};
use librairii_app::ports::{creation::InputError, operations::OperationScope};
use librairii_core::{
    OperationKind,
    operations::{is_terminal, text_present},
};
use std::{
    fs,
    path::{Component, Path, PathBuf},
};
#[derive(Debug)]
pub struct ExportDestination<'database, 'library> {
    database: &'database Database<'library>,
    operation: OperationScope,
    path: PathBuf,
    directory: Directory,
}
impl ExportDestination<'_, '_> {
    /// Revalidate immediately before handing this one argument to the platform
    /// file manager. Native composition must never interpolate it into a shell.
    pub fn path_for_reveal(&self) -> Result<&Path> {
        let id = self.operation.identity.id.clone();
        let header = self
            .database
            .barrier_read(move |tx| rows::header(tx, &id))?
            .wait()?
            .ok_or(StorageError::SnapshotChanged)?;
        if header.immutable_scope() != self.operation || !is_terminal(header.state) {
            return Err(StorageError::SnapshotChanged);
        }
        self.directory.matches_metadata(
            &fs::symlink_metadata(&self.path)
                .map_err(|e| StorageError::io("revalidate export reveal directory", e))?,
        )?;
        Ok(&self.path)
    }
}
impl<'library> Database<'library> {
    pub fn export_destination(&self, id: String) -> Result<ExportDestination<'_, 'library>> {
        if id.capacity() > 256 {
            return Err(InputError::Invalid.into());
        }
        let header = self
            .barrier_read(move |tx| rows::header(tx, &id))?
            .wait()?
            .ok_or(InputError::Invalid)?;
        if header.kind != OperationKind::Export || !is_terminal(header.state) {
            return Err(InputError::Invalid.into());
        }
        let raw = header.destination.as_deref().ok_or(InputError::Invalid)?;
        let path = Path::new(raw);
        if raw.len() > 16 * 1024
            || !text_present(raw)
            || raw.contains('\0')
            || !path.is_absolute()
            || path
                .components()
                .any(|c| matches!(c, Component::CurDir | Component::ParentDir))
        {
            return Err(InputError::Invalid.into());
        }
        if fs::canonicalize(path)
            .map_err(|e| StorageError::io("resolve export reveal directory", e))?
            != path
        {
            return Err(InputError::Invalid.into());
        }
        let directory = Directory::open(path)?;
        Ok(ExportDestination {
            database: self,
            operation: header.immutable_scope(),
            path: path.to_owned(),
            directory,
        })
    }
}

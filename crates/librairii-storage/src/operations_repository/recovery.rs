//! Startup operates under the actual native/root/SQLite owners, before intake.
use super::rows;
use crate::{
    Result, StorageError,
    database::{Database, TransactionOutcome},
};
use librairii_core::{
    OperationKind,
    operations::{ItemState, encode_kind},
};
use sha2::{Digest, Sha256};
use std::{thread, time::Duration};
impl Database<'_> {
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    pub(crate) fn operation_temp_scope(
        &self,
        id: String,
        item: i64,
        running: bool,
    ) -> Result<(
        String,
        Option<librairii_app::ports::operations::OperationPath>,
        OperationKind,
    )> {
        self.barrier_read(move |tx| {
            let view = rows::view(tx, &id, Some(item))?.ok_or(StorageError::SnapshotChanged)?;
            let i = view.item.ok_or(StorageError::SnapshotChanged)?;
            let h = view.operation;
            if running
                && (librairii_core::operations::is_terminal(h.state)
                    || i.state != ItemState::Running)
            {
                return Err(StorageError::SnapshotChanged);
            }
            let mut hash = Sha256::new();
            hash.update(b"librairii.temporary-scope.v1");
            for value in [
                Some(h.identity.id.as_bytes()),
                Some(h.identity.created_at.as_bytes()),
                Some(encode_kind(h.kind).as_bytes()),
                h.export_scope.discriminator.as_deref().map(str::as_bytes),
                h.export_scope.group_ids.as_deref().map(str::as_bytes),
                h.export_scope.group_names.as_deref().map(str::as_bytes),
                h.destination
                    .as_deref()
                    .map(|p| p.as_os_str().as_encoded_bytes()),
                Some(i.operation_id.as_bytes()),
                i.identity.story_uuid.as_deref().map(str::as_bytes),
                i.story_title.as_deref().map(str::as_bytes),
                Some(i.source_name.as_bytes()),
                i.output_name
                    .as_ref()
                    .map(|p| p.as_os_str().as_encoded_bytes()),
                i.archive_relative_path
                    .as_ref()
                    .map(|p| p.as_os_str().as_encoded_bytes()),
                i.archive_sha256.as_deref().map(str::as_bytes),
                i.export_skip_code.as_deref().map(str::as_bytes),
            ] {
                match value {
                    None => hash.update([0]),
                    Some(v) => {
                        hash.update([1]);
                        hash.update((v.len() as u64).to_le_bytes());
                        hash.update(v);
                    }
                }
            }
            for number in [
                Some(h.counts.total),
                Some(h.total_bytes),
                Some(i.identity.id),
                i.identity.resolved_story_id,
                Some(i.progress.total),
            ] {
                match number {
                    None => hash.update([0]),
                    Some(n) => {
                        hash.update([1]);
                        hash.update(n.to_le_bytes());
                    }
                }
            }
            Ok((
                librairii_core::Checksum::from_bytes(hash.finalize().into()).to_string(),
                h.destination,
                h.kind,
            ))
        })?
        .wait()
    }
    pub(crate) fn recover_operation_startup(&self) -> Result<()> {
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        let cleanup = self.recover_native_temporaries();
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        let cleanup: Result<()> = Ok(());
        let cleanup_failed = cleanup.is_err();
        let at = self
            .barrier_read(|tx| {
                Ok(
                    tx.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%fZ','now')", [], |r| {
                        r.get::<_, String>(0)
                    })?,
                )
            })?
            .wait()?;
        // An idempotent desired-state repair, not a repeated item completion. A fresh
        // writer barrier after an unknown must positively prove no active rows remain.
        loop {
            let at = at.clone();
            let pending=self.repair_transaction(move|tx| {
    let inconsistent:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM file_operations o WHERE status IN ('queued','running') AND (total_items<>(SELECT count(*) FROM file_operation_items i WHERE i.operation_id=o.id) OR completed_items<>(SELECT count(*) FROM file_operation_items i WHERE i.operation_id=o.id AND i.status IN ('succeeded','skipped','conflicted','failed','cancelled'))))",[],|r|r.get(0))?;
    if inconsistent{return Err(StorageError::SnapshotChanged);}
    let code=if cleanup_failed{"interrupted_cleanup_failed"}else{"interrupted"};
    // Frozen kind-specific messages, preserved terminal outcomes and byte counts.
    let message="CASE kind WHEN 'import' THEN 'Import stopped when the application closed. Select the archive again to retry.' WHEN 'metadata_sync' THEN 'Metadata refresh stopped when the application closed. Start a new refresh to retry.' ELSE 'Export stopped when the application closed. Completed files were preserved; start a new export to retry.' END";
    tx.execute(&format!("UPDATE file_operation_items SET status='cancelled',outcome_code=?,outcome_message=(SELECT {message} FROM file_operations o WHERE o.id=operation_id) WHERE status IN ('pending','running') AND operation_id IN (SELECT id FROM file_operations WHERE status IN ('queued','running'))"),[code])?;
    tx.execute(&format!("UPDATE file_operations SET status='interrupted',completed_items=total_items,error_code=?,error_message={message},finished_at=? WHERE status IN ('queued','running')"),rusqlite::params![code,at])?;Ok(())
   });
            match pending {
    Ok(p)=>match p.wait()? {
     TransactionOutcome::Committed(())=>break,
     TransactionOutcome::ProvenNotCommitted(error)=>return Err(error),
     TransactionOutcome::CommitUncertain(_)=>{
      if self.barrier_read(|tx|Ok(tx.query_row("SELECT NOT EXISTS(SELECT 1 FROM file_operations WHERE status IN ('queued','running'))",[],|r|r.get::<_,bool>(0))?))?.wait()? {break;}
     }
    },
    Err(StorageError::QueueFull)=>{},Err(error)=>return Err(error),
   }
            thread::sleep(Duration::from_millis(10));
        }
        cleanup
    }
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    pub(crate) fn recover_native_temporaries(&self) -> Result<()> {
        let owner = match self.native_file_owner() {
            Ok(owner) => owner,
            Err(StorageError::IdentityMismatch) => return Ok(()),
            Err(error) => return Err(error),
        };
        let files = self.metadata_files()?;
        let custody = files.try_custody()?;
        for temporary in owner.registered_temporaries()? {
            let (scope, destination, kind) =
                self.operation_temp_scope(temporary.operation.clone(), temporary.item, false)?;
            if scope != temporary.scope_sha256
                || temporary.destination.as_ref().is_some_and(|d| {
                    kind != OperationKind::Export || destination.as_ref() != Some(d)
                })
            {
                return Err(StorageError::SnapshotChanged);
            }
            files.recover_registered_temporary(&custody, &temporary)?;
            owner.forget_temporary(&temporary)?;
        }
        files.require_empty_staging()
    }
}

//! Real SQLite job source. Each fetch owns one item, not an operation-wide Vec.
use super::{TerminalRepository, creation, rows};
use crate::{
    Result, StorageError,
    database::{Pending, TransactionOutcome},
};
use librairii_app::ports::{
    creation::NewOperation,
    jobs::{JobStore, StoreResult, WorkItem},
};
use librairii_core::operations::{CompletionAggregate, ItemState, RawOutcome};
use rusqlite::{OptionalExtension, params};

fn read<T>(pending: Result<Pending<'_, T>>) -> StoreResult<T> {
    match pending.and_then(Pending::wait) {
        Ok(value) => StoreResult::Known(value),
        Err(_) => StoreResult::Retry,
    }
}
fn write<T>(pending: Result<Pending<'_, TransactionOutcome<T>>>) -> StoreResult<T> {
    match pending {
        Err(StorageError::QueueFull | StorageError::IntakeClosed | StorageError::ProfileBusy) => {
            StoreResult::Retry
        }
        Err(_) => StoreResult::Rejected,
        Ok(p) => match p.wait() {
            Ok(TransactionOutcome::Committed(value)) => StoreResult::Known(value),
            Ok(TransactionOutcome::ProvenNotCommitted(
                StorageError::SnapshotChanged | StorageError::OperationInput(_),
            )) => StoreResult::Rejected,
            Ok(TransactionOutcome::ProvenNotCommitted(_)) => StoreResult::Retry,
            _ => StoreResult::Unknown,
        },
    }
}
fn invalid() -> StorageError {
    crate::StorageError::SnapshotChanged
}
impl JobStore for TerminalRepository<'_, '_> {
    fn generation(&self) -> u64 {
        self.generation
    }
    fn create(&self, input: &NewOperation, at: &str) -> StoreResult<()> {
        if input.validate().is_err() {
            return StoreResult::Rejected;
        }
        let input = input.clone();
        let at = at.to_owned();
        write(self.database.transaction(Default::default(),move|tx| {
            if input.kind==librairii_core::OperationKind::MetadataSync {
                let other:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM file_operations WHERE kind='metadata_sync' AND status IN ('queued','running') AND id<>?)",[&input.id],|r|r.get(0))?;
                if other{return Err(invalid());}
            }
            creation::create(tx,&input,&at)?;Ok(())
        }))
    }
    fn creation_readback(&self, input: &NewOperation, at: &str) -> StoreResult<bool> {
        match read(
            self.database
                .readback_operation_creation(input.clone(), at.to_owned()),
        ) {
            StoreResult::Known(value) => StoreResult::Known(value.is_some()),
            StoreResult::Rejected => StoreResult::Rejected,
            _ => StoreResult::Unknown,
        }
    }
    fn next_pending(&self, excluded: &[i64]) -> StoreResult<Option<WorkItem>> {
        // At most W running + 2W queued + one source slot. Exclusions are bound
        // parameters; raw IDs and creation order keep the legacy FIFO ordering.
        if excluded.len() > 25 {
            return StoreResult::Rejected;
        }
        let excluded = excluded.to_vec();
        read(self.database.barrier_read(move|tx| {
            let marks=std::iter::repeat_n("?",excluded.len()).collect::<Vec<_>>().join(",");
            let sql=format!("SELECT i.operation_id,i.id FROM file_operation_items i JOIN file_operations o ON o.id=i.operation_id WHERE o.status IN ('queued','running') AND i.status='pending' AND i.id NOT IN ({marks}) ORDER BY o.created_at,o.id,i.id LIMIT 1");
            let found:Option<(String,i64)>=tx.query_row(&sql,rusqlite::params_from_iter(excluded),|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
            let Some((operation,item))=found else{return Ok(None);};
            let v=rows::view(tx,&operation,Some(item))?.ok_or_else(invalid)?;
            Ok(Some(WorkItem{operation:v.operation.immutable_scope(),item:v.item.ok_or_else(invalid)?}))
        }))
    }
    fn start_item(&self, work: &WorkItem, at: &str) -> StoreResult<()> {
        let work = work.clone();
        let at = at.to_owned();
        write(self.database.repair_transaction(move|tx| {
            let v=rows::view(tx,&work.operation.identity.id,Some(work.item.identity.id))?.ok_or_else(invalid)?;
            if v.operation.immutable_scope()!=work.operation||v.item.as_ref().map(|i|i.immutable_scope())!=Some(work.item.immutable_scope()){return Err(invalid());}
            if v.item.is_some_and(|i|i.state==ItemState::Running){return Ok(());}
            if tx.execute("UPDATE file_operations SET status='running',started_at=COALESCE(started_at,?) WHERE id=? AND status IN ('queued','running')",params![at,work.operation.identity.id])?!=1 {return Err(invalid());}
            if tx.execute("UPDATE file_operation_items SET status='running' WHERE id=? AND operation_id=? AND status='pending'",params![work.item.identity.id,work.operation.identity.id])?!=1{return Err(invalid());}Ok(())
        }))
    }
    fn start_readback(&self, work: &WorkItem) -> StoreResult<bool> {
        let work = work.clone();
        read(self.database.barrier_read(move |tx| {
            let v = rows::view(tx, &work.operation.identity.id, Some(work.item.identity.id))?
                .ok_or_else(invalid)?;
            let item = v.item.ok_or_else(invalid)?;
            if v.operation.immutable_scope() != work.operation
                || item.immutable_scope() != work.item.immutable_scope()
            {
                return Err(invalid());
            }
            match item.state {
                ItemState::Running => Ok(true),
                ItemState::Pending => Ok(false),
                _ => Err(invalid()),
            }
        }))
    }
    fn ready_to_finish(
        &self,
    ) -> StoreResult<
        Option<(
            librairii_app::ports::operations::OperationScope,
            librairii_core::operations::TerminalResult,
        )>,
    > {
        read(self.database.barrier_read(|tx| {
            let id:Option<String>=tx.query_row("SELECT id FROM file_operations WHERE status IN ('queued','running') AND completed_items=total_items ORDER BY created_at,id LIMIT 1",[],|r|r.get(0)).optional()?;
            let Some(id)=id else{return Ok(None);};let header=rows::header(tx,&id)?.ok_or_else(invalid)?;
            let mut aggregate=CompletionAggregate::new(header.kind,header.counts.total).map_err(|_|invalid())?;
            let mut q=tx.prepare("SELECT status,outcome_code,outcome_message FROM file_operation_items WHERE operation_id=? ORDER BY id")?;
            let mut rows=q.query([&id])?;
            while let Some(row)=rows.next()? {let state:String=row.get(0)?;aggregate.record(ItemState::from_persisted(&state).map_err(|_|invalid())?,&RawOutcome{code:row.get(1)?,message:row.get(2)?}).map_err(|_|invalid())?;}
            Ok(Some((header.immutable_scope(),aggregate.terminal_result().map_err(|_|invalid())?)))
        }))
    }
    fn save_progress(&self, item: i64, completed: i64, total: i64) -> StoreResult<()> {
        if completed < 0 || total < 0 || completed > total {
            return StoreResult::Rejected;
        }
        write(self.database.repair_transaction(move|tx| {
            tx.execute("UPDATE file_operation_items SET completed_bytes=? WHERE id=? AND status='running' AND total_bytes=? AND completed_bytes<=? AND EXISTS(SELECT 1 FROM file_operations o WHERE o.id=operation_id AND o.status IN ('queued','running'))",params![completed,item,total,completed])?;Ok(())
        }))
    }
    fn cancelled(&self, id: &str) -> StoreResult<bool> {
        let id = id.to_owned();
        read(self.database.barrier_read(move |tx| {
            Ok(tx.query_row(
                "SELECT cancel_requested FROM file_operations WHERE id=?",
                [id],
                |r| r.get::<_, i64>(0),
            )? == 1)
        }))
    }
    fn request_cancel(&self, id: &str) -> StoreResult<()> {
        let id = id.to_owned();
        write(self.database.repair_transaction(move|tx| {
            if tx.execute("UPDATE file_operations SET cancel_requested=1 WHERE id=? AND status IN ('queued','running')",[id])?!=1 {return Err(invalid());}Ok(())
        }))
    }
    fn cancel_all(&self) -> StoreResult<()> {
        write(self.database.repair_transaction(|tx| {tx.execute("UPDATE file_operations SET cancel_requested=1 WHERE status IN ('queued','running')",[])?;Ok(())}))
    }
    fn fence(&self) {
        self.database.fence_operations();
    }
}

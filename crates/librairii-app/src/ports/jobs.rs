//! Blocking operation orchestration. Implementations borrow the actual admitted
//! library; these observations cannot authorize managed file deletion.
use super::{creation::NewOperation, operations::*};
use librairii_core::operations::TerminalResult;

#[derive(Debug)]
pub enum StoreResult<T> {
    Known(T),
    /// No write was admitted or a transaction positively rolled back.
    Retry,
    /// Required same-writer readback must precede another attempt.
    Unknown,
    /// Invalid/conflicting immutable input. No effect was admitted.
    Rejected,
}
#[derive(Clone, Debug)]
pub struct WorkItem {
    pub operation: OperationScope,
    pub item: OperationItem,
}
/// Call only on blocking runtime threads, never the paint/input thread.
/// Returned rows are owned observations; the borrow retains the real DB owner.
pub trait JobStore: OperationTerminalStore + Sync {
    fn generation(&self) -> u64;
    fn create(&self, input: &NewOperation, at: &str) -> StoreResult<()>;
    fn creation_readback(&self, input: &NewOperation, at: &str) -> StoreResult<bool>;
    fn next_pending(&self, excluded: &[i64]) -> StoreResult<Option<WorkItem>>;
    fn start_item(&self, work: &WorkItem, at: &str) -> StoreResult<()>;
    fn start_readback(&self, work: &WorkItem) -> StoreResult<bool>;
    fn ready_to_finish(&self) -> StoreResult<Option<(OperationScope, TerminalResult)>>;
    fn cancelled(&self, id: &str) -> StoreResult<bool>;
    fn request_cancel(&self, id: &str) -> StoreResult<()>;
    fn cancel_all(&self) -> StoreResult<()>;
    /// Any unexpected runtime panic permanently prevents a clean library close.
    fn fence(&self);
}

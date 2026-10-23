//! Partial tasks 6.1/6.7: legacy operation identities and effect-free policy.
//!
//! The string codecs follow the frozen Go `internal/operations/model.go`.
//! Transition and aggregate policies follow `repository.go` and
//! `manager.go:937-1024`. A returned candidate is not a database acknowledgement,
//! import receipt, cleanup permission or proof of committed effects. This module
//! performs no I/O and does not implement the JSON snapshot envelope, scheduling,
//! persistence, recovery, cancellation effects or shutdown.

mod codec;
mod data;
mod history;
mod policy;

pub use codec::{
    ExportGroupKind, ExportSourceType, ItemState, OperationCodecError, decode_kind, decode_state,
    encode_kind, encode_state, is_terminal,
};
pub use data::{
    ExportSource, HistoryKey, ObservationKind, OperationObservation, RawOutcome, StoredExportScope,
    StoredItemIdentity, StoredOperationIdentity,
};
pub use history::{
    HISTORY_LOOKAHEAD, HISTORY_PAGE_SIZE, HistoryCursor, HistoryError, HistoryWindow,
    OperationViewRow, active_and_latest, history_window, reveal_eligible,
};
pub use policy::{
    CompletionAggregate, CompletionCandidate, Counts, InterruptionCandidate, ItemRecord,
    OperationPolicyError, Progress, TerminalResult, checked_byte_total, complete_item, finish,
    interrupt, mark_item_running, mark_running, request_cancel,
};

/// Frozen Go whitespace predicate for operation input, independent of the host
/// Unicode version. It grants no path or persistence authority.
pub fn text_present(value: &str) -> bool {
    !crate::collection_codec::tag_trim(value).is_empty()
}
pub fn valid_output_name(value: &str) -> bool {
    text_present(value)
        && crate::collection_codec::tag_trim(value) == value
        && !matches!(value, "." | "..")
        && !value.contains(['/', '\\', '\0'])
}

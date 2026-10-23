//! Value-level view selection from the frozen Go History/Active policies.
//! Supplied observations are not authoritative database reads or path receipts.
use super::{HistoryKey, ObservationKind, OperationObservation, StoredOperationIdentity};
use crate::{OperationKind, OperationState};

pub const HISTORY_PAGE_SIZE: usize = 20;
pub const HISTORY_LOOKAHEAD: usize = HISTORY_PAGE_SIZE + 1;

/// Raw identities and opaque payloads stay borrowed. Selection never decodes,
/// clones or rewrites item order, scope, outcomes, timestamps or private paths.
/// As in the source table, each supplied stored operation has a unique raw ID.
#[derive(Debug)]
pub struct OperationViewRow<'data, T: ?Sized> {
    pub identity: &'data StoredOperationIdentity,
    pub kind: OperationKind,
    pub observation: OperationObservation,
    pub payload: &'data T,
}

/// The source resolves a nonempty raw before-ID in a separate lookup. A found
/// key need not occur in the candidate window. Start corresponds to an empty
/// raw cursor; Missing is never inferred from candidate-window absence.
#[derive(Clone, Copy, Debug)]
pub enum HistoryCursor<'key> {
    Start,
    Found(&'key HistoryKey),
    Missing,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum HistoryError {
    #[error("history cursor is unavailable")]
    MissingCursor,
}

#[derive(Debug)]
pub struct HistoryWindow<'rows, 'data, T: ?Sized> {
    pub operations: Vec<&'rows OperationViewRow<'data, T>>,
    /// The twentieth original ID only when a twenty-first candidate exists.
    /// This is raw text, not UUID Display or a serialized time/ID envelope.
    pub next_before: Option<&'data str>,
}

fn creation_key<'data, T: ?Sized>(row: &OperationViewRow<'data, T>) -> (&'data str, &'data str) {
    (&row.identity.created_at, &row.identity.id)
}

fn terminal_export_key<'data, T: ?Sized>(
    row: &OperationViewRow<'data, T>,
) -> (&'data str, &'data str) {
    (
        row.identity
            .finished_at
            .as_deref()
            .unwrap_or(&row.identity.created_at),
        &row.identity.id,
    )
}

/// Select up to twenty rows, retaining at most twenty-one candidate references.
/// This bound covers selection only, not caller-owned input, item payloads or
/// active-operation output. Candidate order may be arbitrary. Raw tuple `<`
/// excludes newer inserts after a cursor; older inserts may still appear.
///
/// The caller supplies the lookup and candidate observations. This function
/// establishes no SQL completeness, atomic snapshot, generation or durability.
pub fn history_window<'rows, 'data, T: ?Sized>(
    rows: &'rows [OperationViewRow<'data, T>],
    cursor: HistoryCursor<'_>,
) -> Result<HistoryWindow<'rows, 'data, T>, HistoryError> {
    let before = match cursor {
        HistoryCursor::Start => None,
        HistoryCursor::Found(key) => Some((key.created_at.as_str(), key.id.as_str())),
        HistoryCursor::Missing => return Err(HistoryError::MissingCursor),
    };
    let mut selected: Vec<&OperationViewRow<'data, T>> = Vec::with_capacity(HISTORY_LOOKAHEAD);
    for row in rows {
        if row.observation.kind != ObservationKind::StoredSnapshot
            || before.is_some_and(|before| creation_key(row) >= before)
        {
            continue;
        }
        let position = selected
            .iter()
            .position(|other| creation_key(row) > creation_key(other))
            .unwrap_or(selected.len());
        if position < HISTORY_LOOKAHEAD {
            // Never insert a twenty-second reference or collect the full input.
            if selected.len() == HISTORY_LOOKAHEAD {
                selected.pop();
            }
            selected.insert(position, row);
        }
    }
    let next_before = if selected.len() > HISTORY_PAGE_SIZE {
        selected.pop();
        selected.last().map(|row| row.identity.id.as_str())
    } else {
        None
    };
    Ok(HistoryWindow {
        operations: selected,
        next_before,
    })
}

/// All supplied stored queued/running rows in raw creation order, followed by
/// ONE latest stored terminal export. The appended export is not globally
/// re-sorted with active rows. This output has no History20 or operation cap.
/// `COALESCE(finished_at, created_at)` falls back for NULL only, not empty text.
pub fn active_and_latest<'rows, 'data, T: ?Sized>(
    rows: &'rows [OperationViewRow<'data, T>],
) -> Vec<&'rows OperationViewRow<'data, T>> {
    let mut active: Vec<_> = rows
        .iter()
        .filter(|row| {
            row.observation.kind == ObservationKind::StoredSnapshot
                && matches!(
                    row.observation.state,
                    OperationState::Queued | OperationState::Running
                )
        })
        .collect();
    active.sort_unstable_by(|a, b| creation_key(b).cmp(&creation_key(a)));
    let latest = rows
        .iter()
        .filter(|row| row.kind == OperationKind::Export && row.observation.is_saved_terminal())
        .max_by(|a, b| terminal_export_key(a).cmp(&terminal_export_key(b)));
    if let Some(latest) = latest {
        active.push(latest);
    }
    active
}

/// Only the application-level eligibility predicate. Nonempty whitespace,
/// relative paths and NUL text still require actual host path validation.
/// Success never creates a directory/reveal/activation/custody capability.
pub fn reveal_eligible(
    kind: OperationKind,
    observation: OperationObservation,
    raw_destination: Option<&str>,
) -> bool {
    kind == OperationKind::Export
        && observation.is_saved_terminal()
        && raw_destination.is_some_and(|path| !path.is_empty())
}

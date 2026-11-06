//! Domain predicates and SQL, always on the existing storage worker transaction.
use super::rows;
use crate::Result;
use librairii_app::ports::operations::{
    CommandWitness, OperationScope, TerminalView, UnknownOutcome,
};
use librairii_core::{
    OperationState,
    operations::{encode_state, is_terminal},
};
use rusqlite::{Transaction, params};

pub(super) enum Inspection {
    Unapplied(Box<TerminalView>),
    Matching(Box<TerminalView>),
    Conflict(Box<TerminalView>),
    Unknown(UnknownOutcome),
}

pub(super) fn scope(witness: &CommandWitness) -> &OperationScope {
    match witness {
        CommandWitness::CompleteItem(c) => &c.operation,
        CommandWitness::Finish(c) => &c.operation,
        CommandWitness::Interrupt(c) => &c.operation,
    }
}

pub(super) fn generation(witness: &CommandWitness) -> u64 {
    match witness {
        CommandWitness::CompleteItem(c) => c.generation,
        CommandWitness::Finish(c) => c.generation,
        CommandWitness::Interrupt(c) => c.generation,
    }
}

pub(super) fn item_id(witness: &CommandWitness) -> Option<i64> {
    match witness {
        CommandWitness::CompleteItem(c) => Some(c.item.id),
        _ => None,
    }
}

pub(super) fn valid(witness: &CommandWitness) -> bool {
    if scope(witness).identity.id.is_empty() {
        return false;
    }
    match witness {
        CommandWitness::CompleteItem(c) => {
            c.state.is_terminal()
                && c.item.operation_id == c.operation.identity.id
                && c.completed_bytes >= 0
                && c.completed_bytes <= c.item.total_bytes
        }
        CommandWitness::Finish(c) => is_terminal(c.state) && c.state != OperationState::Interrupted,
        // Frozen Go requires exact nonempty values, without trimming.
        CommandWitness::Interrupt(c) => !c.code.is_empty() && !c.message.is_empty(),
    }
}

pub(super) fn inspect(tx: &Transaction<'_>, witness: &CommandWitness) -> Result<Inspection> {
    let expected = scope(witness);
    let Some(view) = rows::view(tx, &expected.identity.id, item_id(witness))? else {
        return Ok(Inspection::Unknown(UnknownOutcome::Missing));
    };
    if view.operation.immutable_scope() != *expected
        || view.operation.counts.validate().is_err()
        || view.siblings.items != view.operation.counts.total
        || view.siblings.terminal != view.operation.counts.completed
    {
        return Ok(Inspection::Unknown(UnknownOutcome::Contradictory));
    }
    let view = Box::new(view);
    let matching = match witness {
        CommandWitness::CompleteItem(c) => {
            let Some(item) = view.item.as_ref() else {
                return Ok(Inspection::Unknown(UnknownOutcome::Missing));
            };
            if item.immutable_scope() != c.item {
                return Ok(Inspection::Unknown(UnknownOutcome::Contradictory));
            }
            item.state == c.state
                && item.identity.story_id == c.stored_story_id()
                && item.progress.completed == c.completed_bytes
                && item.outcome == c.stored_outcome()
        }
        CommandWitness::Finish(c) => {
            view.operation.counts.completed == view.operation.counts.total
                && view.operation.state == c.state
                && view.operation.outcome == c.stored_outcome()
                && view.operation.identity.finished_at.as_deref() == Some(c.finished_at.as_str())
        }
        CommandWitness::Interrupt(c) => {
            view.operation.counts.completed == view.operation.counts.total
                && view.operation.state == OperationState::Interrupted
                && view.operation.outcome.code.as_deref() == Some(c.code.as_str())
                && view.operation.outcome.message.as_deref() == Some(c.message.as_str())
                && view.operation.identity.finished_at.as_deref() == Some(c.finished_at.as_str())
        }
    };
    if matching {
        return Ok(Inspection::Matching(view));
    }
    if is_terminal(view.operation.state)
        || view
            .item
            .as_ref()
            .is_some_and(|item| item.state.is_terminal())
    {
        return Ok(Inspection::Conflict(view));
    }
    // A coherent active parent (and, for Complete, an active exact input item)
    // is positive nonapplication evidence. Absence or a failed read is not.
    Ok(Inspection::Unapplied(view))
}

fn exactly_one(rows: usize) -> Result<()> {
    if rows != 1 {
        // A real transaction-body error forces storage rollback, including an
        // earlier item update. Never return Ok(domain refusal) after mutation.
        return Err(rusqlite::Error::QueryReturnedNoRows.into());
    }
    Ok(())
}

pub(super) fn write(tx: &Transaction<'_>, witness: &CommandWitness) -> Result<()> {
    let id = &scope(witness).identity.id;
    match witness {
        CommandWitness::CompleteItem(c) => {
            exactly_one(tx.execute(
                "UPDATE file_operation_items SET story_id = ?, status = ?,
                 outcome_code = NULLIF(?, ''), outcome_message = NULLIF(?, ''), completed_bytes = ?
                 WHERE id = ? AND operation_id = ? AND status IN ('pending','running')",
                params![
                    c.stored_story_id(),
                    c.state.as_persisted(),
                    c.outcome.code.as_deref().unwrap_or(""),
                    c.outcome.message.as_deref().unwrap_or(""),
                    c.completed_bytes,
                    c.item.id,
                    id
                ],
            )?)?;
            exactly_one(tx.execute(
                "UPDATE file_operations SET completed_items = completed_items + 1
                 WHERE id = ? AND status IN ('queued','running') AND completed_items < total_items",
                [id],
            )?)?;
        }
        CommandWitness::Finish(c) => {
            exactly_one(tx.execute(
                "UPDATE file_operations SET status = ?, error_code = NULLIF(?, ''),
                 error_message = NULLIF(?, ''), finished_at = ?
                 WHERE id = ? AND status IN ('queued','running') AND completed_items = total_items",
                params![
                    encode_state(c.state),
                    c.outcome.code.as_deref().unwrap_or(""),
                    c.outcome.message.as_deref().unwrap_or(""),
                    c.finished_at,
                    id
                ],
            )?)?;
        }
        CommandWitness::Interrupt(c) => {
            tx.execute(
                "UPDATE file_operation_items SET status = 'cancelled', outcome_code = ?, outcome_message = ?
                 WHERE operation_id = ? AND status IN ('pending','running')",
                params![c.code, c.message, id],
            )?;
            exactly_one(tx.execute(
                "UPDATE file_operations SET status = 'interrupted', completed_items = total_items,
                 error_code = ?, error_message = ?, finished_at = ?
                 WHERE id = ? AND status IN ('queued','running')",
                params![c.code, c.message, c.finished_at, id],
            )?)?;
        }
    }
    Ok(())
}

use super::{catalog, definition, value};
use crate::Result;
use librairii_core::tagging::{
    AssignmentResult, AssignmentWorkspace, DefinitionState, Kind, TagError, ValueState, story_ids,
};
use rusqlite::{Transaction, params};
use std::collections::BTreeSet;
fn require(tx: &Transaction<'_>, ids: &[i64], definition_id: i64, kind: Kind) -> Result<()> {
    let d = definition(tx, definition_id)?;
    if d.kind != kind {
        return Err(TagError::AssignmentKind.into());
    }
    if d.source == "derived" {
        return Err(TagError::DerivedAssignment.into());
    }
    require_stories(tx, ids)
}
fn require_stories(tx: &Transaction<'_>, ids: &[i64]) -> Result<()> {
    for id in ids {
        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM stories WHERE id=?)",
            [id],
            |r| r.get(0),
        )?;
        if !exists {
            return Err(TagError::StoryNotFound.into());
        }
    }
    Ok(())
}
fn require_value(tx: &Transaction<'_>, id: i64, definition_id: i64) -> Result<()> {
    if value(tx, id)?.definition_id != definition_id {
        return Err(TagError::ValueNotFound.into());
    }
    Ok(())
}
fn set(
    tx: &Transaction<'_>,
    story: i64,
    definition: i64,
    value: Option<i64>,
    assigned: bool,
) -> Result<usize> {
    Ok(if assigned {
        tx.execute("INSERT INTO story_tag_assignments(story_id,definition_id,value_id,source) VALUES(?,?,?,'manual') ON CONFLICT DO NOTHING",params![story,definition,value])?
    } else {
        tx.execute("DELETE FROM story_tag_assignments WHERE story_id=? AND definition_id=? AND value_id IS ? AND source='manual'",params![story,definition,value])?
    })
}
fn toggle_inner(
    tx: &Transaction<'_>,
    ids: &[i64],
    definition: i64,
    value: Option<i64>,
    assigned: bool,
) -> Result<AssignmentResult> {
    let mut result = AssignmentResult {
        requested_stories: ids.len(),
        ..AssignmentResult::default()
    };
    for &id in ids {
        let changed = set(tx, id, definition, value, assigned)?;
        result.changed_stories += changed;
        if assigned {
            result.assignments_added += changed;
        } else {
            result.assignments_removed += changed;
        }
    }
    Ok(result)
}
pub(super) fn boolean(
    tx: &Transaction<'_>,
    ids: &[i64],
    definition: i64,
    assigned: bool,
) -> Result<AssignmentResult> {
    let ids = story_ids(ids)?;
    require(tx, &ids, definition, Kind::Boolean)?;
    toggle_inner(tx, &ids, definition, None, assigned)
}
pub(super) fn toggle(
    tx: &Transaction<'_>,
    ids: &[i64],
    definition: i64,
    value: i64,
    assigned: bool,
) -> Result<AssignmentResult> {
    let ids = story_ids(ids)?;
    if value <= 0 {
        return Err(TagError::InvalidAssignment.into());
    }
    require(tx, &ids, definition, Kind::Choice)?;
    require_value(tx, value, definition)?;
    toggle_inner(tx, &ids, definition, Some(value), assigned)
}
pub(super) fn replace(
    tx: &Transaction<'_>,
    ids: &[i64],
    definition: i64,
    values: &[i64],
) -> Result<AssignmentResult> {
    let ids = story_ids(ids)?;
    if values.iter().any(|&v| v <= 0) {
        return Err(TagError::InvalidAssignment.into());
    }
    require(tx, &ids, definition, Kind::Choice)?;
    let desired: BTreeSet<_> = values.iter().copied().collect();
    for &v in &desired {
        require_value(tx, v, definition)?;
    }
    let mut result = AssignmentResult {
        requested_stories: ids.len(),
        ..AssignmentResult::default()
    };
    for id in ids {
        let mut stmt=tx.prepare("SELECT value_id FROM story_tag_assignments WHERE story_id=? AND definition_id=? AND value_id IS NOT NULL AND source='manual'")?;
        let current = stmt
            .query_map(params![id, definition], |r| r.get::<_, i64>(0))?
            .collect::<rusqlite::Result<BTreeSet<_>>>()?;
        let mut changed = false;
        for &v in current.difference(&desired) {
            result.assignments_removed += set(tx, id, definition, Some(v), false)?;
            changed = true;
        }
        for &v in desired.difference(&current) {
            result.assignments_added += set(tx, id, definition, Some(v), true)?;
            changed = true;
        }
        result.changed_stories += usize::from(changed);
    }
    Ok(result)
}
/// Catalog, target existence and mixed-state counts share one reader snapshot.
pub fn workspace(tx: &Transaction<'_>, ids: &[i64]) -> Result<AssignmentWorkspace> {
    let ids = story_ids(ids)?;
    require_stories(tx, &ids)?;
    let catalog = catalog(tx)?;
    let targets = serde_json::to_string(&ids).map_err(|_| TagError::InvalidAssignment)?;
    let mut states = Vec::with_capacity(catalog.len());
    for e in &catalog {
        let mut state = DefinitionState {
            definition_id: e.definition.id,
            assigned_stories: 0,
            values: vec![],
        };
        if e.definition.kind == Kind::Boolean {
            state.assigned_stories=tx.query_row("SELECT COUNT(DISTINCT story_id) FROM story_tag_assignments WHERE definition_id=? AND value_id IS NULL AND story_id IN (SELECT value FROM json_each(?))",params![e.definition.id,targets],|r|r.get(0))?;
        } else {
            for v in &e.values {
                let count=tx.query_row("SELECT COUNT(DISTINCT story_id) FROM story_tag_assignments WHERE definition_id=? AND value_id=? AND story_id IN (SELECT value FROM json_each(?))",params![e.definition.id,v.id,targets],|r|r.get(0))?;
                state.values.push(ValueState {
                    value_id: v.id,
                    assigned_stories: count,
                });
            }
        }
        states.push(state);
    }
    Ok(AssignmentWorkspace {
        catalog,
        requested_stories: ids.len(),
        states,
    })
}

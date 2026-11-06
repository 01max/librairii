use super::{choice, definition, user, value};
use crate::Result;
use librairii_core::{
    collection_codec::{self, References},
    tagging::{Definition, TagError, Value},
};
use rusqlite::{OptionalExtension, Transaction};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeleteTarget {
    Definition(i64),
    Value(i64),
}
#[derive(Clone, Debug, Eq, PartialEq)]
struct Reference {
    id: i64,
    version: i64,
    payload: Vec<u8>,
    validity: String,
    revision: i64,
}
/// Confirmation data plus an opaque snapshot. It is never serialized authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeletionPlan {
    pub target: DeleteTarget,
    pub definition: Definition,
    pub value: Option<Value>,
    pub value_count: i64,
    pub assignment_count: i64,
    pub affected_collection_ids: Vec<i64>,
    references: Vec<Reference>,
}
fn matches(q: &References, target: DeleteTarget) -> bool {
    match target {
        DeleteTarget::Definition(id) => {
            q.boolean_filters
                .iter()
                .any(|f| f.definition_id == id && f.state != "ignored")
                || q.choice_filters.iter().any(|f| f.definition_id == id)
        }
        DeleteTarget::Value(id) => q.choice_filters.iter().any(|f| f.value_ids.contains(&id)),
    }
}
fn references(tx: &Transaction<'_>, target: DeleteTarget) -> Result<Vec<Reference>> {
    let legacy = tx
        .query_row(
            "SELECT definition_id FROM legacy_broken_tag WHERE singleton=1",
            [],
            |r| r.get::<_, i64>(0),
        )
        .optional()?
        .unwrap_or(0);
    let mut stmt=tx.prepare("SELECT id,query_version,CAST(query_payload AS BLOB),validity_state,revision FROM collections ORDER BY id")?;
    let mut rows = stmt.query([])?;
    let mut out = vec![];
    while let Some(row) = rows.next()? {
        let r = Reference {
            id: row.get(0)?,
            version: row.get(1)?,
            payload: row.get(2)?,
            validity: row.get(3)?,
            revision: row.get(4)?,
        };
        match collection_codec::decode_references(r.version, &r.payload, legacy) {
            Ok(q) if matches(&q, target) => out.push(r),
            Ok(_) => {}
            Err(collection_codec::CodecError::FrozenData) => {
                return Err(crate::StorageError::CompatibilityInvariant);
            }
            Err(_) if r.validity == "needs_attention" => {}
            Err(_) => return Err(TagError::SavedQueryInvalid.into()),
        }
    }
    Ok(out)
}
pub fn plan_deletion(tx: &Transaction<'_>, target: DeleteTarget) -> Result<DeletionPlan> {
    let (d, v, value_count, assignments) = match target {
        DeleteTarget::Definition(id) => {
            let d = definition(tx, id)?;
            user(&d)?;
            let values = tx.query_row(
                "SELECT COUNT(*) FROM tag_values WHERE definition_id=?",
                [id],
                |r| r.get(0),
            )?;
            let assignments = tx.query_row(
                "SELECT COUNT(*) FROM story_tag_assignments WHERE definition_id=?",
                [id],
                |r| r.get(0),
            )?;
            (d, None, values, assignments)
        }
        DeleteTarget::Value(id) => {
            let v = value(tx, id)?;
            let d = definition(tx, v.definition_id)?;
            choice(&d)?;
            let assignments = tx.query_row(
                "SELECT COUNT(*) FROM story_tag_assignments WHERE value_id=?",
                [id],
                |r| r.get(0),
            )?;
            (d, Some(v), 1, assignments)
        }
    };
    let references = references(tx, target)?;
    Ok(DeletionPlan {
        target,
        definition: d,
        value: v,
        value_count,
        assignment_count: assignments,
        affected_collection_ids: references.iter().map(|r| r.id).collect(),
        references,
    })
}
pub(super) fn delete(tx: &Transaction<'_>, plan: DeletionPlan) -> Result<()> {
    let current = plan_deletion(tx, plan.target)?;
    if current != plan {
        return Err(TagError::DeletePlanStale.into());
    }
    for id in &plan.affected_collection_ids {
        tx.execute("UPDATE collections SET validity_state='needs_attention',updated_at=CURRENT_TIMESTAMP WHERE id=? AND validity_state='valid'",[id])?;
    }
    let (table, id) = match plan.target {
        DeleteTarget::Definition(id) => ("tag_definitions", id),
        DeleteTarget::Value(id) => ("tag_values", id),
    };
    tx.execute(&format!("DELETE FROM {table} WHERE id=?"), [id])?;
    Ok(())
}

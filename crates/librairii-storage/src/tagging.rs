//! Tag repositories. Every command uses the existing owned writer transaction.
mod age;
mod assignments;
pub use age::{after_story_create_age, ensure_age_facets, project_age_snapshot};
mod deletion;
use crate::{
    Result, StorageError,
    database::{Database, Pending, TransactionOutcome, WorkCancellation},
};
pub use assignments::workspace;
pub use deletion::{DeleteTarget, DeletionPlan, plan_deletion};
use librairii_core::tagging::{
    self, CatalogEntry, CreateDefinition, Definition, Kind, TagError, Value,
};
use rusqlite::{OptionalExtension, Row, Transaction, params};

#[derive(Debug)]
pub enum Command {
    Delete(Box<DeletionPlan>),
    Boolean {
        story_ids: Vec<i64>,
        definition_id: i64,
        assigned: bool,
    },
    ChoiceToggle {
        story_ids: Vec<i64>,
        definition_id: i64,
        value_id: i64,
        assigned: bool,
    },
    ChoiceValues {
        story_ids: Vec<i64>,
        definition_id: i64,
        value_ids: Vec<i64>,
    },
    CreateDefinition(CreateDefinition),
    RenameDefinition {
        id: i64,
        label: String,
    },
    RecolorDefinition {
        id: i64,
        color: String,
    },
    ReorderDefinitions(Vec<i64>),
    CreateValue {
        definition_id: i64,
        key: String,
        label: String,
    },
    RenameValue {
        id: i64,
        label: String,
    },
    ReorderValues {
        definition_id: i64,
        ids: Vec<i64>,
    },
}
#[derive(Debug)]
pub enum Response {
    Deleted,
    Assignment(librairii_core::tagging::AssignmentResult),
    Definition(Definition),
    Value(Value),
    Catalog(Vec<CatalogEntry>),
}
impl Database<'_> {
    pub fn edit_tags(
        &self,
        command: Command,
        cancellation: WorkCancellation,
    ) -> Result<Pending<'_, TransactionOutcome<Response>>> {
        self.transaction(cancellation, move |tx| execute(tx, command))
    }
    pub fn seed_age_facets(
        &self,
        config: librairii_core::tagging::AgeConfig,
        cancellation: WorkCancellation,
    ) -> Result<Pending<'_, TransactionOutcome<i64>>> {
        self.transaction(
            cancellation,
            move |tx| Ok(ensure_age_facets(tx, &config)?.0),
        )
    }
    pub fn rebuild_age_projection(
        &self,
        snapshot: i64,
        config: librairii_core::tagging::AgeConfig,
        cancellation: WorkCancellation,
    ) -> Result<Pending<'_, TransactionOutcome<librairii_core::tagging::ProjectionResult>>> {
        self.transaction(cancellation, move |tx| {
            project_age_snapshot(tx, snapshot, &config)
        })
    }
    pub fn tag_deletion_plan(&self, target: DeleteTarget) -> Result<Pending<'_, DeletionPlan>> {
        self.read(move |tx| plan_deletion(tx, target))
    }
    pub fn assignment_workspace(
        &self,
        ids: Vec<i64>,
    ) -> Result<Pending<'_, librairii_core::tagging::AssignmentWorkspace>> {
        self.read(move |tx| workspace(tx, &ids))
    }
    /// One bounded-page projection on the owned reader. Boolean and choice
    /// labels remain independent of color and never reconstruct assignments.
    pub fn story_tag_labels(
        &self,
        ids: Vec<i64>,
    ) -> Result<Pending<'_, std::collections::BTreeMap<i64, Vec<tagging::DisplayTag>>>> {
        let ids = if ids.is_empty() {
            ids
        } else {
            tagging::story_ids(&ids)?
        };
        self.read(move|tx|{
            let mut labels=std::collections::BTreeMap::<i64,Vec<tagging::DisplayTag>>::new();
            let targets=serde_json::to_string(&ids).map_err(|_|TagError::InvalidAssignment)?;
            let mut statement=tx.prepare("SELECT a.story_id,d.label,v.label,d.color FROM story_tag_assignments a JOIN tag_definitions d ON d.id=a.definition_id LEFT JOIN tag_values v ON v.id=a.value_id WHERE a.story_id IN (SELECT value FROM json_each(?)) ORDER BY CASE d.source WHEN 'builtin' THEN 0 WHEN 'user' THEN 1 ELSE 2 END,d.position,d.id,v.position,v.id")?;
            for row in statement.query_map([targets],|row|Ok((row.get::<_,i64>(0)?,tagging::DisplayTag{definition:row.get(1)?,value:row.get(2)?,color:row.get(3)?})))?{
                let (id,label)=row?;labels.entry(id).or_default().push(label);
            }
            Ok(labels)
        })
    }
    pub fn tag_catalog(&self) -> Result<Pending<'_, Vec<CatalogEntry>>> {
        self.read(catalog)
    }
}
fn definition_row(r: &Row<'_>) -> rusqlite::Result<Definition> {
    let kind: String = r.get(5)?;
    Ok(Definition {
        id: r.get(0)?,
        key: r.get(1)?,
        normalized_key: r.get(2)?,
        label: r.get(3)?,
        color: r.get(4)?,
        kind: match kind.as_str() {
            "boolean" => Kind::Boolean,
            "choice" => Kind::Choice,
            _ => return Err(rusqlite::Error::InvalidQuery),
        },
        source: r.get(6)?,
        presentation: r.get(7)?,
        position: r.get(8)?,
        protected: r.get(9)?,
    })
}
const DEFINITION: &str = "SELECT id,key,normalized_key,label,color,kind,source,presentation,position,is_protected FROM tag_definitions";
pub(super) fn definition(tx: &Transaction<'_>, id: i64) -> Result<Definition> {
    tx.query_row(&format!("{DEFINITION} WHERE id=?"), [id], definition_row)
        .optional()?
        .ok_or(TagError::DefinitionNotFound.into())
}
pub(super) fn user(d: &Definition) -> Result<()> {
    if d.source != "user" || d.protected {
        return Err(TagError::ProtectedDefinition.into());
    }
    Ok(())
}
pub(super) fn choice(d: &Definition) -> Result<()> {
    if d.kind != Kind::Choice {
        return Err(TagError::ValuesNotAllowed.into());
    }
    user(d)
}
fn value_row(r: &Row<'_>) -> rusqlite::Result<Value> {
    Ok(Value {
        id: r.get(0)?,
        definition_id: r.get(1)?,
        key: r.get(2)?,
        normalized_key: r.get(3)?,
        label: r.get(4)?,
        position: r.get(5)?,
    })
}
pub(super) fn value(tx: &Transaction<'_>, id: i64) -> Result<Value> {
    tx.query_row(
        "SELECT id,definition_id,key,normalized_key,label,position FROM tag_values WHERE id=?",
        [id],
        value_row,
    )
    .optional()?
    .ok_or(TagError::ValueNotFound.into())
}
pub fn catalog(tx: &Transaction<'_>) -> Result<Vec<CatalogEntry>> {
    let mut stmt=tx.prepare(&format!("{DEFINITION} ORDER BY CASE source WHEN 'builtin' THEN 0 WHEN 'user' THEN 1 ELSE 2 END,position,id"))?;
    let definitions = stmt
        .query_map([], definition_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    definitions.into_iter().map(|definition| {
        let mut stmt=tx.prepare("SELECT id,definition_id,key,normalized_key,label,position FROM tag_values WHERE definition_id=? ORDER BY position,id")?;
        let values=stmt.query_map([definition.id],value_row)?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(CatalogEntry{definition,values})
    }).collect()
}
fn unique(error: rusqlite::Error, duplicate: TagError) -> StorageError {
    match &error {
        rusqlite::Error::SqliteFailure(e, _) if matches!(e.extended_code, 2067 | 1555) => {
            duplicate.into()
        }
        _ => error.into(),
    }
}
fn value_key(s: &str) -> Result<String> {
    tagging::normalize_key(s).map_err(|_| TagError::InvalidValue.into())
}
fn value_label(s: &str) -> Result<String> {
    tagging::normalize_label(s).map_err(|_| TagError::InvalidValue.into())
}
pub fn execute(tx: &Transaction<'_>, command: Command) -> Result<Response> {
    match command {
        Command::Delete(plan) => {
            deletion::delete(tx, *plan)?;
            Ok(Response::Deleted)
        }
        Command::Boolean {
            story_ids,
            definition_id,
            assigned,
        } => Ok(Response::Assignment(assignments::boolean(
            tx,
            &story_ids,
            definition_id,
            assigned,
        )?)),
        Command::ChoiceToggle {
            story_ids,
            definition_id,
            value_id,
            assigned,
        } => Ok(Response::Assignment(assignments::toggle(
            tx,
            &story_ids,
            definition_id,
            value_id,
            assigned,
        )?)),
        Command::ChoiceValues {
            story_ids,
            definition_id,
            value_ids,
        } => Ok(Response::Assignment(assignments::replace(
            tx,
            &story_ids,
            definition_id,
            &value_ids,
        )?)),
        Command::CreateDefinition(input) => {
            let key = tagging::normalize_key(&input.key)?;
            let label = tagging::normalize_label(&input.label)?;
            let color = tagging::normalize_color(&input.color)?;
            tx.execute("INSERT INTO tag_definitions(key,normalized_key,label,color,kind,source,position,is_protected) VALUES(?1,?1,?2,?3,?4,'user',(SELECT COALESCE(MAX(position),-1)+1 FROM tag_definitions WHERE source='user'),0)",params![key,label,color,input.kind.as_str()]).map_err(|e|unique(e,TagError::DuplicateDefinition))?;
            Ok(Response::Definition(definition(
                tx,
                tx.last_insert_rowid(),
            )?))
        }
        Command::RenameDefinition { id, label } => {
            let label = tagging::normalize_label(&label)?;
            user(&definition(tx, id)?)?;
            tx.execute(
                "UPDATE tag_definitions SET label=?,updated_at=CURRENT_TIMESTAMP WHERE id=?",
                params![label, id],
            )?;
            Ok(Response::Definition(definition(tx, id)?))
        }
        Command::RecolorDefinition { id, color } => {
            let color = tagging::normalize_color(&color)?;
            definition(tx, id)?;
            tx.execute(
                "UPDATE tag_definitions SET color=?,updated_at=CURRENT_TIMESTAMP WHERE id=?",
                params![color, id],
            )?;
            Ok(Response::Definition(definition(tx, id)?))
        }
        Command::CreateValue {
            definition_id,
            key,
            label,
        } => {
            let key = value_key(&key)?;
            let label = value_label(&label)?;
            choice(&definition(tx, definition_id)?)?;
            tx.execute("INSERT INTO tag_values(definition_id,key,normalized_key,label,position) VALUES(?1,?2,?2,?3,(SELECT COALESCE(MAX(position),-1)+1 FROM tag_values WHERE definition_id=?1))",params![definition_id,key,label]).map_err(|e|unique(e,TagError::DuplicateValue))?;
            Ok(Response::Value(value(tx, tx.last_insert_rowid())?))
        }
        Command::RenameValue { id, label } => {
            let label = value_label(&label)?;
            let v = value(tx, id)?;
            choice(&definition(tx, v.definition_id)?)?;
            tx.execute(
                "UPDATE tag_values SET label=?,updated_at=CURRENT_TIMESTAMP WHERE id=?",
                params![label, id],
            )?;
            Ok(Response::Value(value(tx, id)?))
        }
        Command::ReorderDefinitions(ids) => {
            reorder(tx, None, &ids)?;
            Ok(Response::Catalog(catalog(tx)?))
        }
        Command::ReorderValues { definition_id, ids } => {
            choice(&definition(tx, definition_id)?)?;
            reorder(tx, Some(definition_id), &ids)?;
            Ok(Response::Catalog(catalog(tx)?))
        }
    }
}
fn reorder(tx: &Transaction<'_>, parent: Option<i64>, ids: &[i64]) -> Result<()> {
    let (table, clause) = if parent.is_some() {
        ("tag_values", "definition_id=?")
    } else {
        ("tag_definitions", "source='user'")
    };
    let mut stmt = tx.prepare(&format!(
        "SELECT id,position FROM {table} WHERE {clause} ORDER BY position,id"
    ))?;
    let args = parent.into_iter().collect::<Vec<_>>();
    let current = stmt
        .query_map(rusqlite::params_from_iter(args.iter()), |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut expected = current.iter().map(|&(id, _)| id).collect::<Vec<_>>();
    expected.sort_unstable();
    let mut actual = ids.to_vec();
    actual.sort_unstable();
    if expected != actual || actual.windows(2).any(|w| w[0] == w[1]) {
        return Err(TagError::InvalidOrder.into());
    }
    // Reserve a disjoint positive range before swapping unique positions.
    let max = current.iter().map(|&(_, p)| p).max().unwrap_or(-1);
    let offset = max
        .checked_add(i64::try_from(ids.len()).map_err(|_| TagError::InvalidOrder)?)
        .and_then(|v| v.checked_add(1))
        .ok_or(TagError::InvalidOrder)?;
    max.checked_add(offset).ok_or(TagError::InvalidOrder)?;
    if !ids.is_empty() {
        let mut args = vec![offset];
        args.extend(parent);
        tx.execute(
            &format!(
                "UPDATE {table} SET position=position+?,updated_at=CURRENT_TIMESTAMP WHERE {clause}"
            ),
            rusqlite::params_from_iter(args),
        )?;
        for (position, id) in ids.iter().enumerate() {
            tx.execute(
                &format!("UPDATE {table} SET position=?,updated_at=CURRENT_TIMESTAMP WHERE id=?"),
                params![position as i64, id],
            )?;
        }
    }
    Ok(())
}

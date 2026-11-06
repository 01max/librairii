use crate::Result;
use librairii_core::tagging::{AgeConfig, Kind, ProjectionResult, TagError};
use rusqlite::{OptionalExtension, Transaction, params};
pub fn ensure_age_facets(tx: &Transaction<'_>, config: &AgeConfig) -> Result<(i64, Vec<i64>)> {
    config.validate()?;
    tx.execute("INSERT INTO tag_definitions(key,normalized_key,label,color,kind,source,presentation,position,is_protected) VALUES('age','age','Age','#267D5D','choice','derived','system',0,1) ON CONFLICT(normalized_key) DO NOTHING",[])?;
    let id = tx.query_row(
        "SELECT id FROM tag_definitions WHERE normalized_key='age' COLLATE NOCASE",
        [],
        |r| r.get::<_, i64>(0),
    )?;
    let d = super::definition(tx, id)?;
    if d.key != "age"
        || d.normalized_key != "age"
        || d.label != "Age"
        || d.kind != Kind::Choice
        || d.source != "derived"
        || d.presentation != "system"
        || d.position != 0
        || !d.protected
    {
        return Err(TagError::DerivedFacetDrift.into());
    }
    let mut ids = vec![];
    for (index, b) in config.bands.iter().enumerate() {
        tx.execute("INSERT INTO tag_values(definition_id,key,normalized_key,label,position) VALUES(?1,?2,?2,?3,?4) ON CONFLICT(definition_id,normalized_key) DO NOTHING",params![id,b.key,b.label,index as i64])?;
        let (v,key,normalized,label,position):(i64,String,String,String,i64)=tx.query_row("SELECT id,key,normalized_key,label,position FROM tag_values WHERE definition_id=? AND normalized_key=? COLLATE NOCASE",params![id,b.key],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?)))?;
        if key != b.key || normalized != b.key || label != b.label || position != index as i64 {
            return Err(TagError::DerivedFacetDrift.into());
        }
        ids.push(v);
    }
    let count: i64 = tx.query_row(
        "SELECT COUNT(*) FROM tag_values WHERE definition_id=?",
        [id],
        |r| r.get(0),
    )?;
    if count != config.bands.len() as i64 {
        return Err(TagError::DerivedFacetDrift.into());
    }
    Ok((id, ids))
}
pub fn project_age_snapshot(
    tx: &Transaction<'_>,
    snapshot: i64,
    config: &AgeConfig,
) -> Result<ProjectionResult> {
    if snapshot <= 0 {
        return Err(TagError::InvalidProjectionConfig.into());
    }
    let exists: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM catalog_snapshots WHERE id=?)",
        [snapshot],
        |r| r.get(0),
    )?;
    if !exists {
        return Err(TagError::InvalidProjectionConfig.into());
    }
    let (definition, values) = ensure_age_facets(tx, config)?;
    tx.execute(
        "DELETE FROM story_tag_assignments WHERE definition_id=? AND source='derived'",
        [definition],
    )?;
    let mut result = ProjectionResult {
        definition_id: definition,
        ..ProjectionResult::default()
    };
    let mut stmt=tx.prepare("SELECT s.id,m.minimum_age,m.maximum_age FROM official_story_metadata m JOIN stories s ON s.uuid=m.story_uuid WHERE m.snapshot_id=? ORDER BY s.id")?;
    let mut rows = stmt.query([snapshot])?;
    while let Some(row) = rows.next()? {
        let id: i64 = row.get(0)?;
        result.matched_stories += 1;
        if let Some(index) = config.matching_band(row.get(1)?, row.get(2)?) {
            tx.execute("INSERT INTO story_tag_assignments(story_id,definition_id,value_id,source) VALUES(?,?,?,'derived')",params![id,definition,values[index]])?;
            result.assigned_stories += 1;
        } else {
            result.unassigned += 1;
        }
    }
    Ok(result)
}
/// Compose inside the import writer transaction; no nested writer admission.
pub fn after_story_create_age(tx: &Transaction<'_>, story: i64, config: &AgeConfig) -> Result<()> {
    if story <= 0 {
        return Err(TagError::InvalidProjectionConfig.into());
    }
    let (definition, values) = ensure_age_facets(tx, config)?;
    let ages=tx.query_row("SELECT m.minimum_age,m.maximum_age FROM stories s JOIN official_story_metadata m ON m.story_uuid=s.uuid JOIN catalog_snapshots c ON c.id=m.snapshot_id WHERE s.id=? AND c.status='active' ORDER BY c.activated_at DESC,c.id DESC LIMIT 1",[story],|r|Ok((r.get::<_,Option<i64>>(0)?,r.get::<_,Option<i64>>(1)?))).optional()?;
    if let Some((minimum, maximum)) = ages
        && let Some(index) = config.matching_band(minimum, maximum)
    {
        tx.execute("INSERT INTO story_tag_assignments(story_id,definition_id,value_id,source) VALUES(?,?,?,'derived')",params![story,definition,values[index]])?;
    }
    Ok(())
}

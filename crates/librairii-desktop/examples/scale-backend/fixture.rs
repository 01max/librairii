use super::Result;
use librairii_core::collection_codec::{self, Query};
use librairii_storage::{
    StorageError,
    database::{Database, TransactionOutcome, WorkCancellation},
};
use rusqlite::params;
use serde::Deserialize;
use std::{fs, path::Path};

#[derive(Clone, Debug, Deserialize)]
pub struct Story {
    pub id: i64,
    pub uuid: String,
    pub embedded_title: String,
    pub display_name_normalized: String,
    pub embedded_artwork_path: String,
    pub created_at: String,
    pub is_broken: i64,
    pub byte_size: i64,
    pub language: String,
    pub minimum_age: i64,
    pub maximum_age: i64,
    pub official_title: String,
    pub validation_state: String,
}

pub fn committed<T>(outcome: TransactionOutcome<T>) -> librairii_storage::Result<T> {
    match outcome {
        TransactionOutcome::Committed(value) => Ok(value),
        TransactionOutcome::ProvenNotCommitted(error)
        | TransactionOutcome::CommitUncertain(error) => Err(error),
    }
}

pub fn criteria() -> Result<Vec<Query>> {
    [r#"{"name":"moon"}"#, r#"{"languages":["en-GB"]}"#,
        r#"{"compatibilities":["compatible"]}"#, "{}",
        r#"{"choiceFilters":[{"definitionId":2,"valueIds":[1]}]}"#,
        r#"{"name":"forest","languages":["en-GB"],"compatibilities":["compatible"],"choiceFilters":[{"definitionId":2,"valueIds":[1]}]}"#]
        .into_iter().map(|payload| Ok(collection_codec::decode(5, payload.as_bytes(), 0)?)).collect()
}

pub fn seed(db: &Database<'_>, repository: &Path, root: &Path) -> Result<Vec<Story>> {
    let source = repository.join("preservation/corpus/performance");
    let rows = fs::read_to_string(source.join("stories.jsonl"))?
        .lines()
        .map(serde_json::from_str)
        .collect::<std::result::Result<Vec<Story>, _>>()?;
    if rows.len() != 5000 || rows.iter().enumerate().any(|(n, s)| s.id != n as i64 + 1) {
        return Err("retained fixture must contain all 5000 exact ordered identities".into());
    }
    let artwork = root.join("catalog/embedded");
    fs::create_dir_all(&artwork)?;
    for number in 1..=24 {
        let name = format!("performance-cover-{number:02}.png");
        fs::copy(
            source.join("catalog/embedded").join(&name),
            artwork.join(name),
        )?;
    }
    let seeded = rows.clone();
    let queries = criteria()?
        .into_iter()
        .map(collection_codec::encode)
        .collect::<std::result::Result<Vec<_>, _>>()?;
    committed(db.transaction(WorkCancellation::default(), move |tx| {
        tx.execute_batch("INSERT INTO tag_definitions(id,key,normalized_key,label,color,kind,source,presentation,position,is_protected) VALUES(2,'mood','mood','Mood','#405CF5','choice','user','default',0,0);
            INSERT INTO tag_values(id,definition_id,key,normalized_key,label,position) VALUES(1,2,'calm','calm','Calm',0);
            INSERT INTO catalog_syncs(id,locale,status,started_at) VALUES('10000000-0000-4000-8000-000000000000','en-GB','running','2026-01-01T00:00:00Z');
            INSERT INTO catalog_snapshots(id,sync_id,locale,raw_path,raw_sha256,byte_size,record_count,status,fetched_at) VALUES(1,'10000000-0000-4000-8000-000000000000','en-GB','catalog/performance/catalog.json','ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff',1280000,5000,'staged','2026-01-01T00:00:00Z');")?;
        {
            let mut story = tx.prepare("INSERT INTO stories(id,uuid,embedded_title,embedded_description,embedded_artwork_path,display_name_normalized,is_broken,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?)")?;
            let mut archive = tx.prepare("INSERT INTO story_archives(id,story_id,original_filename,detected_format,sha256,byte_size,managed_path,validation_state,created_at) VALUES(?,?,?,'zip',?,?,?,?,?)")?;
            let mut metadata = tx.prepare("INSERT INTO official_story_metadata(snapshot_id,story_uuid,locale,title,title_normalized,description,author,publisher,language,duration_seconds,minimum_age,maximum_age,provenance,source_record_id) VALUES(1,?,'en-GB',?,?,?,'Librairii Fixture','Synthetic Press',?,?,?,?,'lunii_catalog',?)")?;
            for row in seeded {
                let description = format!("Copyright-free synthetic performance story {:05}.", row.id);
                story.execute(params![row.id,row.uuid,row.embedded_title,description,row.embedded_artwork_path,row.display_name_normalized,row.is_broken,row.created_at,row.created_at])?;
                let filename = format!("synthetic-story-{:05}.zip",row.id);
                let checksum = format!("{:064x}",row.id);
                archive.execute(params![row.id,row.id,filename,checksum,row.byte_size,format!("archives/{checksum}/{filename}"),row.validation_state,row.created_at])?;
                metadata.execute(params![row.uuid,row.official_title,row.display_name_normalized,description,row.language,300+row.id%1800,row.minimum_age,row.maximum_age,format!("synthetic-{:05}",row.id)])?;
                if row.id % 3 == 0 { tx.execute("INSERT INTO story_tag_assignments(story_id,definition_id,value_id,source) VALUES(?,2,1,'manual')",[row.id])?; }
            }
        }
        let names = ["Moon stories", "English stories", "Compatible stories", "All stories", "Calm stories", "Combined stories"];
        for (n,payload) in queries.iter().enumerate() {
            tx.execute("INSERT INTO collections(id,name,normalized_name,position,query_version,query_payload,validity_state) VALUES(?,?,?,?,5,?,'valid')",params![n as i64+1,names[n],names[n].to_lowercase(),n as i64,payload])?;
        }
        tx.execute_batch("UPDATE catalog_snapshots SET status='active',activated_at='2026-01-01T00:00:00Z' WHERE id=1;
            UPDATE catalog_syncs SET status='succeeded',matched_story_count=5000,finished_at='2026-01-01T00:00:00Z';")?;
        Ok(())
    })?.wait()?)?;
    committed(
        db.transaction(WorkCancellation::default(), |tx| {
            tx.execute_batch("ANALYZE")?;
            Ok(())
        })?
        .wait()?,
    )?;
    let counts = db
        .read(|tx| {
            let count: i64 = tx.query_row("SELECT COUNT(*) FROM stories", [], |row| row.get(0))?;
            if count != 5000 {
                return Err(StorageError::CompatibilityInvariant);
            }
            Ok(count)
        })?
        .wait()?;
    assert_eq!(counts, 5000);
    Ok(rows)
}

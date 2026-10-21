#![cfg(unix)]
use librairii_core::{
    collection_codec::Query,
    shelves::{Shelf, ShelfError},
};
use librairii_storage::{
    IsolatedLibrary, Result, StorageError,
    database::{Database, TransactionOutcome, WorkCancellation},
    shelves::{self, Command, Response},
};
use librairii_testkit::TestRoots;
use std::error::Error;
type TestResult<T = ()> = std::result::Result<T, Box<dyn Error>>;
fn committed<T>(v: TransactionOutcome<T>) -> Result<T> {
    match v {
        TransactionOutcome::Committed(v) => Ok(v),
        TransactionOutcome::ProvenNotCommitted(e) | TransactionOutcome::CommitUncertain(e) => {
            Err(e)
        }
    }
}
fn edit(db: &Database<'_>, command: Command) -> Result<Response> {
    committed(
        db.edit_shelves(command, WorkCancellation::default())?
            .wait()?,
    )
}
fn create(db: &Database<'_>, name: &str) -> Result<Shelf> {
    match edit(db, Command::Create { name: name.into() })? {
        Response::Shelf(s) => Ok(s),
        _ => Err(StorageError::CompatibilityInvariant),
    }
}
fn library(roots: &TestRoots) -> TestResult<IsolatedLibrary> {
    Ok(IsolatedLibrary::initialize_from_override(
        Some(roots.library().as_os_str()),
        "a0000000-0000-4000-8000-000000000001".parse()?,
    )?)
}
fn uuid(id: i64) -> String {
    format!("10000000-0000-4000-8000-{id:012}")
}
fn collection(db: &Database<'_>, name: &str, criteria: Query) -> Result<i64> {
    match committed(
        db.edit_collections(
            librairii_storage::collections::Command::Create {
                name: name.into(),
                criteria,
            },
            WorkCancellation::default(),
        )?
        .wait()?,
    )? {
        librairii_storage::collections::Response::Collection(c) => Ok(c.id),
        _ => Err(StorageError::CompatibilityInvariant),
    }
}
#[test]
fn crud_namespace_order_unavailable_duplicate_and_restart_are_independent() -> TestResult {
    let roots = TestRoots::new()?;
    let lib = library(&roots)?;
    let mut db = Database::open_isolated(&lib)?;
    collection(&db, "ÉTÉ", Query::default())?;
    let s = create(&db, " \tÉTÉ  ")?;
    assert_eq!(s.name, "ÉTÉ");
    assert!(matches!(
        create(&db, "ete"),
        Err(StorageError::Shelf(ShelfError::DuplicateName))
    ));
    let id = s.id;
    committed(db.transaction(WorkCancellation::default(),move|tx|{
        tx.execute("INSERT INTO shelf_stories(shelf_id,story_uuid,position,protected,cached_title) VALUES(?,?,7,0,'Gone')",rusqlite::params![id,uuid(1)])?;
        tx.execute("INSERT INTO shelf_exclusions(shelf_id,story_uuid) VALUES(?,?)",rusqlite::params![id,uuid(2)])?;
        tx.execute("INSERT INTO shelf_sources(shelf_id,collection_id,collection_name) VALUES(?,1,'Old name')",[id])?;
        let source=tx.last_insert_rowid();
        tx.execute("INSERT INTO shelf_story_origins(shelf_id,story_uuid,source_id) VALUES(?,?,?)",rusqlite::params![id,uuid(1),source])?;Ok(())
    })?.wait()?)?;
    let Response::Shelf(copy) = edit(
        &db,
        Command::Duplicate {
            id,
            name: "Independent".into(),
        },
    )?
    else {
        return Err("copy".into());
    };
    assert_eq!(
        (copy.count, copy.unavailable_count, copy.revision),
        (1, 1, 1)
    );
    let a = db.shelf_members(id)?.wait()?;
    let b = db.shelf_members(copy.id)?.wait()?;
    assert_eq!(a[0].cached_title, "Gone");
    assert_eq!(b[0].position, 7);
    assert!(!b[0].manually_retained);
    assert_ne!(a[0].origins[0].id, b[0].origins[0].id);
    edit(&db, Command::Reorder(vec![copy.id, id]))?;
    let before = db.shelves()?.wait()?;
    assert!(edit(&db, Command::Reorder(vec![id, id])).is_err());
    assert_eq!(db.shelves()?.wait()?, before);
    edit(
        &db,
        Command::Rename {
            id,
            name: "Changed".into(),
        },
    )?;
    edit(&db, Command::Delete { id })?;
    assert_eq!(db.shelf_members(copy.id)?.wait()?, b);
    db.close()?;
    let mut db = Database::open_isolated(&lib)?;
    assert_eq!(db.shelf_members(copy.id)?.wait()?, b);
    assert_eq!(db.collections()?.wait()?.len(), 1);
    db.close()?;
    Ok(())
}

fn seed(db: &Database<'_>, count: i64) -> Result<()> {
    committed(db.transaction(WorkCancellation::default(),move|tx|{for id in 1..=count {
        tx.execute("INSERT INTO stories(id,uuid,display_name_normalized) VALUES(?1,printf('10000000-0000-4000-8000-%012d',?1),'story')",[id])?;
        tx.execute("INSERT INTO story_archives(story_id,original_filename,detected_format,sha256,byte_size,managed_path,validation_state) VALUES(?1,'story.zip','zip',printf('%064d',?1),1,'archives/'||?1,'valid')",[id])?;
    }Ok(())})?.wait()?)
}

fn curate(db: &Database<'_>, id: i64, action: shelves::Curation) -> Result<Shelf> {
    committed(
        db.curate_shelf(id, action, "fr-FR".into(), WorkCancellation::default())?
            .wait()?,
    )
}
#[test]
fn manual_curation_retention_exclusions_order_and_noops_are_explicit() -> TestResult {
    use shelves::Curation::*;
    let roots = TestRoots::new()?;
    let lib = library(&roots)?;
    let mut db = Database::open_isolated(&lib)?;
    seed(&db, 3)?;
    let s = create(&db, "Manual")?;
    let s = curate(&db, s.id, Add(vec![uuid(2), uuid(1), uuid(2)]))?;
    assert_eq!(s.count, 2);
    assert_eq!(s.revision, 2);
    let unchanged = curate(&db, s.id, Add(vec![uuid(1)]))?;
    assert_eq!(s, unchanged);
    curate(&db, s.id, Reorder(vec![uuid(1), uuid(2)]))?;
    assert!(curate(&db, s.id, Reorder(vec![uuid(1), uuid(1)])).is_err());
    curate(&db, s.id, Remove(vec![uuid(1)]))?;
    let id = s.id;
    assert!(
        db.read(move |tx| shelves::exclusions(tx, id))?
            .wait()?
            .is_empty()
    );
    curate(&db, s.id, RemoveAndExclude(vec![uuid(2)]))?;
    let cleared = curate(&db, s.id, ClearExclusion(uuid(2)))?;
    assert_eq!(cleared.count, 0);
    curate(&db, s.id, Exclude(vec![uuid(3)]))?;
    curate(&db, s.id, Add(vec![uuid(3), uuid(2)]))?;
    let m = db.shelf_members(s.id)?.wait()?;
    assert_eq!(
        m.iter().map(|m| m.uuid.clone()).collect::<Vec<_>>(),
        vec![uuid(3), uuid(2)]
    );
    assert!(m.iter().all(|m| m.manually_retained));
    assert!(
        db.read(move |tx| shelves::exclusions(tx, id))?
            .wait()?
            .is_empty()
    );
    let before = db.shelf_members(s.id)?.wait()?;
    assert!(curate(&db, s.id, Add(vec![uuid(1), uuid(999)])).is_err());
    assert_eq!(db.shelf_members(s.id)?.wait()?, before);
    assert!(
        curate(
            &db,
            s.id,
            Add(vec!["10000000000040008000000000000001".into()])
        )
        .is_err()
    );
    db.close()?;
    Ok(())
}

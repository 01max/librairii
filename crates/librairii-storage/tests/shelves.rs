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
#[test]
fn uuid_reimport_reconnects_new_numeric_id_without_rewriting_approved_state() -> TestResult {
    use shelves::Curation::*;
    let roots = TestRoots::new()?;
    let lib = library(&roots)?;
    let mut db = Database::open_isolated(&lib)?;
    seed(&db, 2)?;
    let s = create(&db, "Durable")?;
    curate(&db, s.id, Add(vec![uuid(1), uuid(2)]))?;
    let id = s.id;
    let c = collection(&db, "All", Query::default())?;
    committed(db.transaction(WorkCancellation::default(),move|tx|{
        tx.execute("INSERT INTO shelf_sources(shelf_id,collection_id,collection_name) VALUES(?,?,'Accepted from All')",rusqlite::params![id,c])?;let src=tx.last_insert_rowid();
        tx.execute("INSERT INTO shelf_story_origins(shelf_id,story_uuid,source_id) VALUES(?,?,?)",rusqlite::params![id,uuid(1),src])?;
        tx.execute("UPDATE shelf_stories SET protected=0 WHERE shelf_id=? AND story_uuid=?",rusqlite::params![id,uuid(1)])?;
        tx.execute("INSERT INTO shelf_exclusions(shelf_id,story_uuid) VALUES(?,?)",rusqlite::params![id,uuid(9)])?;Ok(())
    })?.wait()?)?;
    let before = db.shelf_members(id)?.wait()?;
    committed(
        db.transaction(WorkCancellation::default(), |tx| {
            tx.execute("DELETE FROM stories WHERE id=1", [])?;
            Ok(())
        })?
        .wait()?,
    )?;
    let p = db
        .shelf_page(id, Default::default(), "fr-FR".into())?
        .wait()?;
    assert_eq!(p.total, 2);
    assert!(p.members[0].story.is_none());
    assert!(!p.members[0].member.cached_title.is_empty());
    assert_eq!(p.shelf.unavailable_count, 1);
    assert_eq!(db.shelf_members(id)?.wait()?, before);
    committed(db.transaction(WorkCancellation::default(),|tx|{
        tx.execute("INSERT INTO stories(id,uuid,display_name_normalized) VALUES(99,?,'reimported')",[uuid(1)])?;
        tx.execute("INSERT INTO story_archives(story_id,original_filename,detected_format,sha256,byte_size,managed_path,validation_state) VALUES(99,'new.zip','zip',printf('%064d',99),1,'archives/99','valid')",[])?;Ok(())
    })?.wait()?)?;
    let p = db
        .shelf_page(id, Default::default(), "fr-FR".into())?
        .wait()?;
    assert_eq!(p.members[0].story.as_ref().map(|s| s.id), Some(99));
    assert_eq!(p.shelf.unavailable_count, 0);
    assert_eq!(db.shelf_members(id)?.wait()?, before);
    db.close()?;
    let mut db = Database::open_isolated(&lib)?;
    assert_eq!(db.shelf_members(id)?.wait()?, before);
    db.close()?;
    Ok(())
}
#[test]
fn source_union_is_read_only_and_current_matches_do_not_rewrite_origins() -> TestResult {
    let roots = TestRoots::new()?;
    let lib = library(&roots)?;
    let mut db = Database::open_isolated(&lib)?;
    seed(&db, 3)?;
    let all = collection(&db, "All", Query::default())?;
    let same = collection(&db, "Same", Query::default())?;
    let s = create(&db, "Linked")?;
    committed(
        db.attach_shelf_sources(s.id, vec![all, same, all], WorkCancellation::default())?
            .wait()?,
    )?;
    let e = db.shelf_source_evaluation(s.id, "fr-FR".into())?.wait()?;
    assert_eq!(e.matching.len(), 3);
    assert_eq!(e.matching[&uuid(1)].len(), 2);
    assert!(!e.incomplete);
    assert_eq!(db.shelves()?.wait()?[0].count, 0);
    let id = s.id;
    committed(
        db.transaction(WorkCancellation::default(), move |tx| {
            tx.execute(
                "UPDATE collections SET validity_state='needs_attention' WHERE id=?",
                [same],
            )?;
            Ok(())
        })?
        .wait()?,
    )?;
    let before = db.collections()?.wait()?;
    let e = db.shelf_source_evaluation(id, "fr-FR".into())?.wait()?;
    assert!(e.incomplete);
    assert_eq!(e.matching.len(), 3);
    assert_eq!(e.matching[&uuid(1)].len(), 1);
    assert_eq!(db.collections()?.wait()?, before);
    assert_eq!(db.shelves()?.wait()?[0].count, 0);
    db.close()?;
    Ok(())
}
#[test]
fn initial_creation_is_explicit_deduplicated_ordered_and_can_be_empty() -> TestResult {
    use shelves::CreationRequest;
    let roots = TestRoots::new()?;
    let lib = library(&roots)?;
    let mut db = Database::open_isolated(&lib)?;
    seed(&db, 3)?;
    let a = collection(&db, "All", Query::default())?;
    let b = collection(&db, "Also", Query::default())?;
    let plan = db
        .shelf_creation_preview(
            CreationRequest {
                name: "Mixed".into(),
                manual_uuids: vec![uuid(2)],
                collection_ids: vec![a, b],
            },
            "fr-FR".into(),
        )?
        .wait()?;
    assert_eq!(plan.preview().candidates.len(), 3);
    assert_eq!(plan.preview().candidates[0].uuid, uuid(2));
    assert!(db.shelves()?.wait()?.is_empty());
    let copy = plan.clone();
    let s = committed(
        db.create_shelf_reviewed(plan, vec![uuid(1), uuid(2)], WorkCancellation::default())?
            .wait()?,
    )?;
    let m = db.shelf_members(s.id)?.wait()?;
    assert_eq!(
        m.iter().map(|m| m.uuid.clone()).collect::<Vec<_>>(),
        vec![uuid(2), uuid(1)]
    );
    assert!(m[0].manually_retained);
    assert!(!m[1].manually_retained);
    assert_eq!(m[1].origins.len(), 2);
    assert!(
        committed(
            db.create_shelf_reviewed(copy, vec![], WorkCancellation::default())?
                .wait()?
        )
        .is_err()
    );
    let plan = db
        .shelf_creation_preview(
            CreationRequest {
                name: "Empty linked".into(),
                collection_ids: vec![a],
                ..Default::default()
            },
            "fr-FR".into(),
        )?
        .wait()?;
    let empty = committed(
        db.create_shelf_reviewed(plan, vec![], WorkCancellation::default())?
            .wait()?,
    )?;
    assert_eq!((empty.count, empty.source_count, empty.revision), (0, 1, 1));
    let plan = db
        .shelf_creation_preview(
            CreationRequest {
                name: "Stale".into(),
                collection_ids: vec![a],
                ..Default::default()
            },
            "fr-FR".into(),
        )?
        .wait()?;
    committed(
        db.transaction(WorkCancellation::default(), |tx| {
            tx.execute("DELETE FROM stories WHERE id=3", [])?;
            Ok(())
        })?
        .wait()?,
    )?;
    assert!(matches!(
        committed(
            db.create_shelf_reviewed(plan, vec![uuid(1)], WorkCancellation::default())?
                .wait()?
        ),
        Err(StorageError::Shelf(ShelfError::StaleReview))
    ));
    db.close()?;
    Ok(())
}
fn preview(db: &Database<'_>, id: i64) -> Result<shelves::UpdatePlan> {
    db.shelf_update_preview(id, "fr-FR".into())?.wait()
}
fn apply(
    db: &Database<'_>,
    plan: shelves::UpdatePlan,
    decisions: Vec<shelves::Decision>,
) -> Result<Shelf> {
    committed(
        db.apply_shelf_updates(plan, decisions, WorkCancellation::default())?
            .wait()?,
    )
}
fn decision(id: i64, action: shelves::Action) -> shelves::Decision {
    shelves::Decision {
        uuid: uuid(id),
        action,
    }
}
#[test]
fn update_decisions_defer_keep_exclude_and_remove_atomically_with_origins() -> TestResult {
    use shelves::Action::*;
    let roots = TestRoots::new()?;
    let lib = library(&roots)?;
    let mut db = Database::open_isolated(&lib)?;
    seed(&db, 4)?;
    let a = collection(&db, "All", Query::default())?;
    let b = collection(&db, "Also", Query::default())?;
    let s = create(&db, "Review")?;
    committed(
        db.attach_shelf_sources(s.id, vec![a, b], WorkCancellation::default())?
            .wait()?,
    )?;
    let p = preview(&db, s.id)?;
    let before = db.shelves()?.wait()?;
    assert_eq!(p.preview().additions.len(), 4);
    assert!(apply(&db, p, vec![decision(1, Add), decision(1, Exclude)]).is_err());
    assert_eq!(db.shelves()?.wait()?, before);
    let p = preview(&db, s.id)?;
    assert_eq!(apply(&db, p, vec![])?, before[0]);
    let p = preview(&db, s.id)?;
    apply(
        &db,
        p,
        vec![
            decision(2, Add),
            decision(1, Add),
            decision(3, Exclude),
            decision(4, NotNow),
        ],
    )?;
    let m = db.shelf_members(s.id)?.wait()?;
    assert_eq!(m[0].uuid, uuid(1));
    assert_eq!(m[0].origins.len(), 2);
    let id = s.id;
    committed(db.transaction(WorkCancellation::default(),|tx|{tx.execute("UPDATE collections SET query_payload='{}',validity_state='needs_attention' WHERE id=2",[])?;tx.execute("UPDATE stories SET display_name_normalized='other' WHERE id IN (1,2)",[])?;tx.execute("UPDATE collections SET query_payload='{\"name\":\"story\"}' WHERE id=1",[])?;Ok(())})?.wait()?)?;
    let p = preview(&db, id)?;
    assert!(p.preview().incomplete);
    assert!(!p.preview().up_to_date);
    assert_eq!(
        p.preview()
            .additions
            .iter()
            .map(|c| c.uuid.clone())
            .collect::<Vec<_>>(),
        vec![uuid(4)]
    );
    assert!(p.preview().removals.is_empty());
    committed(db.transaction(WorkCancellation::default(),|tx|{tx.execute("UPDATE collections SET validity_state='valid',query_payload='{\"name\":\"story\"}' WHERE id=2",[])?;Ok(())})?.wait()?)?;
    let p = preview(&db, id)?;
    assert_eq!(p.preview().removals.len(), 2);
    apply(
        &db,
        p,
        vec![decision(1, Keep), decision(2, RemoveAndExclude)],
    )?;
    let p = preview(&db, id)?;
    assert!(p.preview().removals.is_empty());
    let m = db.shelf_members(id)?.wait()?;
    assert_eq!(m.len(), 1);
    assert!(m[0].manually_retained);
    assert_eq!(m[0].origins[0].name, "All");
    assert_eq!(
        db.read(move |tx| shelves::exclusions(tx, id))?.wait()?,
        vec![uuid(2), uuid(3)]
    );
    db.close()?;
    Ok(())
}
#[test]
fn broken_only_changes_preserve_review_but_sources_availability_and_decisions_invalidate()
-> TestResult {
    use shelves::Action::*;
    let roots = TestRoots::new()?;
    let lib = library(&roots)?;
    let mut db = Database::open_isolated(&lib)?;
    seed(&db, 3)?;
    let a = collection(&db, "All", Query::default())?;
    let s = create(&db, "Snapshot")?;
    committed(
        db.attach_shelf_sources(s.id, vec![a], WorkCancellation::default())?
            .wait()?,
    )?;
    let p = preview(&db, s.id)?;
    committed(
        db.transaction(WorkCancellation::default(), |tx| {
            tx.execute("UPDATE stories SET is_broken=1", [])?;
            Ok(())
        })?
        .wait()?,
    )?;
    apply(&db, p, vec![decision(1, Add)])?;
    let p = preview(&db, s.id)?;
    curate(&db, s.id, shelves::Curation::Exclude(vec![uuid(3)]))?;
    assert!(matches!(
        apply(&db, p, vec![decision(2, Add)]),
        Err(StorageError::Shelf(ShelfError::StaleReview))
    ));
    let p = preview(&db, s.id)?;
    committed(
        db.transaction(WorkCancellation::default(), |tx| {
            tx.execute("DELETE FROM stories WHERE id=2", [])?;
            Ok(())
        })?
        .wait()?,
    )?;
    assert!(apply(&db, p, vec![decision(2, Add)]).is_err());
    let p = preview(&db, s.id)?;
    committed(
        db.transaction(WorkCancellation::default(), |tx| {
            tx.execute("UPDATE stories SET is_broken=0", [])?;
            Ok(())
        })?
        .wait()?,
    )?;
    apply(&db, p, vec![])?;
    db.close()?;
    Ok(())
}

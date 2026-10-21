#![cfg(unix)]
use librairii_core::{
    CollectionId, ShelfId, StoryUuid,
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
fn story(id: i64) -> StoryUuid {
    let mut bytes = [0x10, 0, 0, 0, 0, 0, 0x40, 0, 0x80, 0, 0, 0, 0, 0, 0, 0];
    let mut n = id as u64;
    for byte in bytes[10..].iter_mut().rev() {
        *byte = (n % 10) as u8 | ((((n / 10) % 10) as u8) << 4);
        n /= 100;
    }
    StoryUuid::from_bytes(bytes)
}
fn uuid(id: i64) -> String {
    story(id).to_string()
}
fn collection(db: &Database<'_>, name: &str, criteria: Query) -> Result<CollectionId> {
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
        librairii_storage::collections::Response::Collection(c) => {
            CollectionId::new(c.id).map_err(|_| StorageError::CompatibilityInvariant)
        }
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
        tx.execute("INSERT INTO shelf_stories(shelf_id,story_uuid,position,protected,cached_title) VALUES(?,?,7,0,'Gone')",rusqlite::params![id.get(),uuid(1)])?;
        tx.execute("INSERT INTO shelf_exclusions(shelf_id,story_uuid) VALUES(?,?)",rusqlite::params![id.get(),uuid(2)])?;
        tx.execute("INSERT INTO shelf_sources(shelf_id,collection_id,collection_name) VALUES(?,1,'Old name')",[id.get()])?;
        let source=tx.last_insert_rowid();
        tx.execute("INSERT INTO shelf_story_origins(shelf_id,story_uuid,source_id) VALUES(?,?,?)",rusqlite::params![id.get(),uuid(1),source])?;Ok(())
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

fn curate(db: &Database<'_>, id: ShelfId, action: shelves::Curation) -> Result<Shelf> {
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
    let s = curate(&db, s.id, Add(vec![story(2), story(1), story(2)]))?;
    assert_eq!(s.count, 2);
    assert_eq!(s.revision, 2);
    let unchanged = curate(&db, s.id, Add(vec![story(1)]))?;
    assert_eq!(s, unchanged);
    curate(&db, s.id, Reorder(vec![story(1).into(), story(2).into()]))?;
    assert!(curate(&db, s.id, Reorder(vec![story(1).into(), story(1).into()])).is_err());
    curate(&db, s.id, Remove(vec![story(1).into()]))?;
    let id = s.id;
    assert!(
        db.read(move |tx| shelves::exclusions(tx, id))?
            .wait()?
            .is_empty()
    );
    curate(&db, s.id, RemoveAndExclude(vec![story(2).into()]))?;
    let cleared = curate(&db, s.id, ClearExclusion(story(2).into()))?;
    assert_eq!(cleared.count, 0);
    curate(&db, s.id, Exclude(vec![story(3).into()]))?;
    curate(&db, s.id, Add(vec![story(3), story(2)]))?;
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
    assert!(curate(&db, s.id, Add(vec![story(1), story(999)])).is_err());
    assert_eq!(db.shelf_members(s.id)?.wait()?, before);
    assert!(
        librairii_core::shelves::parse_uuids(&["10000000000040008000000000000001".into()]).is_err()
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
    curate(&db, s.id, Add(vec![story(1), story(2)]))?;
    let id = s.id;
    let c = collection(&db, "All", Query::default())?;
    committed(db.transaction(WorkCancellation::default(),move|tx|{
        tx.execute("INSERT INTO shelf_sources(shelf_id,collection_id,collection_name) VALUES(?,?,'Accepted from All')",rusqlite::params![id.get(),c.get()])?;let src=tx.last_insert_rowid();
        tx.execute("INSERT INTO shelf_story_origins(shelf_id,story_uuid,source_id) VALUES(?,?,?)",rusqlite::params![id.get(),uuid(1),src])?;
        tx.execute("UPDATE shelf_stories SET protected=0 WHERE shelf_id=? AND story_uuid=?",rusqlite::params![id.get(),uuid(1)])?;
        tx.execute("INSERT INTO shelf_exclusions(shelf_id,story_uuid) VALUES(?,?)",rusqlite::params![id.get(),uuid(9)])?;Ok(())
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
                [same.get()],
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
                manual_uuids: vec![story(2)],
                collection_ids: vec![a, b],
            },
            "fr-FR".into(),
        )?
        .wait()?;
    assert_eq!(plan.preview().candidate_count(), 3);
    assert_eq!(plan.preview().candidates_page(0)[0].uuid, uuid(2));
    assert!(db.shelves()?.wait()?.is_empty());
    let copy = plan.clone();
    let s = committed(
        db.create_shelf_reviewed(
            plan,
            vec![story(1).into(), story(2).into()],
            WorkCancellation::default(),
        )?
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
            db.create_shelf_reviewed(plan, vec![story(1).into()], WorkCancellation::default())?
                .wait()?
        ),
        Err(StorageError::Shelf(ShelfError::StaleReview))
    ));
    db.close()?;
    Ok(())
}
fn preview(db: &Database<'_>, id: ShelfId) -> Result<shelves::UpdatePlan> {
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
        uuid: story(id).into(),
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
    assert_eq!(p.preview().addition_count(), 4);
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
            .additions_page(0)
            .iter()
            .map(|c| c.uuid.clone())
            .collect::<Vec<_>>(),
        vec![uuid(4)]
    );
    assert!(p.preview().removal_count() == 0);
    committed(db.transaction(WorkCancellation::default(),|tx|{tx.execute("UPDATE collections SET validity_state='valid',query_payload='{\"name\":\"story\"}' WHERE id=2",[])?;Ok(())})?.wait()?)?;
    let p = preview(&db, id)?;
    assert_eq!(p.preview().removal_count(), 2);
    apply(
        &db,
        p,
        vec![decision(1, Keep), decision(2, RemoveAndExclude)],
    )?;
    let p = preview(&db, id)?;
    assert!(p.preview().removal_count() == 0);
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
    curate(&db, s.id, shelves::Curation::Exclude(vec![story(3).into()]))?;
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
#[test]
fn ticket_limit_is_shared_across_kinds_and_two_queued_applies_cannot_replay() -> TestResult {
    let roots = TestRoots::new()?;
    let lib = library(&roots)?;
    let mut db = Database::open_isolated(&lib)?;
    seed(&db, 1)?;
    let a = collection(&db, "All", Query::default())?;
    let s = create(&db, "Tickets")?;
    committed(
        db.attach_shelf_sources(s.id, vec![a], WorkCancellation::default())?
            .wait()?,
    )?;
    let oldest = preview(&db, s.id)?;
    for _ in 0..64 {
        db.shelf_creation_preview(
            shelves::CreationRequest {
                name: "Other".into(),
                ..Default::default()
            },
            "fr-FR".into(),
        )?
        .wait()?;
    }
    assert!(matches!(
        apply(&db, oldest, vec![]),
        Err(StorageError::Shelf(ShelfError::StaleReview))
    ));
    let p = preview(&db, s.id)?;
    let replay = p.clone();
    let first = db.apply_shelf_updates(
        p,
        vec![decision(1, shelves::Action::Add)],
        WorkCancellation::default(),
    )?;
    let second = db.apply_shelf_updates(
        replay,
        vec![decision(1, shelves::Action::Add)],
        WorkCancellation::default(),
    )?;
    assert_eq!(committed(first.wait()?)?.count, 1);
    assert!(matches!(
        committed(second.wait()?),
        Err(StorageError::Shelf(ShelfError::StaleReview))
    ));
    let foreign = preview(&db, s.id)?;
    db.close()?;
    let mut db = Database::open_isolated(&lib)?;
    assert!(
        db.apply_shelf_updates(foreign, vec![], WorkCancellation::default())
            .is_err()
    );
    db.close()?;
    Ok(())
}
fn sourced(
    db: &Database<'_>,
    name: &str,
    collections: Vec<CollectionId>,
    selected: Vec<StoryUuid>,
) -> Result<Shelf> {
    let p = db
        .shelf_creation_preview(
            shelves::CreationRequest {
                name: name.into(),
                collection_ids: collections,
                ..Default::default()
            },
            "fr-FR".into(),
        )?
        .wait()?;
    committed(
        db.create_shelf_reviewed(
            p,
            selected.into_iter().map(Into::into).collect(),
            WorkCancellation::default(),
        )?
        .wait()?,
    )
}
#[test]
fn detachment_requires_complete_decisions_preserves_overlap_and_retains_unavailable() -> TestResult
{
    let roots = TestRoots::new()?;
    let lib = library(&roots)?;
    let mut db = Database::open_isolated(&lib)?;
    seed(&db, 3)?;
    let a = collection(&db, "All", Query::default())?;
    let mut q = Query::default();
    q.name = "different".into();
    let b = collection(&db, "Other", q)?;
    let s = sourced(&db, "Detach", vec![a, b], vec![story(1), story(2)])?;
    committed(
        db.transaction(WorkCancellation::default(), |tx| {
            tx.execute(
                "UPDATE stories SET display_name_normalized='different' WHERE id=1",
                [],
            )?;
            tx.execute("DELETE FROM stories WHERE id=2", [])?;
            Ok(())
        })?
        .wait()?,
    )?;
    let p = db
        .shelf_source_change_preview(s.id, vec![b], "fr-FR".into())?
        .wait()?;
    assert_eq!(p.preview().affected_count(), 1);
    assert_eq!(p.preview().affected_page(0)[0].uuid, uuid(2));
    assert!(p.preview().affected_page(0)[0].story.is_none());
    let before = db.shelves()?.wait()?;
    assert!(
        committed(
            db.change_shelf_sources(
                p,
                false,
                vec![decision(2, shelves::Action::RemoveAndExclude)],
                WorkCancellation::default()
            )?
            .wait()?
        )
        .is_err()
    );
    assert_eq!(db.shelves()?.wait()?, before);
    let p = db
        .shelf_source_change_preview(s.id, vec![b], "fr-FR".into())?
        .wait()?;
    committed(
        db.change_shelf_sources(p, true, vec![], WorkCancellation::default())?
            .wait()?,
    )?;
    let m = db.shelf_members(s.id)?.wait()?;
    assert!(!m[0].manually_retained);
    assert!(m[1].manually_retained);
    assert!(!m[0].origins[0].active);
    committed(
        db.attach_shelf_sources(s.id, vec![a], WorkCancellation::default())?
            .wait()?,
    )?;
    assert!(db.shelf_members(s.id)?.wait()?[1].manually_retained);
    let p = db
        .shelf_source_change_preview(s.id, vec![], "fr-FR".into())?
        .wait()?;
    assert!(
        committed(
            db.change_shelf_sources(
                p,
                false,
                vec![decision(1, shelves::Action::NotNow)],
                WorkCancellation::default()
            )?
            .wait()?
        )
        .is_err()
    );
    let p = db
        .shelf_source_change_preview(s.id, vec![], "fr-FR".into())?
        .wait()?;
    committed(
        db.change_shelf_sources(p, true, vec![], WorkCancellation::default())?
            .wait()?,
    )?;
    assert!(
        db.shelf_members(s.id)?
            .wait()?
            .iter()
            .all(|m| m.manually_retained)
    );
    assert!(preview(&db, s.id)?.preview().up_to_date);
    db.close()?;
    Ok(())
}
#[test]
fn warning_acknowledgements_persist_without_decisions_and_renew_on_coverage_changes() -> TestResult
{
    use librairii_core::shelves::WarningAcknowledgements;
    use librairii_storage::profile::NativeProfileStore;
    let roots = TestRoots::new()?;
    let lib = library(&roots)?;
    let mut db = Database::open_isolated(&lib)?;
    seed(&db, 2)?;
    let a = collection(&db, "All", Query::default())?;
    let s = sourced(&db, "Warnings", vec![a], vec![story(1), story(2)])?;
    committed(
        db.transaction(WorkCancellation::default(), |tx| {
            tx.execute(
                "UPDATE collections SET query_payload='{\"name\":\"nothing\"}'",
                [],
            )?;
            Ok(())
        })?
        .wait()?,
    )?;
    let p = preview(&db, s.id)?;
    let key = p.mismatch_key().ok_or("mismatch key")?;
    let before = db.shelf_members(s.id)?.wait()?;
    let mut acknowledgements = WarningAcknowledgements::new(lib.identity().typed());
    assert!(acknowledgements.needs_prompt(s.id, key));
    acknowledgements.acknowledge(s.id, key)?;
    let store = NativeProfileStore::create_fresh(roots.profile())?;
    store.save_shelf_warnings(&acknowledgements)?;
    drop(store);
    let store = NativeProfileStore::open_native(roots.profile())?;
    let restored = store
        .load_shelf_warnings(lib.identity().typed())?
        .ok_or("preferences")?;
    assert!(!restored.needs_prompt(s.id, key));
    assert_eq!(db.shelf_members(s.id)?.wait()?, before);
    committed(
        db.transaction(WorkCancellation::default(), |tx| {
            tx.execute("UPDATE stories SET is_broken=1", [])?;
            Ok(())
        })?
        .wait()?,
    )?;
    assert_eq!(preview(&db, s.id)?.mismatch_key(), Some(key));
    committed(
        db.transaction(WorkCancellation::default(), |tx| {
            tx.execute("DELETE FROM stories WHERE id=2", [])?;
            Ok(())
        })?
        .wait()?,
    )?;
    let renewed = preview(&db, s.id)?
        .mismatch_key()
        .ok_or("new mismatch key")?;
    assert!(restored.needs_prompt(s.id, renewed));
    let path = roots.profile().join("shelf-warnings.json");
    let mut wire: serde_json::Value = serde_json::from_slice(&std::fs::read(&path)?)?;
    wire["version"] = 99.into();
    let bytes = serde_json::to_vec(&wire)?;
    std::fs::write(&path, &bytes)?;
    assert!(store.load_shelf_warnings(lib.identity().typed()).is_err());
    assert!(store.save_shelf_warnings(&acknowledgements).is_err());
    assert_eq!(std::fs::read(path)?, bytes);
    db.close()?;
    Ok(())
}
#[test]
fn linked_deletion_is_atomic_across_shelves_and_preserves_unavailable_origins() -> TestResult {
    let roots = TestRoots::new()?;
    let lib = library(&roots)?;
    let mut db = Database::open_isolated(&lib)?;
    seed(&db, 2)?;
    let a = collection(&db, "All", Query::default())?;
    let b = collection(&db, "Also", Query::default())?;
    let first = sourced(&db, "First", vec![a], vec![story(1), story(2)])?;
    let second = sourced(&db, "Second", vec![a, b], vec![story(1)])?;
    committed(db.transaction(WorkCancellation::default(),|tx|{tx.execute("DELETE FROM stories WHERE id=2",[])?;tx.execute_batch("CREATE TRIGGER fail_shelf_collection_delete BEFORE DELETE ON collections BEGIN SELECT RAISE(ABORT,'test failure'); END")?;Ok(())})?.wait()?)?;
    let p = db
        .linked_collection_deletion_preview(a, "fr-FR".into())?
        .wait()?;
    assert_eq!(p.preview().shelf_count(), 2);
    assert_eq!(
        p.preview().shelf(0).ok_or("first shelf")?.affected_count(),
        2
    );
    assert!(p.preview().shelf(1).ok_or("second shelf")?.affected_count() == 0);
    let before = db.shelf_members(first.id)?.wait()?;
    let before_shelves = db.shelves()?.wait()?;
    assert!(
        committed(
            db.delete_linked_collection(p, true, vec![], WorkCancellation::default())?
                .wait()?
        )
        .is_err()
    );
    assert_eq!(db.shelf_members(first.id)?.wait()?, before);
    assert_eq!(db.shelves()?.wait()?, before_shelves);
    assert_eq!(db.collections()?.wait()?.len(), 2);
    committed(
        db.transaction(WorkCancellation::default(), |tx| {
            tx.execute_batch("DROP TRIGGER fail_shelf_collection_delete")?;
            Ok(())
        })?
        .wait()?,
    )?;
    let p = db
        .linked_collection_deletion_preview(a, "fr-FR".into())?
        .wait()?;
    let receipt = committed(
        db.delete_linked_collection(
            p,
            false,
            vec![shelves::ShelfDecisions {
                shelf_id: first.id,
                decisions: vec![
                    decision(1, shelves::Action::RemoveAndExclude),
                    decision(2, shelves::Action::Keep),
                ],
            }],
            WorkCancellation::default(),
        )?
        .wait()?,
    )?;
    assert_eq!(receipt.id(), a.get());
    let m = db.shelf_members(first.id)?.wait()?;
    assert_eq!(m.len(), 1);
    assert_eq!(m[0].uuid, uuid(2));
    assert!(m[0].manually_retained);
    assert_eq!(m[0].origins[0].name, "All");
    assert_eq!(m[0].origins[0].collection_id, None);
    assert!(!m[0].origins[0].active);
    let m = db.shelf_members(second.id)?.wait()?;
    assert!(!m[0].manually_retained);
    assert_eq!(m[0].origins.len(), 2);
    assert_eq!(db.collections()?.wait()?.len(), 1);
    db.close()?;
    let mut db = Database::open_isolated(&lib)?;
    assert_eq!(db.shelf_members(first.id)?.wait()?[0].uuid, uuid(2));
    db.close()?;
    Ok(())
}
#[test]
fn five_thousand_story_union_preview_apply_and_every_model_page_meet_budget() -> TestResult {
    use std::time::{Duration, Instant};
    let roots = TestRoots::new()?;
    let lib = library(&roots)?;
    let mut db = Database::open_isolated(&lib)?;
    seed(&db, 5000)?;
    committed(db.transaction(WorkCancellation::default(),|tx|{tx.execute("UPDATE stories SET embedded_title=printf('Story %05d shared %s',id,CASE WHEN id%2=0 THEN 'moon' ELSE 'forest' END),display_name_normalized=printf('story %05d shared %s',id,CASE WHEN id%2=0 THEN 'moon' ELSE 'forest' END),is_broken=(id%5=0)",[])?;Ok(())})?.wait()?)?;
    let mut q = Query::default();
    q.name = "shared".into();
    let all = collection(&db, "All shared", q)?;
    let mut q = Query::default();
    q.name = "moon".into();
    let moon = collection(&db, "Moon", q)?;
    let s = create(&db, "Large")?;
    committed(
        db.attach_shelf_sources(s.id, vec![all, moon], WorkCancellation::default())?
            .wait()?,
    )?;
    let mut worst = Duration::ZERO;
    let mut p = preview(&db, s.id)?;
    for _ in 0..4 {
        let started = Instant::now();
        p = preview(&db, s.id)?;
        worst = worst.max(started.elapsed());
        assert_eq!(p.preview().addition_count(), 5000);
        assert_eq!(p.preview().additions_page(0).len(), 25);
        assert_eq!(p.preview().additions_page(199).len(), 25);
        assert!(p.preview().additions_page(usize::MAX).is_empty());
    }
    let selected = (0..2)
        .flat_map(|page| p.preview().additions_page(page))
        .map(|c| {
            Ok(shelves::Decision {
                uuid: c.reference()?,
                action: shelves::Action::Add,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let started = Instant::now();
    apply(&db, p, selected)?;
    let apply_time = started.elapsed();
    assert_eq!(preview(&db, s.id)?.preview().addition_count(), 4950);
    let opened = db
        .shelf_page(
            s.id,
            librairii_core::query::QueryRequest {
                page_size: 100,
                ..Default::default()
            },
            "fr-FR".into(),
        )?
        .wait()?;
    assert_eq!(opened.total, 50);
    assert_eq!(opened.members.len(), 25);
    assert_eq!(opened.members[0].member.origins.len(), 1);
    assert_eq!(opened.members[1].member.origins.len(), 2);
    let p = preview(&db, s.id)?;
    let excludes = p.preview().bulk_decisions(shelves::Action::Exclude)?;
    apply(&db, p, excludes)?;
    let excluded = db.shelf_exclusions_page(s.id, 0)?.wait()?;
    assert_eq!(excluded.total, 4950);
    assert_eq!(excluded.rows.len(), 25);
    assert!(
        db.shelf_exclusions_page(s.id, usize::MAX)?
            .wait()?
            .rows
            .is_empty()
    );
    let creation = db
        .shelf_creation_preview(
            shelves::CreationRequest {
                name: "Many".into(),
                collection_ids: vec![all, moon],
                ..Default::default()
            },
            "fr-FR".into(),
        )?
        .wait()?;
    assert_eq!(creation.preview().candidate_count(), 5000);
    assert_eq!(creation.preview().candidates_page(100).len(), 25);
    let detach = db
        .shelf_source_change_preview(s.id, vec![], "fr-FR".into())?
        .wait()?;
    assert_eq!(detach.preview().affected_count(), 50);
    assert_eq!(detach.preview().affected_page(0).len(), 25);
    eprintln!("5000 stories: worst preview={worst:?}; apply 50={apply_time:?}");
    assert!(worst < Duration::from_secs(5));
    assert!(apply_time < Duration::from_secs(5));
    db.close()?;
    Ok(())
}
fn source_json(s: &librairii_core::shelves::Source) -> serde_json::Value {
    serde_json::json!({"id":s.id,"collectionId":s.collection_id.map(CollectionId::get).unwrap_or(0),"name":s.name,"active":s.active})
}
fn candidates_json(candidates: &[shelves::Candidate]) -> Vec<serde_json::Value> {
    candidates.iter().map(|c|serde_json::json!({"uuid":c.uuid,"title":c.title,"manual":c.manual,"sources":c.sources.iter().map(source_json).collect::<Vec<_>>(),"origins":c.origins.iter().map(source_json).collect::<Vec<_>>()})).collect()
}
fn shelf_state(db: &Database<'_>, id: ShelfId) -> Result<serde_json::Value> {
    db.read(move|tx|{
    let s=shelves::read(tx,id)?;let members=shelves::members(tx,id)?.iter().map(|m|serde_json::json!({"uuid":m.uuid,"position":m.position,"protected":m.manually_retained,"cachedTitle":m.cached_title,"origins":m.origins.iter().map(source_json).collect::<Vec<_>>()})).collect::<Vec<_>>();
    Ok(serde_json::json!({"shelf":{"id":s.id.get(),"name":s.name,"position":s.position,"revision":s.revision,"count":s.count,"brokenCount":s.broken_count,"unavailableCount":s.unavailable_count,"sourceCount":s.source_count},"members":members,"sources":shelves::sources(tx,id,false)?.iter().map(source_json).collect::<Vec<_>>(),"exclusions":shelves::exclusions(tx,id)?}))
})?.wait()
}
#[test]
fn frozen_go_creation_curation_origins_duplication_and_detachment_match_exact_state() -> TestResult
{
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/shelves/reference.json"))?;
    let roots = TestRoots::new()?;
    let lib = library(&roots)?;
    let mut db = Database::open_isolated(&lib)?;
    seed(&db, 3)?;
    committed(
        db.transaction(WorkCancellation::default(), |tx| {
            tx.execute(
                "UPDATE stories SET embedded_title='Story'||id,display_name_normalized='story'||id",
                [],
            )?;
            Ok(())
        })?
        .wait()?,
    )?;
    let a = collection(&db, "All", Query::default())?;
    let b = collection(&db, "Also", Query::default())?;
    let mut events = Vec::new();
    let p = db
        .shelf_creation_preview(
            shelves::CreationRequest {
                name: "Chosen".into(),
                manual_uuids: vec![story(2)],
                collection_ids: vec![a, b],
            },
            "fr-FR".into(),
        )?
        .wait()?;
    events.push(serde_json::json!({"initial":candidates_json(p.preview().candidates_page(0)),"incomplete":p.preview().incomplete}));
    let s = committed(
        db.create_shelf_reviewed(
            p,
            vec![story(1).into(), story(2).into()],
            WorkCancellation::default(),
        )?
        .wait()?,
    )?;
    events.push(shelf_state(&db, s.id)?);
    let p = preview(&db, s.id)?;
    events.push(serde_json::json!({"additions":candidates_json(p.preview().additions_page(0)),"removals":candidates_json(p.preview().removals_page(0))}));
    apply(&db, p, vec![decision(3, shelves::Action::Exclude)])?;
    events.push(shelf_state(&db, s.id)?);
    curate(&db, s.id, shelves::Curation::Remove(vec![story(1).into()]))?;
    events.push(shelf_state(&db, s.id)?);
    let p = preview(&db, s.id)?;
    apply(&db, p, vec![decision(1, shelves::Action::Add)])?;
    events.push(shelf_state(&db, s.id)?);
    committed(
        db.transaction(WorkCancellation::default(), |tx| {
            tx.execute(
                "UPDATE collections SET query_payload='{\"name\":\"nothing\"}'",
                [],
            )?;
            Ok(())
        })?
        .wait()?,
    )?;
    let p = preview(&db, s.id)?;
    events.push(serde_json::json!({"additions":candidates_json(p.preview().additions_page(0)),"removals":candidates_json(p.preview().removals_page(0))}));
    apply(&db, p, vec![decision(1, shelves::Action::Keep)])?;
    events.push(shelf_state(&db, s.id)?);
    committed(
        db.transaction(WorkCancellation::default(), |tx| {
            tx.execute("DELETE FROM stories WHERE id=2", [])?;
            tx.execute("UPDATE stories SET is_broken=1", [])?;
            Ok(())
        })?
        .wait()?,
    )?;
    let Response::Shelf(copy) = edit(
        &db,
        Command::Duplicate {
            id: s.id,
            name: "Independent".into(),
        },
    )?
    else {
        return Err("duplicate".into());
    };
    events.push(shelf_state(&db, copy.id)?);
    let p = db
        .shelf_source_change_preview(s.id, vec![], "fr-FR".into())?
        .wait()?;
    committed(
        db.change_shelf_sources(p, true, vec![], WorkCancellation::default())?
            .wait()?,
    )?;
    events.push(shelf_state(&db, s.id)?);
    assert_eq!(serde_json::json!({"events":events}), reference);
    db.close()?;
    Ok(())
}

#[test]
fn frozen_shelf_reference_manifest_binds_probe_and_capture_bytes() -> TestResult {
    use sha2::{Digest, Sha256};
    let manifest: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/shelves/manifest.json"))?;
    assert_eq!(
        manifest["frozenRevision"],
        "73a93303a9a44cf0a92eaf525331561ed8c04460"
    );
    assert_eq!(manifest["repeatCapture"], "byte-identical");
    for (name, bytes) in [
        (
            "reference.json",
            include_bytes!("fixtures/shelves/reference.json").as_slice(),
        ),
        (
            "reference_probe.go.in",
            include_bytes!("fixtures/shelves/reference_probe.go.in").as_slice(),
        ),
    ] {
        let record = manifest["files"]
            .as_array()
            .ok_or("files")?
            .iter()
            .find(|r| r["path"] == name)
            .ok_or("record")?;
        assert_eq!(record["bytes"], bytes.len());
        assert_eq!(
            record["sha256"],
            librairii_core::Checksum::from_bytes(Sha256::digest(bytes).into()).to_string()
        );
    }
    Ok(())
}
#[test]
fn v14_upgrade_tags_metadata_curation_unavailable_reimport_and_restart_compose() -> TestResult {
    use librairii_storage::tagging::{self, Command as TagCommand};
    let roots = TestRoots::new()?;
    let lib = library(&roots)?;
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../preservation/corpus/databases/prefix-014.sqlite3");
    let original = std::fs::read(&source)?;
    std::fs::write(lib.layout().database(), &original)?;
    let mut db = Database::open_isolated(&lib)?;
    committed(
        db.transaction(WorkCancellation::default(), |tx| {
            // This admitted schema fixture also carries a synthetic unfinished removal.
            // Settle that test-only precondition; Task 12 owns real trash recovery.
            tx.execute("DELETE FROM removal_intents", [])?;
            Ok(())
        })?
        .wait()?,
    )?;
    assert_eq!(db.broken_summary(vec![1])?.wait()?.flagged_stories, 1);
    assert_eq!(
        db.assignment_workspace(vec![1])?
            .wait()?
            .states
            .iter()
            .find(|s| s.definition_id == 8)
            .map(|s| s.assigned_stories),
        Some(1)
    );
    let legacy = "123e4567-e89b-42d3-a456-426614174000".to_string();
    let inspected = committed(
        db.edit_collections(
            librairii_storage::collections::Command::Inspect { id: 1 },
            WorkCancellation::default(),
        )?
        .wait()?,
    )?;
    let librairii_storage::collections::Response::Inspection(i) = inspected else {
        return Err("legacy inspection".into());
    };
    assert_eq!(i.collection.id, 1);
    assert_eq!(i.collection.query_version, 5);
    let s = sourced(
        &db,
        "Upgrade",
        vec![CollectionId::new(1)?],
        vec![legacy.parse()?],
    )?;
    let before = db.shelf_members(s.id)?.wait()?;
    committed(
        db.edit_tags(
            TagCommand::Boolean {
                story_ids: vec![1],
                definition_id: 8,
                assigned: false,
            },
            WorkCancellation::default(),
        )?
        .wait()?,
    )?;
    let obsolete = preview(&db, s.id)?;
    assert_eq!(obsolete.preview().removal_count(), 1);
    assert_eq!(db.shelf_members(s.id)?.wait()?, before);
    committed(
        db.edit_tags(
            TagCommand::Boolean {
                story_ids: vec![1],
                definition_id: 8,
                assigned: true,
            },
            WorkCancellation::default(),
        )?
        .wait()?,
    )?;
    assert!(matches!(
        apply(
            &db,
            obsolete,
            vec![shelves::Decision {
                uuid: librairii_core::shelves::StoryReference::from_stored(&legacy)?,
                action: shelves::Action::RemoveAndExclude
            }]
        ),
        Err(StorageError::Shelf(ShelfError::StaleReview))
    ));
    let obsolete = preview(&db, s.id)?;
    committed(db.transaction(WorkCancellation::default(),|tx|{
        tx.execute_batch("INSERT INTO catalog_syncs(id,locale,status,started_at) VALUES('10000000-0000-4000-8000-000000000001','fr-FR','running','2000'); INSERT INTO catalog_snapshots(id,sync_id,locale,raw_path,raw_sha256,byte_size,record_count,fetched_at) VALUES(1,'10000000-0000-4000-8000-000000000001','fr-FR','catalog/synthetic',printf('%064d',1),1,1,'2000'); INSERT INTO official_story_metadata(snapshot_id,story_uuid,locale,title,title_normalized,provenance) VALUES(1,'123e4567-e89b-42d3-a456-426614174000','fr-FR','Forest','forest','lunii_catalog'); UPDATE catalog_snapshots SET status='active',activated_at='2001' WHERE id=1;")?;
        Ok(())
    })?.wait()?)?;
    assert!(matches!(
        apply(&db, obsolete, vec![]),
        Err(StorageError::Shelf(ShelfError::StaleReview))
    ));
    let fresh = preview(&db, s.id)?;
    assert_eq!(fresh.preview().removals_page(0)[0].title, "Forest");
    apply(
        &db,
        fresh,
        vec![shelves::Decision {
            uuid: librairii_core::shelves::StoryReference::from_stored(&legacy)?,
            action: shelves::Action::Keep,
        }],
    )?;
    curate(&db, s.id, shelves::Curation::Exclude(vec![story(9).into()]))?;
    let id = s.id;
    let retained = db.shelf_members(id)?.wait()?;
    assert!(retained[0].manually_retained);
    assert_eq!(
        retained[0].origins[0].collection_id,
        Some(CollectionId::new(1)?)
    );
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
    assert_eq!(p.shelf.unavailable_count, 1);
    assert_eq!(p.members[0].member.cached_title, retained[0].cached_title);
    let old_ticket = preview(&db, id)?;
    assert_eq!(old_ticket.preview().removal_count(), 0);
    assert_eq!(old_ticket.preview().warning_count(), 1);
    db.close()?;
    let mut db = Database::open_isolated(&lib)?;
    assert_eq!(db.shelf_members(id)?.wait()?, retained);
    assert!(
        db.apply_shelf_updates(old_ticket, vec![], WorkCancellation::default())
            .is_err()
    );
    let reimport = legacy.clone();
    committed(db.transaction(WorkCancellation::default(),move|tx|{
        tx.execute("INSERT INTO stories(id,uuid,embedded_title,display_name_normalized) VALUES(101,?,'Reimported','reimported')",[reimport])?;
        tx.execute("INSERT INTO story_archives(story_id,original_filename,detected_format,sha256,byte_size,managed_path,validation_state) VALUES(101,'reimport.zip','zip',printf('%064d',101),1,'archives/101','valid')",[])?;
        tagging::execute(tx,TagCommand::Boolean{story_ids:vec![101],definition_id:8,assigned:true})?;Ok(())
    })?.wait()?)?;
    let p = db
        .shelf_page(id, Default::default(), "fr-FR".into())?
        .wait()?;
    assert_eq!(p.members[0].story.as_ref().map(|s| s.id), Some(101));
    assert_eq!(
        p.members[0].story.as_ref().map(|s| s.title.as_str()),
        Some("Forest")
    );
    assert_eq!(db.shelf_members(id)?.wait()?, retained);
    assert_eq!(
        db.shelf_exclusions_page(id, 0)?.wait()?.rows[0].uuid,
        uuid(9)
    );
    db.close()?;
    assert_eq!(std::fs::read(source)?, original);
    Ok(())
}
#[test]
fn uncertain_remaining_coverage_only_allows_retention_and_view_refinements_never_persist()
-> TestResult {
    let roots = TestRoots::new()?;
    let lib = library(&roots)?;
    let mut db = Database::open_isolated(&lib)?;
    seed(&db, 2)?;
    let a = collection(&db, "All", Query::default())?;
    let b = collection(&db, "Other", Query::default())?;
    let s = sourced(&db, "Conservative", vec![a, b], vec![story(1), story(2)])?;
    committed(
        db.transaction(WorkCancellation::default(), move |tx| {
            tx.execute(
                "UPDATE collections SET validity_state='needs_attention' WHERE id=?",
                [b.get()],
            )?;
            tx.execute("DELETE FROM stories WHERE id=2", [])?;
            Ok(())
        })?
        .wait()?,
    )?;
    let before = db.shelf_members(s.id)?.wait()?;
    let mut request = librairii_core::query::QueryRequest {
        sort: "name_asc".into(),
        ..Default::default()
    };
    request.criteria.name = " \t ".into();
    let p = db.shelf_page(s.id, request, "fr-FR".into())?.wait()?;
    assert_eq!(p.total, 2);
    assert!(p.members[1].story.is_none());
    assert_eq!(db.shelf_members(s.id)?.wait()?, before);
    let p = db
        .shelf_source_change_preview(s.id, vec![b], "fr-FR".into())?
        .wait()?;
    assert!(p.preview().incomplete);
    assert_eq!(p.preview().affected_count(), 2);
    assert!(
        p.preview()
            .bulk_decisions(shelves::Action::RemoveAndExclude)
            .is_err()
    );
    committed(
        db.change_shelf_sources(p, true, vec![], WorkCancellation::default())?
            .wait()?,
    )?;
    assert!(
        db.shelf_members(s.id)?
            .wait()?
            .iter()
            .all(|m| m.manually_retained)
    );
    db.close()?;
    Ok(())
}
#[test]
fn reviewed_actions_preserve_uppercase_uuid_spelling_without_retargeting_canonical_rows()
-> TestResult {
    use librairii_core::shelves::StoryReference;
    let roots = TestRoots::new()?;
    let lib = library(&roots)?;
    let mut db = Database::open_isolated(&lib)?;
    let uppercase = "10000000-0000-4000-8000-00000000000A";
    committed(db.transaction(WorkCancellation::default(),move|tx|{
        tx.execute("INSERT INTO stories(id,uuid,embedded_title,display_name_normalized) VALUES(100,?,'Legacy case','legacy case')",[uppercase])?;
        tx.execute("INSERT INTO story_archives(story_id,original_filename,detected_format,sha256,byte_size,managed_path,validation_state) VALUES(100,'legacy.zip','zip',printf('%064d',100),1,'archives/100','valid')",[])?;Ok(())
    })?.wait()?)?;
    let a = collection(&db, "All", Query::default())?;
    let s = create(&db, "Raw addition")?;
    committed(
        db.attach_shelf_sources(s.id, vec![a], WorkCancellation::default())?
            .wait()?,
    )?;
    let p = preview(&db, s.id)?;
    let decisions = p.preview().bulk_decisions(shelves::Action::Add)?;
    assert_eq!(decisions[0].uuid.as_stored(), uppercase);
    apply(&db, p, decisions)?;
    assert_eq!(db.shelf_members(s.id)?.wait()?[0].uuid, uppercase);
    for detached in [false, true] {
        for action in [shelves::Action::Keep, shelves::Action::RemoveAndExclude] {
            let s = create(&db, &format!("Case {detached} {action:?}"))?;
            committed(
                db.attach_shelf_sources(s.id, vec![a], WorkCancellation::default())?
                    .wait()?,
            )?;
            let id = s.id;
            committed(db.transaction(WorkCancellation::default(),move|tx|{
            tx.execute("INSERT INTO shelf_stories(shelf_id,story_uuid,position,protected,cached_title) VALUES(?,?,0,0,'Cached legacy')",rusqlite::params![id.get(),uppercase])?;Ok(())
        })?.wait()?)?;
            if detached {
                let p = db
                    .shelf_source_change_preview(s.id, vec![], "fr-FR".into())?
                    .wait()?;
                let decisions = p.preview().bulk_decisions(action)?;
                committed(
                    db.change_shelf_sources(p, false, decisions, WorkCancellation::default())?
                        .wait()?,
                )?;
            } else {
                committed(db.transaction(WorkCancellation::default(),move|tx|{tx.execute("UPDATE collections SET query_payload='{\"name\":\"nothing\"}' WHERE id=?",[a.get()])?;Ok(())})?.wait()?)?;
                let p = preview(&db, s.id)?;
                let decisions = p.preview().bulk_decisions(action)?;
                apply(&db, p, decisions)?;
                committed(
                    db.transaction(WorkCancellation::default(), move |tx| {
                        tx.execute(
                            "UPDATE collections SET query_payload='{}' WHERE id=?",
                            [a.get()],
                        )?;
                        Ok(())
                    })?
                    .wait()?,
                )?;
            }
            if action == shelves::Action::Keep {
                let m = db.shelf_members(s.id)?.wait()?;
                assert_eq!(m[0].uuid, uppercase);
                assert!(m[0].manually_retained);
                assert_eq!(m[0].reference()?.as_stored(), uppercase);
            } else {
                assert!(db.shelf_members(s.id)?.wait()?.is_empty());
                let x = db.shelf_exclusions_page(s.id, 0)?.wait()?;
                assert_eq!(x.rows[0].uuid, uppercase);
                curate(
                    &db,
                    s.id,
                    shelves::Curation::ClearExclusion(StoryReference::from_stored(uppercase)?),
                )?;
                assert_eq!(db.shelf_exclusions_page(s.id, 0)?.wait()?.total, 0);
            }
        }
    }
    let raw = StoryReference::from_stored(uppercase)?;
    assert_ne!(raw, raw.identity().into());
    db.close()?;
    Ok(())
}

#[test]
fn creation_selections_preserve_legacy_spelling_and_distinguish_canonical_aliases() -> TestResult {
    let roots = TestRoots::new()?;
    let lib = library(&roots)?;
    let mut db = Database::open_isolated(&lib)?;
    let uppercase = "10000000-0000-4000-8000-00000000000A";
    let lowercase = "10000000-0000-4000-8000-00000000000a";
    let c = collection(&db, "All", Query::default())?;
    for with_alias in [false, true] {
        let (id, uuid) = if with_alias {
            (101, lowercase)
        } else {
            (100, uppercase)
        };
        committed(db.transaction(WorkCancellation::default(), move |tx| {
            tx.execute("INSERT INTO stories(id,uuid,embedded_title,display_name_normalized) VALUES(?,?,'Legacy case','legacy case')", rusqlite::params![id, uuid])?;
            tx.execute("INSERT INTO story_archives(story_id,original_filename,detected_format,sha256,byte_size,managed_path,validation_state) VALUES(?1,'legacy.zip','zip',printf('%064d',?1),1,'archives/'||?1,'valid')", [id])?;
            Ok(())
        })?.wait()?)?;
        for bulk in [false, true] {
            let plan = db
                .shelf_creation_preview(
                    shelves::CreationRequest {
                        name: format!("Case {with_alias} {bulk}"),
                        collection_ids: vec![c],
                        ..Default::default()
                    },
                    "fr-FR".into(),
                )?
                .wait()?;
            let selected = if bulk {
                plan.preview().all_uuids()?
            } else {
                vec![
                    plan.preview()
                        .candidates_page(0)
                        .iter()
                        .find(|candidate| candidate.uuid == uppercase)
                        .ok_or("uppercase candidate")?
                        .reference()?,
                ]
            };
            let expected = selected
                .iter()
                .map(|r| r.as_stored().to_owned())
                .collect::<Vec<_>>();
            assert_eq!(expected.len(), if bulk && with_alias { 2 } else { 1 });
            assert_eq!(expected[0], uppercase);
            let s = committed(
                db.create_shelf_reviewed(plan, selected, WorkCancellation::default())?
                    .wait()?,
            )?;
            let members = db.shelf_members(s.id)?.wait()?;
            assert_eq!(
                members.iter().map(|m| m.uuid.clone()).collect::<Vec<_>>(),
                expected
            );
            assert!(
                members
                    .iter()
                    .all(|m| !m.manually_retained && m.origins.len() == 1)
            );
        }
    }
    db.close()?;
    Ok(())
}

#[test]
fn deleted_shelf_reviews_cannot_target_reused_ids_and_unrelated_plans_survive() -> TestResult {
    let roots = TestRoots::new()?;
    let lib = library(&roots)?;
    let mut db = Database::open_isolated(&lib)?;
    seed(&db, 1)?;
    let c = collection(&db, "All", Query::default())?;
    let other = create(&db, "Other")?;
    committed(
        db.attach_shelf_sources(other.id, vec![c], WorkCancellation::default())?
            .wait()?,
    )?;
    let unaffected = preview(&db, other.id)?;
    let s = create(&db, "Original")?;
    committed(
        db.attach_shelf_sources(s.id, vec![c], WorkCancellation::default())?
            .wait()?,
    )?;
    let old_update = preview(&db, s.id)?;
    let old_sources = db
        .shelf_source_change_preview(s.id, vec![], "fr-FR".into())?
        .wait()?;
    let old_delete = db
        .linked_collection_deletion_preview(c, "fr-FR".into())?
        .wait()?;
    let old_source_ids = db
        .shelf_source_evaluation(s.id, "fr-FR".into())?
        .wait()?
        .sources;

    edit(&db, Command::Delete { id: s.id })?;
    let replacement = create(&db, "Original")?;
    committed(
        db.attach_shelf_sources(replacement.id, vec![c], WorkCancellation::default())?
            .wait()?,
    )?;
    assert_eq!(s.id, replacement.id);
    assert_eq!(
        db.shelf_source_evaluation(replacement.id, "fr-FR".into())?
            .wait()?
            .sources,
        old_source_ids
    );
    let fresh = preview(&db, replacement.id)?;
    assert_eq!(fresh.preview().revision, old_update.preview().revision);
    assert_eq!(
        fresh.preview().additions_page(0),
        old_update.preview().additions_page(0)
    );

    assert!(matches!(
        apply(&db, old_update, vec![decision(1, shelves::Action::Add)]),
        Err(StorageError::Shelf(ShelfError::StaleReview))
    ));
    assert!(matches!(
        committed(
            db.change_shelf_sources(old_sources, true, vec![], WorkCancellation::default())?
                .wait()?
        ),
        Err(StorageError::Shelf(ShelfError::StaleReview))
    ));
    assert!(matches!(
        committed(
            db.delete_linked_collection(old_delete, true, vec![], WorkCancellation::default())?
                .wait()?
        ),
        Err(StorageError::Shelf(ShelfError::StaleReview))
    ));
    assert!(db.shelf_members(replacement.id)?.wait()?.is_empty());
    assert_eq!(
        db.shelf_source_evaluation(replacement.id, "fr-FR".into())?
            .wait()?
            .sources,
        old_source_ids
    );
    assert_eq!(db.collections()?.wait()?.len(), 1);
    apply(&db, unaffected, vec![decision(1, shelves::Action::Add)])?;
    let fresh = preview(&db, replacement.id)?;
    apply(&db, fresh, vec![decision(1, shelves::Action::Add)])?;
    assert_eq!(db.shelf_members(replacement.id)?.wait()?.len(), 1);
    db.close()?;
    Ok(())
}

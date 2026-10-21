#![cfg(unix)]
use librairii_core::{
    collection_codec::Query,
    collections::{Collection, CollectionError, Validity},
};
use librairii_storage::{
    IsolatedLibrary, Result, StorageError,
    collections::{Command, Response},
    database::{Database, TransactionOutcome, WorkCancellation},
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
        db.edit_collections(command, WorkCancellation::default())?
            .wait()?,
    )
}
fn created(db: &Database<'_>, name: &str, criteria: Query) -> Result<Collection> {
    match edit(
        db,
        Command::Create {
            name: name.into(),
            criteria,
        },
    )? {
        Response::Collection(c) => Ok(c),
        _ => Err(StorageError::CompatibilityInvariant),
    }
}
#[test]
fn collection_crud_preserves_ids_namespace_query_bytes_revisions_and_atomic_order() -> TestResult {
    let roots = TestRoots::new()?;
    let library = IsolatedLibrary::initialize_from_override(
        Some(roots.library().as_os_str()),
        "a0000000-0000-4000-8000-000000000001".parse()?,
    )?;
    let mut db = Database::open_isolated(&library)?;
    let all = created(&db, "  All\tStories ", Query::default())?;
    assert_eq!(all.name, b"All Stories");
    assert_eq!(all.query_payload, b"{}");
    let c = created(&db, "ÉTÉ", Query::default())?;
    assert!(matches!(
        created(&db, "ete", Query::default()),
        Err(StorageError::Collection(CollectionError::DuplicateName))
    ));
    let renamed = edit(
        &db,
        Command::Rename {
            id: c.id,
            name: "Histoires d'ÉTÉ".into(),
        },
    )?;
    if let Response::Collection(r) = renamed {
        assert_eq!(r.id, c.id);
        assert_eq!(r.revision, 2);
        assert_eq!(r.query_payload, c.query_payload);
    } else {
        return Err("rename".into());
    }
    let copy = edit(
        &db,
        Command::Duplicate {
            id: c.id,
            name: "Independent".into(),
        },
    )?;
    let Response::Collection(copy) = copy else {
        return Err("duplicate".into());
    };
    assert_ne!(copy.id, c.id);
    assert_eq!(copy.validity, Validity::Valid);
    edit(&db, Command::Reorder(vec![copy.id, c.id, all.id]))?;
    let before = db.collections()?.wait()?;
    assert!(edit(&db, Command::Reorder(vec![all.id, all.id, c.id])).is_err());
    assert_eq!(db.collections()?.wait()?, before);
    edit(&db, Command::Delete { id: copy.id })?;
    assert_eq!(db.collections()?.wait()?.len(), 2);
    committed(db.transaction(WorkCancellation::default(),|tx|{tx.execute("INSERT INTO collections(id,name,normalized_name,position,query_version,query_payload) VALUES(77,'Legacy','legacy',99,1,'{\"page\":9}')",[])?;tx.execute("INSERT INTO shelves(name,normalized_name,position) VALUES('ÉTÉ','ete',0)",[])?;Ok(())})?.wait()?)?;
    let Response::Inspection(i) = edit(&db, Command::Inspect { id: 77 })? else {
        return Err("inspect".into());
    };
    assert!(i.is_openable());
    assert_eq!(i.collection.id, 77);
    assert_eq!(i.collection.query_payload, b"{}");
    assert_eq!(i.collection.revision, 2);
    db.close()?;
    Ok(())
}

fn seed_stories(db: &Database<'_>, count: i64) -> TestResult {
    committed(db.transaction(WorkCancellation::default(),move|tx|{for id in 1..=count {
 tx.execute("INSERT INTO stories(id,uuid,display_name_normalized) VALUES(?1,printf('10000000-0000-4000-8000-%012d',?1),'story')",[id])?;
 tx.execute("INSERT INTO story_archives(story_id,original_filename,detected_format,sha256,byte_size,managed_path,validation_state) VALUES(?1,'story.zip','zip',printf('%064d',?1),1,'archives/'||?1,'valid')",[id])?;
 }Ok(())})?.wait()?)?;
    Ok(())
}
fn counts(db: &Database<'_>) -> Result<Vec<i64>> {
    Ok(committed(
        db.collection_summaries("fr-FR".into(), WorkCancellation::default())?
            .wait()?,
    )?
    .into_iter()
    .map(|s| s.count)
    .collect())
}
#[test]
fn live_counts_follow_library_tags_metadata_and_removal_without_shelf_mutation() -> TestResult {
    let roots = TestRoots::new()?;
    let library = IsolatedLibrary::initialize_from_override(
        Some(roots.library().as_os_str()),
        "a0000000-0000-4000-8000-000000000001".parse()?,
    )?;
    let mut db = Database::open_isolated(&library)?;
    seed_stories(&db, 2)?;
    committed(db.transaction(WorkCancellation::default(),|tx|{
        tx.execute_batch("INSERT INTO tag_definitions(id,key,normalized_key,label,kind,source,position,color) VALUES(44,'favorite','favorite','Favorite','boolean','user',0,'#405CF5'); INSERT INTO shelves(id,name,normalized_name,position) VALUES(1,'Approved','approved',0); INSERT INTO shelf_stories(shelf_id,story_uuid,position,cached_title) VALUES(1,'10000000-0000-4000-8000-000000000001',0,'Retained');")?;Ok(())
    })?.wait()?)?;
    let all = created(&db, "All", Query::default())?;
    let tag = created(
        &db,
        "Favorite",
        librairii_core::collection_codec::decode(
            5,
            br#"{"booleanFilters":[{"definitionId":44,"state":"true"}]}"#,
            0,
        )?,
    )?;
    created(&db, "Moon", {
        let mut q = Query::default();
        q.name = "moon".into();
        q
    })?;
    assert_eq!(counts(&db)?, vec![2, 0, 0]);
    seed_stories_extra(&db)?;
    assert_eq!(counts(&db)?, vec![3, 1, 1]);
    committed(db.transaction(WorkCancellation::default(),|tx|{
        tx.execute_batch("UPDATE stories SET is_broken=1; DELETE FROM story_tag_assignments; DELETE FROM stories WHERE id=2;")?;Ok(())
    })?.wait()?)?;
    assert_eq!(counts(&db)?, vec![2, 0, 1]);
    let librairii_storage::collections::Observation::Ready(e) = committed(
        db.evaluate_collection(
            all.id,
            librairii_core::query::QueryRequest {
                page_size: 1,
                ..Default::default()
            },
            "fr-FR".into(),
            WorkCancellation::default(),
        )?
        .wait()?,
    )?
    else {
        return Err("evaluation".into());
    };
    assert_eq!(e.page.total_items, 2);
    assert_eq!(e.page.stories.len(), 1);
    committed(
        db.transaction(WorkCancellation::default(), move |tx| {
            tx.execute("DELETE FROM tag_definitions WHERE id=44", [])?;
            let before = librairii_storage::collections::read(tx, tag.id)?;
            assert!(librairii_storage::collections::source(tx, tag.id).is_err());
            assert_eq!(librairii_storage::collections::read(tx, tag.id)?, before);
            Ok(())
        })?
        .wait()?,
    )?;
    let v = committed(
        db.evaluate_collection(
            tag.id,
            Default::default(),
            "fr-FR".into(),
            WorkCancellation::default(),
        )?
        .wait()?,
    )?;
    assert!(matches!(
        v,
        librairii_storage::collections::Observation::NeedsAttention(_)
    ));
    assert_eq!(
        db.collections()?.wait()?[1].validity,
        Validity::NeedsAttention
    );
    let shelf = db
        .read(|tx| {
            Ok(tx.query_row(
                "SELECT cached_title FROM shelf_stories WHERE shelf_id=1",
                [],
                |r| r.get::<_, String>(0),
            )?)
        })?
        .wait()?;
    assert_eq!(shelf, "Retained");
    db.close()?;
    Ok(())
}
fn seed_stories_extra(db: &Database<'_>) -> TestResult {
    committed(db.transaction(WorkCancellation::default(),|tx|{
        tx.execute_batch("INSERT INTO stories(id,uuid,display_name_normalized) VALUES(3,'10000000-0000-4000-8000-000000000003','story'); INSERT INTO story_archives(story_id,original_filename,detected_format,sha256,byte_size,managed_path,validation_state) VALUES(3,'story.zip','zip',printf('%064d',3),1,'archives/3','valid'); INSERT INTO story_tag_assignments(story_id,definition_id,source) VALUES(3,44,'manual'); INSERT INTO catalog_syncs(id,locale,status,started_at) VALUES('10000000-0000-4000-8000-000000000001','fr-FR','running','2000'); INSERT INTO catalog_snapshots(id,sync_id,locale,raw_path,raw_sha256,byte_size,record_count,fetched_at) VALUES(1,'10000000-0000-4000-8000-000000000001','fr-FR','catalog/one',printf('%064d',1),1,1,'2000'); INSERT INTO official_story_metadata(snapshot_id,story_uuid,locale,title,title_normalized,provenance) VALUES(1,'10000000-0000-4000-8000-000000000003','fr-FR','Moon','moon','lunii_catalog'); UPDATE catalog_snapshots SET status='active',activated_at='2001' WHERE id=1;")?;Ok(())
    })?.wait()?)?;
    Ok(())
}

#[test]
fn union_preview_counts_overlap_all_pages_and_saves_independent_definition() -> TestResult {
    use librairii_storage::collections::Observation;
    let roots = TestRoots::new()?;
    let library = IsolatedLibrary::initialize_from_override(
        Some(roots.library().as_os_str()),
        "a0000000-0000-4000-8000-000000000001".parse()?,
    )?;
    let mut db = Database::open_isolated(&library)?;
    seed_stories(&db, 251)?;
    let all = created(&db, "All", Query::default())?;
    let mut q = Query::default();
    q.name = "story".into();
    let stories = created(&db, "Stories", q)?;
    let Observation::Ready(p) = committed(
        db.collection_selection_preview(
            vec![stories.id, all.id],
            "fr-FR".into(),
            WorkCancellation::default(),
        )?
        .wait()?,
    )?
    else {
        return Err("preview".into());
    };
    assert_eq!(p.unique_count, 251);
    assert_eq!(p.overlap_count, 251);
    assert_eq!(
        p.sources
            .iter()
            .map(|s| (s.collection.id, s.count))
            .collect::<Vec<_>>(),
        vec![(stories.id, 251), (all.id, 251)]
    );
    assert!(
        committed(
            db.collection_selection_preview(
                vec![all.id, all.id],
                "fr-FR".into(),
                WorkCancellation::default()
            )?
            .wait()?
        )
        .is_err()
    );
    let Observation::Ready(copy) = committed(
        db.save_combined_collection(
            vec![stories.id, all.id],
            "Copied OR".into(),
            WorkCancellation::default(),
        )?
        .wait()?,
    )?
    else {
        return Err("save".into());
    };
    assert_eq!(copy.query_payload, br#"{"anyOf":[{"name":"story"},{}]}"#);
    edit(&db, Command::Delete { id: all.id })?;
    edit(&db, Command::Delete { id: stories.id })?;
    let Observation::Ready(e) = committed(
        db.evaluate_collection(
            copy.id,
            librairii_core::query::QueryRequest {
                page: 3,
                page_size: 100,
                ..Default::default()
            },
            "fr-FR".into(),
            WorkCancellation::default(),
        )?
        .wait()?,
    )?
    else {
        return Err("evaluation".into());
    };
    assert_eq!(e.page.total_items, 251);
    assert_eq!(e.page.stories.len(), 51);
    seed_stories_extra_unfiltered(&db)?;
    assert_eq!(counts(&db)?, vec![252]);
    db.close()?;
    Ok(())
}
fn seed_stories_extra_unfiltered(db: &Database<'_>) -> TestResult {
    committed(db.transaction(WorkCancellation::default(),|tx|{
        tx.execute_batch("INSERT INTO stories(id,uuid,display_name_normalized) VALUES(999,'10000000-0000-4000-8000-000000000999','other'); INSERT INTO story_archives(story_id,original_filename,detected_format,sha256,byte_size,managed_path,validation_state) VALUES(999,'new.zip','zip',printf('%064d',999),1,'archives/999','valid');")?;Ok(())
    })?.wait()?)?;
    Ok(())
}

#[test]
fn explicit_repair_retains_invalid_bytes_rejects_stale_plans_and_rolls_back_writes() -> TestResult {
    let roots = TestRoots::new()?;
    let library = IsolatedLibrary::initialize_from_override(
        Some(roots.library().as_os_str()),
        "a0000000-0000-4000-8000-000000000001".parse()?,
    )?;
    let mut db = Database::open_isolated(&library)?;
    committed(db.transaction(WorkCancellation::default(),|tx|{
        tx.execute("INSERT INTO collections(id,name,normalized_name,position,query_version,query_payload) VALUES(88,'Legacy','legacy',0,99,CAST(X'7B226E616D65223A22FF227D' AS TEXT))",[])?;Ok(())
    })?.wait()?)?;
    let Response::Inspection(i) = edit(&db, Command::Inspect { id: 88 })? else {
        return Err("inspect".into());
    };
    assert_eq!(
        i.collection.query_payload,
        [br#"{"name":""#.as_slice(), &[255], br#""}"#.as_slice()].concat()
    );
    assert_eq!(i.collection.validity, Validity::NeedsAttention);
    let plan = db
        .collection_replacement_plan(88, Query::default())?
        .wait()?;
    edit(
        &db,
        Command::Rename {
            id: 88,
            name: "Renamed".into(),
        },
    )?;
    assert!(matches!(
        committed(
            db.replace_collection(plan, WorkCancellation::default())?
                .wait()?
        ),
        Err(StorageError::Collection(CollectionError::StalePreview))
    ));
    let before = db.collections()?.wait()?;
    let cancelled = WorkCancellation::default();
    let in_writer = cancelled.clone();
    let outcome = db
        .transaction(cancelled, move |tx| {
            librairii_storage::collections::execute(
                tx,
                Command::Rename {
                    id: 88,
                    name: "Must roll back".into(),
                },
            )?;
            in_writer.cancel();
            Ok(())
        })?
        .wait()?;
    assert!(matches!(
        outcome,
        TransactionOutcome::ProvenNotCommitted(StorageError::Cancelled)
    ));
    assert_eq!(db.collections()?.wait()?, before);
    let plan = db
        .collection_replacement_plan(88, Query::default())?
        .wait()?;
    let repaired = committed(
        db.replace_collection(plan, WorkCancellation::default())?
            .wait()?,
    )?;
    assert_eq!(repaired.id, 88);
    assert_eq!(repaired.name, b"Renamed");
    assert_eq!(repaired.query_payload, b"{}");
    assert_eq!(repaired.validity, Validity::Valid);
    assert_eq!(repaired.revision, before[0].revision + 1);
    let empty = db
        .collection_replacement_plan(88, Query::default())?
        .wait()?;
    assert_eq!(
        committed(
            db.replace_collection(empty, WorkCancellation::default())?
                .wait()?
        )?
        .revision,
        repaired.revision
    );
    db.close()?;
    Ok(())
}

#[test]
fn deletion_reports_links_preserves_approved_history_and_transitions_only_committed_active_view()
-> TestResult {
    use librairii_core::{
        CollectionId,
        collection_codec::url::{Location, Scope},
        navigation::{Navigation, Presentation},
        query::QueryRequest,
    };
    let roots = TestRoots::new()?;
    let library = IsolatedLibrary::initialize_from_override(
        Some(roots.library().as_os_str()),
        "a0000000-0000-4000-8000-000000000001".parse()?,
    )?;
    let mut db = Database::open_isolated(&library)?;
    let c = created(&db, "Linked", Query::default())?;
    let mut request = QueryRequest {
        page: 4,
        page_size: 10,
        sort: "imported_desc".into(),
        ..Default::default()
    };
    request.criteria.name = "current refinement".into();
    let mut nav = Navigation::new(
        library.identity().typed(),
        1,
        Location {
            scope: Scope::Collection(CollectionId::new(c.id)?),
            request,
        },
    )?;
    nav.select(vec![9, 3])?;
    nav.set_presentation(Presentation::LargeCovers);
    let stamp = nav.stamp();
    let frame = nav.current().clone();
    let stale = db.collection_deletion_plan(c.id)?.wait()?;
    committed(db.transaction(WorkCancellation::default(),move|tx|{
        tx.execute_batch("INSERT INTO shelves(id,name,normalized_name,position) VALUES(1,'Approved','approved',0); INSERT INTO shelf_stories(shelf_id,story_uuid,position,cached_title) VALUES(1,'10000000-0000-4000-8000-000000000001',0,'Unavailable'); INSERT INTO shelf_exclusions(shelf_id,story_uuid) VALUES(1,'10000000-0000-4000-8000-000000000002');")?;
        tx.execute("INSERT INTO shelf_sources(id,shelf_id,collection_id,collection_name) VALUES(1,1,?,'Linked')",[c.id])?;
        tx.execute_batch("INSERT INTO shelf_story_origins(shelf_id,story_uuid,source_id) VALUES(1,'10000000-0000-4000-8000-000000000001',1);")?;Ok(())
    })?.wait()?)?;
    assert!(matches!(
        committed(
            db.delete_collection(stale, WorkCancellation::default())?
                .wait()?
        ),
        Err(StorageError::Collection(CollectionError::StalePreview))
    ));
    let plan = db.collection_deletion_plan(c.id)?.wait()?;
    let source = &plan.impact().sources[0];
    assert!(source.active);
    assert_eq!(source.approved_members, 1);
    assert_eq!(source.accepted_origins, 1);
    assert_eq!(source.exclusions, 1);
    assert!(matches!(
        committed(
            db.delete_collection(plan, WorkCancellation::default())?
                .wait()?
        ),
        Err(StorageError::Collection(CollectionError::InUse))
    ));
    assert_eq!(nav.current(), &frame);
    committed(
        db.transaction(WorkCancellation::default(), |tx| {
            tx.execute("UPDATE shelf_sources SET active=0 WHERE id=1", [])?;
            Ok(())
        })?
        .wait()?,
    )?;
    let plan = db.collection_deletion_plan(c.id)?.wait()?;
    let receipt = committed(
        db.delete_collection(plan, WorkCancellation::default())?
            .wait()?,
    )?;
    assert!(receipt.apply_navigation(&mut nav, &stamp)?);
    assert_eq!(nav.current().location.scope, Scope::Custom);
    assert_eq!(nav.current().location.request, frame.location.request);
    assert_eq!(nav.current().selection, vec![9, 3]);
    assert_eq!(nav.presentation(), Presentation::LargeCovers);
    let kept = db
        .read(|tx| {
            Ok((
                tx.query_row(
                    "SELECT collection_id FROM shelf_sources WHERE id=1",
                    [],
                    |r| r.get::<_, Option<i64>>(0),
                )?,
                tx.query_row("SELECT COUNT(*) FROM shelf_story_origins", [], |r| {
                    r.get::<_, i64>(0)
                })?,
                tx.query_row("SELECT COUNT(*) FROM shelf_stories", [], |r| {
                    r.get::<_, i64>(0)
                })?,
                tx.query_row("SELECT COUNT(*) FROM shelf_exclusions", [], |r| {
                    r.get::<_, i64>(0)
                })?,
            ))
        })?
        .wait()?;
    assert_eq!(kept, (None, 1, 1, 1));
    db.close()?;
    Ok(())
}

#![cfg(unix)]
use librairii_core::{
    CollectionId, ShelfId, collection_codec::Query, export::Scope, query::QueryRequest,
};
use librairii_storage::{
    IsolatedLibrary,
    database::{Database, TransactionOutcome, WorkCancellation},
};
use librairii_testkit::TestRoots;
use std::error::Error;
type TestResult<T = ()> = Result<T, Box<dyn Error>>;
fn committed<T>(v: TransactionOutcome<T>) -> TestResult<T> {
    match v {
        TransactionOutcome::Committed(v) => Ok(v),
        TransactionOutcome::ProvenNotCommitted(e) | TransactionOutcome::CommitUncertain(e) => {
            Err(e.into())
        }
    }
}
fn seed(db: &Database<'_>, count: i64) -> TestResult {
    committed(db.transaction(WorkCancellation::default(),move|tx|{
        for id in 1..=count {
            tx.execute("INSERT INTO stories(id,uuid,embedded_title,display_name_normalized,is_broken) VALUES(?1,printf('10000000-0000-4000-8000-%012d',?1),'Story'||?1,printf('%08d',?1),?1=2)",[id])?;
            tx.execute("INSERT INTO story_archives(story_id,original_filename,detected_format,sha256,byte_size,managed_path,validation_state) VALUES(?1,?1||'.zip','zip',printf('%064d',?1),1,'archives/'||?1,'valid')",[id])?;
        }
        tx.execute_batch("INSERT INTO shelves(id,name,normalized_name,position) VALUES(1,'Approved','approved',0),(2,'Second','second',1);
            INSERT INTO shelf_stories(shelf_id,story_uuid,position,protected,cached_title) VALUES
            (1,'10000000-0000-4000-8000-000000000002',0,0,'Two'),
            (1,'10000000-0000-4000-8000-000000009999',1,0,'Unavailable'),
            (1,'10000000-0000-4000-8000-000000000001',2,0,'One'),
            (2,'10000000-0000-4000-8000-000000000001',0,0,'One');")?;
        Ok(())
    })?.wait()?)?;
    Ok(())
}
#[test]
fn full_query_selection_collection_and_approved_shelf_unions_keep_identity_order_and_overlap()
-> TestResult {
    let roots = TestRoots::new()?;
    let library = IsolatedLibrary::initialize_from_override(
        Some(roots.library().as_os_str()),
        "a0000000-0000-4000-8000-000000000001".parse()?,
    )?;
    let mut db = Database::open_isolated(&library)?;
    seed(&db, 1001)?;
    let all = db
        .resolve_export(
            Scope::CurrentQuery(QueryRequest {
                page: 99,
                page_size: 1,
                ..Default::default()
            }),
            "fr-FR".into(),
        )?
        .wait()?;
    assert_eq!(all.stories.len(), 1001);
    let selection = db
        .resolve_export(Scope::Selection(vec![2, 1, 2]), "fr-FR".into())?
        .wait()?;
    assert_eq!(
        selection.stories.iter().map(|s| s.id).collect::<Vec<_>>(),
        [2, 1]
    );
    assert!(selection.stories[0].broken);
    let shelves = db
        .resolve_export(
            Scope::Shelves(vec![ShelfId::new(1)?, ShelfId::new(2)?]),
            "fr-FR".into(),
        )?
        .wait()?;
    assert_eq!(
        shelves.stories.iter().map(|s| s.id).collect::<Vec<_>>(),
        [2, 0, 1]
    );
    assert_eq!(shelves.stories[1].title, "Unavailable");
    assert_eq!(shelves.collapsed_overlap, 1);
    let selected = db
        .resolve_export(
            Scope::ShelfSelection {
                shelf: ShelfId::new(1)?,
                uuids: vec![shelves.stories[1].uuid.clone()],
            },
            "fr-FR".into(),
        )?
        .wait()?;
    assert_eq!(selected.stories[0].id, 0);
    assert!(
        db.resolve_export(
            Scope::ShelfSelection {
                shelf: ShelfId::new(1)?,
                uuids: vec!["unknown".into()]
            },
            "fr-FR".into()
        )?
        .wait()
        .is_err()
    );
    let mut filter = Query::default();
    filter.name = "00000001".into();
    let refined = db
        .resolve_export(
            Scope::ShelfResult {
                shelf: ShelfId::new(1)?,
                query: QueryRequest {
                    criteria: filter,
                    ..Default::default()
                },
            },
            "fr-FR".into(),
        )?
        .wait()?;
    assert_eq!(
        refined.stories.iter().map(|s| s.id).collect::<Vec<_>>(),
        [1]
    );
    for name in ["First", "Second"] {
        committed(
            db.edit_collections(
                librairii_storage::collections::Command::Create {
                    name: name.into(),
                    criteria: Query::default(),
                },
                WorkCancellation::default(),
            )?
            .wait()?,
        )?;
    }
    let union = db
        .resolve_export(
            Scope::Collections(vec![CollectionId::new(1)?, CollectionId::new(2)?]),
            "fr-FR".into(),
        )?
        .wait()?;
    assert_eq!(union.stories.len(), 1001);
    assert_eq!(union.collapsed_overlap, 1001);
    assert!(
        db.resolve_export(
            Scope::Collections(vec![CollectionId::new(1)?, CollectionId::new(1)?]),
            "fr-FR".into()
        )?
        .wait()
        .is_err()
    );
    db.close()?;
    Ok(())
}

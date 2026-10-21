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

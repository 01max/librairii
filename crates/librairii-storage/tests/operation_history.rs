#![cfg(any(target_os = "macos", target_os = "linux"))]
use librairii_storage::{
    IsolatedLibrary,
    database::{Database, TransactionOutcome},
};
use librairii_testkit::TestRoots;
use std::{error::Error, fs, thread};
type TestResult<T = ()> = Result<T, Box<dyn Error>>;
fn library(roots: &TestRoots) -> TestResult<IsolatedLibrary> {
    Ok(IsolatedLibrary::initialize_from_override(
        Some(roots.library().as_os_str()),
        "b0000000-0000-4000-8000-000000000001".parse()?,
    )?)
}
fn committed<T>(result: TransactionOutcome<T>) -> TestResult<T> {
    match result {
        TransactionOutcome::Committed(v) => Ok(v),
        TransactionOutcome::ProvenNotCommitted(e) | TransactionOutcome::CommitUncertain(e) => {
            Err(e.into())
        }
    }
}
fn id(n: usize) -> String {
    format!("Z{n:035}")
}
fn seed(tx: &rusqlite::Transaction<'_>, n: usize) -> librairii_storage::Result<()> {
    tx.execute("INSERT INTO file_operations(id,kind,status,total_items,created_at) VALUES(?,'import','queued',1,'raw legacy time')",[id(n)])?;
    tx.execute("INSERT INTO file_operation_items(operation_id,source_name,status,outcome_code,outcome_message) VALUES(?,'source.zip','pending','future.code','Unchanged historical text')",[id(n)])?;
    Ok(())
}
#[test]
fn history_twenty_raw_key_pages_remain_stable_with_concurrent_new_inserts() -> TestResult {
    let roots = TestRoots::new()?;
    let lib = library(&roots)?;
    let mut db = Database::open_isolated(&lib)?;
    committed(
        db.transaction(Default::default(), |tx| {
            for n in 1..=45 {
                seed(tx, n)?;
            }
            Ok(())
        })?
        .wait()?,
    )?;
    let first = db.operation_history(String::new())?.wait()?;
    assert_eq!(first.operations.len(), 20);
    assert_eq!(first.operations[0].operation.identity.id, id(45));
    assert_eq!(first.next_before, Some(id(26)));
    let second = thread::scope(|scope| -> TestResult<_> {
        let writer = scope.spawn(|| -> librairii_storage::Result<()> {
            match db
                .transaction(Default::default(), |tx| {
                    seed(tx, 99)?;
                    seed(tx, 0)?;
                    Ok(())
                })?
                .wait()?
            {
                TransactionOutcome::Committed(()) => Ok(()),
                TransactionOutcome::ProvenNotCommitted(e)
                | TransactionOutcome::CommitUncertain(e) => Err(e),
            }
        });
        let second = db
            .operation_history(first.next_before.clone().ok_or("cursor")?)?
            .wait()?;
        writer.join().map_err(|_| "insert worker")??;
        Ok(second)
    })?;
    assert_eq!(
        second
            .operations
            .iter()
            .map(|s| s.operation.identity.id.clone())
            .collect::<Vec<_>>(),
        (6..=25).rev().map(id).collect::<Vec<_>>()
    );
    assert_eq!(second.next_before, Some(id(6)));
    let third = db.operation_history(id(6))?.wait()?;
    assert_eq!(third.operations.len(), 6);
    assert!(third.next_before.is_none());
    assert_eq!(third.operations[5].operation.identity.id, id(0));
    let json = second.presentation()?;
    assert_eq!(
        json["operations"][0]["items"][0]["outcomeCode"],
        "future.code"
    );
    assert_eq!(json["nextBefore"], id(6));
    assert!(db.operation_history("missing".into())?.wait().is_err());
    let empty = db.operation_history(id(0))?.wait()?.presentation()?;
    assert_eq!(empty["operations"], serde_json::json!([]));
    assert!(empty.get("nextBefore").is_none());
    db.close()?;
    Ok(())
}
#[test]
fn active_latest_preserves_null_ordering_and_reveal_requires_retained_valid_directory() -> TestResult
{
    let roots = TestRoots::new()?;
    let lib = library(&roots)?;
    let mut db = Database::open_isolated(&lib)?;
    let destination = roots.profile().join("-exports; literal argument");
    fs::create_dir(&destination)?;
    let raw = destination.to_str().ok_or("destination")?.to_owned();
    committed(db.transaction(Default::default(),move|tx|{
        for n in 1..=30 {seed(tx,n)?;}
        for (n,created,finished) in [(101,"a",Some("x")),(102,"zz",Some("")),(103,"z",None)] {
            tx.execute("INSERT INTO file_operations(id,kind,status,created_at,finished_at,export_source_type,export_source_shelf_ids,export_source_shelf_names,export_destination) VALUES(?,'export','failed',?,?,'shelf','[7]','[\"Old dynamic\"]',?)",rusqlite::params![id(n),created,finished,raw])?;
        }Ok(())
    })?.wait()?)?;
    let active = db.active_and_latest_operations()?.wait()?;
    assert_eq!(active.len(), 31);
    assert_eq!(active[0].operation.identity.id, id(30));
    assert_eq!(active[29].operation.identity.id, id(1));
    assert_eq!(active[30].operation.identity.id, id(103));
    let json = active[30].presentation()?;
    assert_eq!(json["sourceCollectionIds"], serde_json::json!([7]));
    assert_eq!(json["sourceShelfIds"], serde_json::json!([7]));
    assert!(db.export_destination(id(1)).is_err());
    {
        let reveal = db.export_destination(id(103))?;
        assert_eq!(reveal.path_for_reveal()?, destination);
        fs::rename(&destination, roots.profile().join("moved-original"))?;
        fs::create_dir(&destination)?;
        assert!(reveal.path_for_reveal().is_err());
    }
    let alias = roots.profile().join("alias");
    std::os::unix::fs::symlink(&destination, &alias)?;
    let file = roots.profile().join("regular");
    fs::write(&file, b"file")?;
    for (n, path) in [
        "relative".to_owned(),
        " ".into(),
        "/bad\0path".into(),
        alias.to_str().ok_or("alias")?.into(),
        file.to_str().ok_or("file")?.into(),
        roots
            .profile()
            .join("missing")
            .to_str()
            .ok_or("missing")?
            .into(),
    ]
    .into_iter()
    .enumerate()
    {
        let result=db.transaction(Default::default(),move|tx|{tx.execute("INSERT INTO file_operations(id,kind,status,created_at,export_source_type,export_source_shelf_ids,export_source_shelf_names,export_destination) VALUES(?,'export','interrupted','x','selection','[]','[]',?)",rusqlite::params![id(200+n),path])?;Ok(())})?.wait()?;
        if n == 1 {
            assert!(matches!(result, TransactionOutcome::ProvenNotCommitted(_)));
        } else {
            committed(result)?;
        }
        assert!(db.export_destination(id(200 + n)).is_err());
    }
    db.close()?;
    Ok(())
}

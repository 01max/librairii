#![cfg(unix)]
use librairii_core::{collection_codec::url, query::QueryRequest};
use librairii_storage::{
    IsolatedLibrary, StorageError,
    database::{Database, TransactionOutcome, WorkCancellation},
    query,
};
use librairii_testkit::TestRoots;
use rusqlite::{Transaction, params, params_from_iter, types::Value as SqlValue};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::error::Error;
type TestResult<T = ()> = Result<T, Box<dyn Error>>;

fn fixture(bytes: &[u8], sha: &str) -> TestResult<Value> {
    assert_eq!(
        librairii_core::Checksum::from_bytes(Sha256::digest(bytes).into()).to_string(),
        sha
    );
    Ok(serde_json::from_slice(bytes)?)
}
fn sql(value: &Value) -> librairii_storage::Result<SqlValue> {
    match value {
        Value::Null => Ok(SqlValue::Null),
        Value::String(s) => Ok(SqlValue::Text(s.clone())),
        Value::Number(n) => n
            .as_i64()
            .map(SqlValue::Integer)
            .ok_or(StorageError::InvalidNavigation),
        _ => Err(StorageError::InvalidNavigation),
    }
}
fn seed_table(
    tx: &Transaction<'_>,
    snapshot: &Value,
    table: &str,
) -> librairii_storage::Result<()> {
    let data = &snapshot[table];
    let columns = data["columns"]
        .as_array()
        .ok_or(StorageError::InvalidNavigation)?;
    let names = columns
        .iter()
        .map(|v| v.as_str().ok_or(StorageError::InvalidNavigation))
        .collect::<librairii_storage::Result<Vec<_>>>()?;
    let mut info = tx.prepare(&format!("PRAGMA table_info(\"{table}\")"))?;
    let actual = info
        .query_map([], |r| r.get::<_, String>(1))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if actual != names {
        return Err(StorageError::MigrationConflict);
    }
    let Some(rows) = data["rows"].as_array() else {
        return Ok(());
    };
    let fields = names
        .iter()
        .map(|v| format!("\"{}\"", v.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(",");
    let marks = vec!["?"; names.len()].join(",");
    for row in rows {
        let mut row = row.clone();
        if table == "catalog_syncs" {
            row["status"] = "running".into();
            row["finished_at"] = Value::Null;
            row["error_code"] = Value::Null;
            row["error_message"] = Value::Null;
        }
        if table == "catalog_snapshots" {
            row["status"] = "staged".into();
            row["activated_at"] = Value::Null;
        }
        let values = names
            .iter()
            .map(|name| sql(&row[*name]))
            .collect::<librairii_storage::Result<Vec<_>>>()?;
        tx.execute(
            &format!("INSERT INTO \"{table}\"({fields}) VALUES({marks})"),
            params_from_iter(values),
        )?;
    }
    Ok(())
}
fn restore(db: &Database<'_>, snapshot: Value) -> TestResult {
    let outcome = db
        .transaction(WorkCancellation::default(), move |tx| {
            for table in [
                "stories",
                "story_archives",
                "tag_definitions",
                "tag_values",
                "story_tag_assignments",
                "catalog_syncs",
                "catalog_snapshots",
                "official_story_metadata",
            ] {
                seed_table(tx, &snapshot, table)?;
            }
            for row in snapshot["catalog_snapshots"]["rows"]
                .as_array()
                .ok_or(StorageError::InvalidNavigation)?
            {
                tx.execute(
                    "UPDATE catalog_snapshots SET status='active',activated_at=? WHERE id=?",
                    params![row["activated_at"].as_str(), row["id"].as_i64()],
                )?;
                if row["status"] == "superseded" {
                    tx.execute(
                        "UPDATE catalog_snapshots SET status='superseded' WHERE id=?",
                        [row["id"].as_i64()],
                    )?;
                }
            }
        for row in snapshot["catalog_syncs"]["rows"].as_array().ok_or(StorageError::InvalidNavigation)? {
            tx.execute("UPDATE catalog_syncs SET status=?,finished_at=?,error_code=?,error_message=? WHERE id=?",params![row["status"].as_str(),row["finished_at"].as_str(),row["error_code"].as_str(),row["error_message"].as_str(),row["id"].as_str()])?;
        }
            Ok(())
        })?
        .wait()?;
    match outcome {
        TransactionOutcome::Committed(()) => Ok(()),
        TransactionOutcome::ProvenNotCommitted(e) | TransactionOutcome::CommitUncertain(e) => {
            Err(e.into())
        }
    }
}

#[test]
fn all_twenty_four_frozen_count_page_and_full_scope_returns_match_without_normalization()
-> TestResult {
    let reference = fixture(
        include_bytes!("fixtures/query/candidate.json"),
        "c17f99dc76a72277f7c1a66391476ef185e28b7a0bc875f128f8f73f8fe2f342",
    )?;
    let a = fixture(
        include_bytes!("fixtures/query/seed-context-A.json"),
        "8bec91accfde93a44318cfa4495424a3affff53e7827d4aaefe19a4cb69c537c",
    )?;
    let b = fixture(
        include_bytes!("fixtures/query/seed-context-B.json"),
        "669f2dcd4afd211da8c0f36d8d9f62e71f3a2b3b201633abed8851e12e08ffdf",
    )?;
    let cases = reference["cases"].as_array().ok_or("missing cases")?;
    assert_eq!(cases.len(), 24);
    for (context, snapshot) in [("A", a), ("B", b)] {
        let roots = TestRoots::new()?;
        let library = IsolatedLibrary::initialize_from_override(
            Some(roots.library().as_os_str()),
            "a0000000-0000-4000-8000-000000000001".parse()?,
        )?;
        let mut db = Database::open_isolated(&library)?;
        restore(&db, snapshot)?;
        for case in cases
            .iter()
            .filter(|case| case["case"]["context"] == context)
        {
            let name = case["case"]["id"].as_str().ok_or("missing case id")?;
            let request = url::restore_request(
                case["case"]["raw_request_json"]
                    .as_str()
                    .ok_or("request")?
                    .as_bytes(),
            )?;
            let locale = case["case"]["official_locale"]
                .as_str()
                .ok_or("locale")?
                .to_owned();
            let count = db.count(request.clone(), locale.clone())?.wait();
            let page = db.search(request.clone(), locale.clone())?.wait();
            let scope = db
                .read(move |tx| {
                    let mut items = Vec::new();
                    query::visit_scope(tx, request, &locale, |item| {
                        items.push(item);
                        Ok(())
                    })?;
                    Ok(items)
                })?
                .wait();
            if case["count"]["error"].is_null() {
                assert_eq!(
                    count?,
                    case["count"]["actual_return_value"]
                        .as_i64()
                        .ok_or("count")?,
                    "{name}"
                );
                assert_eq!(
                    serde_json::to_value(page?)?,
                    case["search"]["actual_return_value"],
                    "{name}"
                );
                assert_eq!(
                    serde_json::to_value(scope?)?,
                    case["full_scope"]["actual_return_value"],
                    "{name}"
                );
            } else {
                assert_eq!(name, "q24");
                for result in [count.map(|_| ()), page.map(|_| ()), scope.map(|_| ())] {
                    assert!(
                        matches!(result, Err(StorageError::QueryInput(_))),
                        "{name}: {result:?}"
                    );
                }
            }
        }
        db.close()?;
    }
    Ok(())
}

#[test]
fn streaming_scope_exceeds_picker_and_page_limits_and_stops_on_callback_failure() -> TestResult {
    let roots = TestRoots::new()?;
    let library = IsolatedLibrary::initialize_from_override(
        Some(roots.library().as_os_str()),
        "a0000000-0000-4000-8000-000000000001".parse()?,
    )?;
    let mut db = Database::open_isolated(&library)?;
    let result=db.transaction(WorkCancellation::default(),|tx| {
        for id in 1..=1001 {
            tx.execute("INSERT INTO stories(id,uuid,embedded_title,display_name_normalized) VALUES(?1,printf('10000000-0000-4000-8000-%012d',?1),'Story','story')",[id])?;
            tx.execute("INSERT INTO story_archives(story_id,original_filename,detected_format,sha256,byte_size,managed_path) VALUES(?1,'story.zip','zip',printf('%064d',?1),1,'archives/'||?1)",[id])?;
        }Ok(())
    })?.wait()?;
    assert!(matches!(result, TransactionOutcome::Committed(())));
    db.read(|tx| {
        let request = QueryRequest {
            page: 900,
            page_size: 1,
            sort: "imported_desc".into(),
            ..QueryRequest::default()
        };
        let mut previous = 1002;
        let count = query::visit_scope(tx, request.clone(), "en-GB", |row| {
            assert!(row.id < previous);
            previous = row.id;
            Ok(())
        })?;
        assert_eq!(count, 1001);
        let mut seen = 0;
        let result = query::visit_scope(tx, request, "en-GB", |_| {
            seen += 1;
            Err(StorageError::Cancelled)
        });
        assert!(matches!(result, Err(StorageError::Cancelled)));
        assert_eq!(seen, 1);
        Ok(())
    })?
    .wait()?;
    db.close()?;
    Ok(())
}

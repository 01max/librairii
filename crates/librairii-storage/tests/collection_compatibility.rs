use librairii_storage::{
    IsolatedLibrary, StorageError,
    database::{Database, TransactionOutcome, WorkCancellation},
    migrations::{CollectionAttention, CollectionValidity, CompatibilityInspection},
};
use librairii_testkit::TestRoots;
use rusqlite::{Connection, types::ValueRef};
use serde_json::{Value, json};
#[cfg(unix)]
use sha2::{Digest, Sha256};
use std::error::Error;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;
fn isolated(roots: &TestRoots) -> TestResult<IsolatedLibrary> {
    Ok(IsolatedLibrary::initialize_from_override(
        Some(roots.library().as_os_str()),
        "A0000000-0000-4000-8000-000000000001".parse()?,
    )?)
}
fn committed<T>(value: TransactionOutcome<T>) -> Result<T, StorageError> {
    match value {
        TransactionOutcome::Committed(value) => Ok(value),
        TransactionOutcome::ProvenNotCommitted(error)
        | TransactionOutcome::CommitUncertain(error) => Err(error),
    }
}
fn seed(db: &Database<'_>, id: i64, version: i64, payload: &str, validity: &str) -> TestResult {
    let payload = payload.to_owned();
    let validity = validity.to_owned();
    committed(db.transaction(WorkCancellation::default(), move |tx| {
        tx.execute("INSERT INTO collections(id,name,normalized_name,position,query_version,query_payload,validity_state,created_at,updated_at) VALUES(?1,'Collection '||?1,'collection '||?1,?1,?2,?3,?4,'2000-01-01 00:00:00','2000-01-01 00:00:00')", rusqlite::params![id,version,payload,validity])?;
        Ok(())
    })?.wait()?)?;
    Ok(())
}
fn inspection(db: &Database<'_>, id: i64) -> TestResult<CompatibilityInspection> {
    Ok(committed(
        db.inspect_collection_compatibility(id, WorkCancellation::default())?
            .wait()?,
    )?)
}
fn rows(connection: &Connection, sql: &str) -> librairii_storage::Result<Vec<Value>> {
    let mut statement = connection.prepare(sql)?;
    let columns = statement
        .column_names()
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let mut cursor = statement.query([])?;
    let mut result = Vec::new();
    while let Some(row) = cursor.next()? {
        let mut object = serde_json::Map::new();
        for (index, name) in columns.iter().enumerate() {
            let value = match row.get_ref(index)? {
                ValueRef::Null => Value::Null,
                ValueRef::Integer(n) => json!(n),
                ValueRef::Real(n) => json!(n),
                ValueRef::Text(b) | ValueRef::Blob(b) => Value::String(
                    std::str::from_utf8(b)
                        .map_err(|e| StorageError::Io {
                            operation: "read synthetic snapshot UTF8",
                            source: std::io::Error::other(e),
                        })?
                        .to_owned(),
                ),
            };
            object.insert(name.clone(), value);
        }
        result.push(Value::Object(object));
    }
    Ok(result)
}
fn snapshot(connection: &Connection) -> librairii_storage::Result<Value> {
    let mut tables = serde_json::Map::new();
    for table in rows(
        connection,
        "SELECT name FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
    )? {
        let name = table["name"]
            .as_str()
            .ok_or(StorageError::IntegrityFailed)?;
        tables.insert(
            name.to_owned(),
            Value::Array(rows(
                connection,
                &format!(
                    "SELECT * FROM \"{}\" ORDER BY rowid",
                    name.replace('"', "\"\"")
                ),
            )?),
        );
    }
    let integrity: String = connection.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
    Ok(
        json!({"tables":tables, "schema":rows(connection,"SELECT type,name,tbl_name,sql FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%' ORDER BY type,name")?,"integrity":integrity,"foreignKeyViolations":rows(connection,"PRAGMA foreign_key_check")?}),
    )
}
fn db_snapshot(db: &Database<'_>) -> TestResult<Value> {
    Ok(db.barrier_read(|tx| snapshot(tx))?.wait()?)
}
// This is the EXISTING producer's declared timestamp policy, not a new
// comparison normalization: preservation/generator/databases.go::fixTimes.
// Apply only to these independent synthetic copies AFTER checking real-time
// effects. Payload/version/revision/attention/other domain bytes are exact.
#[cfg(unix)]
fn producer_times(db: &Database<'_>) -> TestResult {
    committed(db.transaction(WorkCancellation::default(), |tx| {
        for table in rows(tx,"SELECT name FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name")? {
            let name = table["name"].as_str().ok_or(StorageError::IntegrityFailed)?.replace('"', "\"\"");
            for column in rows(tx, &format!("PRAGMA table_info(\"{name}\")"))? {
                let column = column["name"].as_str().ok_or(StorageError::IntegrityFailed)?;
                if matches!(column,"created_at"|"updated_at"|"applied_at"|"added_at"|"accepted_at"|"started_at"|"finished_at") {
                    tx.execute(&format!("UPDATE \"{name}\" SET \"{column}\"=?1 WHERE \"{column}\" IS NOT NULL AND \"{column}\"!=?1"), ["2000-01-01 00:00:00"])?;
                }
            }
        }
        Ok(())
    })?.wait()?)?;
    Ok(())
}
#[cfg(unix)]
fn corpus_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../preservation/corpus/databases")
}

#[cfg(unix)]
fn prefix_query_outcomes(db: &Database<'_>, prefix: usize, input: &[u8]) -> TestResult {
    let captured: Value =
        serde_json::from_slice(include_bytes!("fixtures/compatibility/prefix-queries.json"))?;
    assert_eq!(
        captured["oracleCommit"],
        "73a93303a9a44cf0a92eaf525331561ed8c04460"
    );
    let digest = |bytes: &[u8]| {
        librairii_core::Checksum::from_bytes(Sha256::digest(bytes).into()).to_string()
    };
    assert_eq!(
        captured["generatorSha256"],
        digest(include_bytes!(
            "../../../scripts/storage-query-oracle.go.in"
        ))
    );
    assert_eq!(
        captured["oracleSourceManifestSha256"],
        digest(include_bytes!("../../../preservation/oracle-source.json"))
    );
    let prefixes = captured["prefixes"].as_array().ok_or("prefixes missing")?;
    assert_eq!(prefixes.len(), 18);
    let reference = &prefixes[prefix - 1];
    assert_eq!(reference["prefix"], prefix);
    assert_eq!(reference["inputSha256"], digest(input));
    let cases = reference["cases"].as_array().ok_or("cases missing")?;
    assert_eq!(cases.len(), 6);
    for case in cases {
        let raw = case["request"].as_str().ok_or("request missing")?;
        let request = librairii_core::collection_codec::url::restore_request(raw.as_bytes())?;
        let count = db.count(request.clone(), "en-GB".into())?.wait();
        let page = db.search(request, "en-GB".into())?.wait();
        if case["error"] == "invalid_query" {
            for result in [count.map(|_| ()), page.map(|_| ())] {
                assert!(
                    matches!(result, Err(StorageError::QueryInput(_))),
                    "prefix{prefix} {raw}: {result:?}"
                );
            }
        } else {
            assert!(case.get("error").is_none(), "unknown oracle error");
            assert_eq!(
                serde_json::to_value(count?)?,
                case["count"],
                "prefix{prefix} {raw}"
            );
            assert_eq!(
                serde_json::to_value(page?)?,
                case["page"],
                "prefix{prefix} {raw}"
            );
        }
    }
    Ok(())
}

#[test]
#[cfg(unix)]
fn schema_only_then_explicit_same_writer_projection_matches_all_eighteen_go_prefixes() -> TestResult
{
    let mut deltas = 0;
    for prefix in 1..=18 {
        let roots = TestRoots::new()?;
        let library = isolated(&roots)?;
        let source = corpus_root().join(format!("prefix-{prefix:03}.sqlite3"));
        let original = std::fs::read(&source)?;
        std::fs::write(library.layout().database(), &original)?;
        let oracle: Value = serde_json::from_slice(&std::fs::read(
            corpus_root().join(format!("prefix-{prefix:03}.json")),
        )?)?;
        let mut db = Database::open_isolated(&library)?;
        producer_times(&db)?;
        let schema_only = db_snapshot(&db)?;
        assert_eq!(
            schema_only["schema"], oracle["afterGoOpen"]["schema"],
            "prefix{prefix}: actual schema-only phase"
        );
        let mut non_collection = schema_only.clone();
        let mut expected_non_collection = oracle["afterGoOpen"].clone();
        let before_rows = non_collection["tables"]
            .as_object_mut()
            .ok_or("tables missing")?
            .remove("collections")
            .ok_or("collections missing")?;
        let expected_rows = expected_non_collection["tables"]
            .as_object_mut()
            .ok_or("tables missing")?
            .remove("collections")
            .ok_or("collections missing")?;
        assert_eq!(
            non_collection, expected_non_collection,
            "prefix{prefix}: every other domain row/constraint/trigger"
        );
        let before_rows = before_rows.as_array().ok_or("rows missing")?;
        let expected_rows = expected_rows.as_array().ok_or("rows missing")?;
        assert_eq!(before_rows.len(), expected_rows.len());
        let original_rows = oracle["before"]["tables"]
            .get(if prefix < 16 {
                "shelves"
            } else {
                "collections"
            })
            .and_then(Value::as_array);
        for (index, (actual, expected)) in before_rows.iter().zip(expected_rows).enumerate() {
            let original_row = original_rows
                .and_then(|rows| rows.get(index))
                .ok_or("original Collection missing")?;
            assert_eq!(
                actual["query_version"], original_row["query_version"],
                "schema-only must not project"
            );
            assert_eq!(
                actual["query_payload"], original_row["query_payload"],
                "schema-only must not project"
            );
            for (key, value) in actual.as_object().ok_or("row missing")? {
                if value != &expected[key] {
                    assert!(
                        matches!(key.as_str(), "query_version" | "query_payload" | "revision"),
                        "unexplained prefix{prefix} difference {key}"
                    );
                    deltas += 1;
                }
            }
        }
        let ids = db
            .barrier_read(|tx| {
                Ok(tx
                    .prepare("SELECT id FROM collections ORDER BY position,id")?
                    .query_map([], |row| row.get::<_, i64>(0))?
                    .collect::<rusqlite::Result<Vec<_>>>()?)
            })?
            .wait()?;
        let before_epoch = db
            .barrier_read(|tx| {
                Ok(tx.query_row("SELECT unixepoch()", [], |row| row.get::<_, i64>(0))?)
            })?
            .wait()?;
        let mut opened = Vec::new();
        for id in ids {
            let result = inspection(&db, id)?;
            assert!(result.is_openable());
            assert_eq!(result.attention, None);
            let c = result.collection;
            opened.push(json!({"id":id,"error":"","result":{"collection":{"id":c.id,"name":String::from_utf8(c.name)?,"normalizedName":String::from_utf8(c.normalized_name)?,"position":c.position,"queryVersion":c.query_version,"queryPayload":String::from_utf8(c.query_payload)?,"validity":"valid","revision":c.revision},"query":serde_json::from_str::<Value>(result.decoded_query_payload.as_deref().ok_or("query absent")?)?}}));
            let (updated, now) = db
                .barrier_read(move |tx| {
                    Ok(tx.query_row(
                        "SELECT unixepoch(updated_at),unixepoch() FROM collections WHERE id=?",
                        [id],
                        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
                    )?)
                })?
                .wait()?;
            assert!(
                updated >= before_epoch && updated <= now,
                "actual CURRENT_TIMESTAMP effect is required"
            );
            let after_first = db_snapshot(&db)?;
            inspection(&db, id)?;
            assert_eq!(
                db_snapshot(&db)?,
                after_first,
                "repeat Inspect must preserve payload/revision/timestamps"
            );
        }
        assert_eq!(
            Value::Array(opened),
            oracle["openedCollections"],
            "prefix{prefix}: actual Open domain outcome"
        );
        producer_times(&db)?;
        assert_eq!(
            db_snapshot(&db)?,
            oracle["afterGoOpen"],
            "prefix{prefix}: explicit service-open phase must close EVERY delta"
        );
        prefix_query_outcomes(&db, prefix, &original)?;
        db.close()?;
        assert_eq!(
            std::fs::read(&source)?,
            original,
            "immutable prefix input changed"
        );
    }
    assert_eq!(
        deltas, 24,
        "retain all24 SQL-only differences until explicit projection closes them"
    );
    Ok(())
}

#[test]
fn inspection_is_lazy_idempotent_and_never_heals_existing_attention() -> TestResult {
    let roots = TestRoots::new()?;
    let library = isolated(&roots)?;
    let mut db = Database::open_isolated(&library)?;
    seed(&db, 1, 1, r#"{"name":" ÉTÉ ","page":9}"#, "valid")?;
    seed(&db, 2, 1, r#"{"name":" Moon "}"#, "needs_attention")?;
    let first = inspection(&db, 1)?;
    assert!(first.is_openable());
    assert_eq!(first.collection.query_version, 5);
    assert_eq!(first.collection.query_payload, br#"{"name":"ete"}"#);
    assert_eq!(first.collection.revision, 2);
    let before_repeat = db_snapshot(&db)?;
    assert_eq!(inspection(&db, 1)?, first);
    assert_eq!(db_snapshot(&db)?, before_repeat);
    let attention = inspection(&db, 2)?;
    assert!(!attention.is_openable());
    assert_eq!(
        attention.attention,
        Some(CollectionAttention::MissingCriteria)
    );
    assert_eq!(
        attention.collection.validity,
        CollectionValidity::NeedsAttention
    );
    assert_eq!(attention.collection.revision, 2);
    assert!(attention.decoded_query_payload.is_some());
    assert_eq!(inspection(&db, 2)?, attention);
    db.close()?;
    Ok(())
}

#[test]
fn missing_and_wrong_kind_or_value_criteria_are_distinct_from_sql_failure() -> TestResult {
    let roots = TestRoots::new()?;
    let library = isolated(&roots)?;
    let mut db = Database::open_isolated(&library)?;
    for (id, payload) in [
        (
            1,
            r#"{"booleanFilters":[{"definitionId":99,"state":"true"}]}"#,
        ),
        (
            2,
            r#"{"choiceFilters":[{"definitionId":1,"valueIds":[9]}]}"#,
        ),
        (
            3,
            r#"{"booleanFilters":[{"definitionId":1,"state":"true"}]}"#,
        ),
    ] {
        seed(&db, id, 5, payload, "valid")?;
    }
    committed(db.transaction(WorkCancellation::default(),|tx| {
        tx.execute("INSERT INTO tag_definitions(id,key,normalized_key,label,color,kind,source,position) VALUES(1,'choice','choice','Choice','#123456','choice','user',1)",[])?;
        Ok(())
    })?.wait()?)?;
    for id in 1..=3 {
        let inspected = inspection(&db, id)?;
        assert_eq!(
            inspected.attention,
            Some(CollectionAttention::MissingCriteria)
        );
        assert_eq!(inspected.collection.revision, 2);
        assert!(inspected.decoded_query_payload.is_none());
        assert_eq!(
            inspection(&db, id)?,
            inspected,
            "repeated attention must not increment revision"
        );
    }
    seed(
        &db,
        4,
        5,
        r#"{"booleanFilters":[{"definitionId":1,"state":"true"}]}"#,
        "valid",
    )?;
    committed(
        db.transaction(WorkCancellation::default(), |tx| {
            tx.execute_batch("ALTER TABLE tag_definitions RENAME TO saved_definitions")?;
            Ok(())
        })?
        .wait()?,
    )?;
    let before = db_snapshot(&db)?;
    assert!(matches!(
        db.inspect_collection_compatibility(4, WorkCancellation::default())?
            .wait()?,
        TransactionOutcome::ProvenNotCommitted(StorageError::Sqlite(_))
    ));
    assert_eq!(
        db_snapshot(&db)?,
        before,
        "real SQL failure must not mark attention"
    );
    committed(
        db.transaction(WorkCancellation::default(), |tx| {
            tx.execute_batch("ALTER TABLE saved_definitions RENAME TO tag_definitions")?;
            Ok(())
        })?
        .wait()?,
    )?;
    db.close()?;
    Ok(())
}

#[test]
fn invalid_codec_marks_attention_but_trigger_abort_cancel_and_not_found_never_fake_commit()
-> TestResult {
    let roots = TestRoots::new()?;
    let library = isolated(&roots)?;
    let mut db = Database::open_isolated(&library)?;
    seed(&db, 1, 99, "{}", "valid")?;
    seed(&db, 2, 5, r#"{"unknown":null}"#, "valid")?;
    seed(&db, 3, 1, r#"{"name":"Moon"}"#, "valid")?;
    seed(&db, 4, 5, r#"{"unknown":null}"#, "valid")?;
    for id in 1..=2 {
        let inspected = inspection(&db, id)?;
        assert_eq!(
            inspected.attention,
            Some(CollectionAttention::UnmigratableQuery)
        );
        assert_eq!(inspected.collection.revision, 2);
        assert_eq!(inspection(&db, id)?, inspected);
    }
    let token = WorkCancellation::default();
    token.cancel();
    let before = db_snapshot(&db)?;
    assert!(matches!(
        db.inspect_collection_compatibility(3, token)?.wait()?,
        TransactionOutcome::ProvenNotCommitted(StorageError::Cancelled)
    ));
    assert_eq!(db_snapshot(&db)?, before);
    committed(db.transaction(WorkCancellation::default(),|tx| {tx.execute_batch("CREATE TRIGGER compatibility_inject_failure AFTER UPDATE OF query_payload ON collections BEGIN SELECT RAISE(ABORT,'synthetic projection failure'); END")?;Ok(())})?.wait()?)?;
    let before = db_snapshot(&db)?;
    assert!(matches!(
        db.inspect_collection_compatibility(3, WorkCancellation::default())?
            .wait()?,
        TransactionOutcome::ProvenNotCommitted(StorageError::Sqlite(_))
    ));
    assert_eq!(
        db_snapshot(&db)?,
        before,
        "payload AND revision trigger effects must roll back"
    );
    committed(
        db.transaction(WorkCancellation::default(), |tx| {
            tx.execute_batch("DROP TRIGGER compatibility_inject_failure")?;
            Ok(())
        })?
        .wait()?,
    )?;
    committed(db.transaction(WorkCancellation::default(), |tx| {
        tx.execute_batch("CREATE TRIGGER compatibility_attention_abort AFTER UPDATE OF validity_state ON collections BEGIN SELECT RAISE(ABORT,'synthetic attention failure'); END")?;
        Ok(())
    })?.wait()?)?;
    let before = db_snapshot(&db)?;
    assert!(matches!(
        db.inspect_collection_compatibility(4, WorkCancellation::default())?
            .wait()?,
        TransactionOutcome::ProvenNotCommitted(StorageError::Sqlite(_))
    ));
    assert_eq!(
        db_snapshot(&db)?,
        before,
        "attention AND revision effects must roll back together"
    );
    committed(
        db.transaction(WorkCancellation::default(), |tx| {
            tx.execute_batch("DROP TRIGGER compatibility_attention_abort")?;
            Ok(())
        })?
        .wait()?,
    )?;
    let token = WorkCancellation::default();
    let pending = db.inspect_collection_compatibility(3, token.clone())?;
    let result = committed(pending.wait()?)?;
    token.cancel();
    assert_eq!(result.collection.revision, 2);
    assert_eq!(
        inspection(&db, 3)?,
        result,
        "late cancellation cannot undo committed projection"
    );
    assert!(matches!(
        db.inspect_collection_compatibility(999, WorkCancellation::default())?
            .wait()?,
        TransactionOutcome::ProvenNotCommitted(StorageError::CollectionNotFound)
    ));
    db.close()?;
    Ok(())
}

#[test]
fn actual_reference_kinds_value_owners_recursive_branches_and_retirement_remain_distinct()
-> TestResult {
    let roots = TestRoots::new()?;
    let library = isolated(&roots)?;
    let mut db = Database::open_isolated(&library)?;
    committed(db.transaction(WorkCancellation::default(),|tx| {
        tx.execute_batch("INSERT INTO legacy_broken_tag(singleton,definition_id) VALUES(1,7);
            INSERT INTO tag_definitions(id,key,normalized_key,label,color,kind,source,position) VALUES(1,'choice','choice','Choice','#123456','choice','user',1),(2,'other','other','Other','#123456','choice','user',2),(7,'reused','reused','Reused','#123456','boolean','user',7);
            INSERT INTO tag_values(id,definition_id,key,normalized_key,label,position) VALUES(11,1,'ok','ok','OK',0),(12,2,'other','other','Other',0);")?;
        Ok(())
    })?.wait()?)?;
    seed(
        &db,
        1,
        5,
        r#"{"anyOf":[{"choiceFilters":[{"definitionId":1,"valueIds":[11,11]}]}]}"#,
        "valid",
    )?;
    seed(
        &db,
        2,
        5,
        r#"{"anyOf":[{"choiceFilters":[{"definitionId":1,"valueIds":[12]}]}]}"#,
        "valid",
    )?;
    seed(
        &db,
        3,
        3,
        r#"{"booleanFilters":[{"definitionId":7,"state":"true"}]}"#,
        "valid",
    )?;
    seed(
        &db,
        4,
        4,
        r#"{"booleanFilters":[{"definitionId":7,"state":"true"}]}"#,
        "valid",
    )?;
    let correct = inspection(&db, 1)?;
    assert!(correct.is_openable());
    assert_eq!(
        correct.collection.query_payload,
        br#"{"anyOf":[{"choiceFilters":[{"definitionId":1,"valueIds":[11]}]}]}"#
    );
    assert_eq!(
        inspection(&db, 2)?.attention,
        Some(CollectionAttention::MissingCriteria)
    );
    assert_eq!(inspection(&db, 3)?.collection.query_payload, b"{}");
    assert_eq!(
        inspection(&db, 4)?.collection.query_payload,
        br#"{"booleanFilters":[{"definitionId":7,"state":"true"}]}"#
    );
    seed(&db, 5, 1, "{}", "valid")?;
    committed(
        db.transaction(WorkCancellation::default(), |tx| {
            tx.execute_batch("ALTER TABLE legacy_broken_tag RENAME TO saved_legacy")?;
            Ok(())
        })?
        .wait()?,
    )?;
    let before = db_snapshot(&db)?;
    assert!(matches!(
        db.inspect_collection_compatibility(5, WorkCancellation::default())?
            .wait()?,
        TransactionOutcome::ProvenNotCommitted(StorageError::Sqlite(_))
    ));
    assert_eq!(
        db_snapshot(&db)?,
        before,
        "actual legacy-reference SQL failure is not attention"
    );
    committed(
        db.transaction(WorkCancellation::default(), |tx| {
            tx.execute_batch("ALTER TABLE saved_legacy RENAME TO legacy_broken_tag")?;
            Ok(())
        })?
        .wait()?,
    )?;
    db.close()?;
    Ok(())
}

#[test]
fn projection_commit_failure_is_uncertain_fences_retry_and_retains_fresh_writer_readback()
-> TestResult {
    let roots = TestRoots::new()?;
    let library = isolated(&roots)?;
    let mut db = Database::open_isolated(&library)?;
    seed(&db, 1, 1, r#"{"name":"Moon"}"#, "valid")?;
    committed(db.transaction(WorkCancellation::default(),|tx| {
        tx.execute_batch("CREATE TABLE compatibility_commit_parent(id INTEGER PRIMARY KEY);
            CREATE TABLE compatibility_commit_child(parent INTEGER REFERENCES compatibility_commit_parent(id) DEFERRABLE INITIALLY DEFERRED);
            CREATE TRIGGER compatibility_commit_inject AFTER UPDATE OF query_payload ON collections BEGIN INSERT INTO compatibility_commit_child(parent) VALUES(999); END;")?;
        Ok(())
    })?.wait()?)?;
    let before = db_snapshot(&db)?;
    assert!(matches!(
        db.inspect_collection_compatibility(1, WorkCancellation::default())?
            .wait()?,
        TransactionOutcome::CommitUncertain(StorageError::Sqlite(_))
    ));
    assert!(
        matches!(
            db.inspect_collection_compatibility(1, WorkCancellation::default()),
            Err(StorageError::RuntimeFenced)
        ),
        "no blind retry after commit error"
    );
    assert_eq!(
        db_snapshot(&db)?,
        before,
        "fresh same-writer readback observes actual rollback; error alone was not proof"
    );
    committed(db.repair_transaction(|tx| {
        tx.execute_batch("DROP TRIGGER compatibility_commit_inject;DROP TABLE compatibility_commit_child;DROP TABLE compatibility_commit_parent;")?;
        Ok(())
    })?.wait()?)?;
    assert!(
        matches!(
            db.inspect_collection_compatibility(1, WorkCancellation::default()),
            Err(StorageError::RuntimeFenced)
        ),
        "readback/repair does not silently reactivate ordinary work"
    );
    db.close()?;
    Ok(())
}

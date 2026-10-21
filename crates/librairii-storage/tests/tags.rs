#![cfg(unix)]
use librairii_core::tagging::{CreateDefinition, Kind, TagError};
use librairii_storage::{
    IsolatedLibrary, Result, StorageError,
    database::{Database, TransactionOutcome, WorkCancellation},
    tagging::{Command, Response},
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
    committed(db.edit_tags(command, WorkCancellation::default())?.wait()?)
}
fn create(db: &Database<'_>, key: &str, kind: Kind) -> Result<i64> {
    match edit(
        db,
        Command::CreateDefinition(CreateDefinition {
            key: key.into(),
            label: key.into(),
            color: "#405CF5".into(),
            kind,
        }),
    )? {
        Response::Definition(d) => Ok(d.id),
        _ => Err(StorageError::CompatibilityInvariant),
    }
}
fn value(db: &Database<'_>, id: i64, key: &str) -> Result<i64> {
    match edit(
        db,
        Command::CreateValue {
            definition_id: id,
            key: key.into(),
            label: key.into(),
        },
    )? {
        Response::Value(v) => Ok(v.id),
        _ => Err(StorageError::CompatibilityInvariant),
    }
}
#[test]
fn crud_preserves_stable_keys_normalized_uniqueness_and_atomic_order() -> TestResult {
    let roots = TestRoots::new()?;
    let library = IsolatedLibrary::initialize_from_override(
        Some(roots.library().as_os_str()),
        "a0000000-0000-4000-8000-000000000001".parse()?,
    )?;
    let mut db = Database::open_isolated(&library)?;
    let a = create(&db, "Été favorites", Kind::Boolean)?;
    let b = create(&db, "Genre", Kind::Choice)?;
    assert!(matches!(
        create(&db, "ete--favorites", Kind::Boolean),
        Err(StorageError::Tag(TagError::DuplicateDefinition))
    ));
    let v = value(&db, b, "Été")?;
    assert!(matches!(
        value(&db, b, "ete"),
        Err(StorageError::Tag(TagError::DuplicateValue))
    ));
    assert!(matches!(
        value(&db, a, "bad"),
        Err(StorageError::Tag(TagError::ValuesNotAllowed))
    ));
    edit(
        &db,
        Command::RenameDefinition {
            id: a,
            label: "Renamed".into(),
        },
    )?;
    edit(
        &db,
        Command::RenameValue {
            id: v,
            label: "Summer".into(),
        },
    )?;
    edit(&db, Command::ReorderDefinitions(vec![b, a]))?;
    let before = db.tag_catalog()?.wait()?;
    assert_eq!(before[1].definition.key, "ete-favorites");
    assert_eq!(before[0].values[0].key, "ete");
    assert!(edit(&db, Command::ReorderDefinitions(vec![a, a])).is_err());
    assert_eq!(db.tag_catalog()?.wait()?, before);
    assert!(
        edit(
            &db,
            Command::RecolorDefinition {
                id: a,
                color: "#FFFFFF".into()
            }
        )
        .is_err()
    );
    create(&db, "broken", Kind::Boolean)?;
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
fn assignment(
    db: &Database<'_>,
    command: Command,
) -> Result<librairii_core::tagging::AssignmentResult> {
    match edit(db, command)? {
        Response::Assignment(v) => Ok(v),
        _ => Err(StorageError::CompatibilityInvariant),
    }
}
#[test]
fn bulk_assignment_mixed_states_additive_toggles_noops_and_all_target_validation() -> TestResult {
    let roots = TestRoots::new()?;
    let library = IsolatedLibrary::initialize_from_override(
        Some(roots.library().as_os_str()),
        "a0000000-0000-4000-8000-000000000001".parse()?,
    )?;
    let mut db = Database::open_isolated(&library)?;
    seed_stories(&db, 3)?;
    let a = create(&db, "flag", Kind::Boolean)?;
    let b = create(&db, "genre", Kind::Choice)?;
    let v = value(&db, b, "one")?;
    let w = value(&db, b, "two")?;
    let r = assignment(
        &db,
        Command::Boolean {
            story_ids: vec![1, 1, 2],
            definition_id: a,
            assigned: true,
        },
    )?;
    assert_eq!(
        (r.requested_stories, r.changed_stories, r.assignments_added),
        (2, 2, 2)
    );
    assert_eq!(
        assignment(
            &db,
            Command::Boolean {
                story_ids: vec![1, 2],
                definition_id: a,
                assigned: true
            }
        )?
        .changed_stories,
        0
    );
    assignment(
        &db,
        Command::ChoiceValues {
            story_ids: vec![1],
            definition_id: b,
            value_ids: vec![v],
        },
    )?;
    assignment(
        &db,
        Command::ChoiceToggle {
            story_ids: vec![1, 2],
            definition_id: b,
            value_id: w,
            assigned: true,
        },
    )?;
    let before = db.assignment_workspace(vec![1, 2, 3])?.wait()?;
    assert_eq!(before.states[0].assigned_stories, 2);
    assert_eq!(before.states[1].values[0].assigned_stories, 1);
    assert_eq!(before.states[1].values[1].assigned_stories, 2);
    assert!(
        edit(
            &db,
            Command::Boolean {
                story_ids: vec![3, 999],
                definition_id: a,
                assigned: true
            }
        )
        .is_err()
    );
    assert!(
        edit(
            &db,
            Command::ChoiceValues {
                story_ids: vec![1, 2],
                definition_id: b,
                value_ids: vec![999]
            }
        )
        .is_err()
    );
    assert_eq!(db.assignment_workspace(vec![1, 2, 3])?.wait()?, before);
    let cancel = WorkCancellation::default();
    cancel.cancel();
    assert!(
        committed(
            db.edit_tags(
                Command::Boolean {
                    story_ids: vec![3],
                    definition_id: a,
                    assigned: true
                },
                cancel
            )?
            .wait()?
        )
        .is_err()
    );
    assert_eq!(db.assignment_workspace(vec![1, 2, 3])?.wait()?, before);
    db.close()?;
    Ok(())
}
#[test]
fn confirmed_deletion_invalidates_entire_nested_collections_and_preserves_shelf_decisions()
-> TestResult {
    use librairii_storage::tagging::DeleteTarget;
    let roots = TestRoots::new()?;
    let library = IsolatedLibrary::initialize_from_override(
        Some(roots.library().as_os_str()),
        "a0000000-0000-4000-8000-000000000001".parse()?,
    )?;
    let mut db = Database::open_isolated(&library)?;
    seed_stories(&db, 2)?;
    let d = create(&db, "genre", Kind::Choice)?;
    let v = value(&db, d, "calm")?;
    assignment(
        &db,
        Command::ChoiceValues {
            story_ids: vec![1],
            definition_id: d,
            value_ids: vec![v],
        },
    )?;
    let payload = format!(
        r#"{{"anyOf":[{{"name":"moon"}},{{"choiceFilters":[{{"definitionId":{d},"valueIds":[{v}]}}]}}]}}"#
    );
    let retained = payload.clone();
    committed(db.transaction(WorkCancellation::default(),move|tx|{
 tx.execute("INSERT INTO collections(id,name,normalized_name,position,query_version,query_payload) VALUES(1,'Calm','calm',0,5,?)",[payload])?;
 tx.execute_batch("INSERT INTO shelves(id,name,normalized_name,position) VALUES(1,'Keep','keep',0); INSERT INTO shelf_stories(shelf_id,story_uuid,position,protected) VALUES(1,'10000000-0000-4000-8000-000000000001',0,1); INSERT INTO shelf_exclusions(shelf_id,story_uuid) VALUES(1,'10000000-0000-4000-8000-000000000002');")?;Ok(())})?.wait()?)?;
    let plan = db.tag_deletion_plan(DeleteTarget::Value(v))?.wait()?;
    assert_eq!(plan.assignment_count, 1);
    assert_eq!(plan.affected_collection_ids, vec![1]);
    assignment(
        &db,
        Command::ChoiceToggle {
            story_ids: vec![2],
            definition_id: d,
            value_id: v,
            assigned: true,
        },
    )?;
    assert!(matches!(
        edit(&db, Command::Delete(Box::new(plan))),
        Err(StorageError::Tag(TagError::DeletePlanStale))
    ));
    let plan = db.tag_deletion_plan(DeleteTarget::Value(v))?.wait()?;
    edit(&db, Command::Delete(Box::new(plan)))?;
    db.read(move |tx| {
        let (payload, state, revision): (String, String, i64) = tx.query_row(
            "SELECT query_payload,validity_state,revision FROM collections WHERE id=1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        assert_eq!(payload, retained);
        assert_eq!(state, "needs_attention");
        assert_eq!(revision, 2);
        assert_eq!(
            tx.query_row(
                "SELECT COUNT(*) FROM shelf_stories WHERE protected=1",
                [],
                |r| r.get::<_, i64>(0)
            )?,
            1
        );
        assert_eq!(
            tx.query_row("SELECT COUNT(*) FROM shelf_exclusions", [], |r| r
                .get::<_, i64>(0))?,
            1
        );
        Ok(())
    })?
    .wait()?;
    let plan = db.tag_deletion_plan(DeleteTarget::Definition(d))?.wait()?;
    edit(&db, Command::Delete(Box::new(plan)))?;
    db.close()?;
    Ok(())
}

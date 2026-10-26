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
#[test]
fn derived_age_identity_bands_and_custom_colors_survive_reseeding_and_projection() -> TestResult {
    use librairii_core::tagging::AgeConfig;
    let roots = TestRoots::new()?;
    let library = IsolatedLibrary::initialize_from_override(
        Some(roots.library().as_os_str()),
        "a0000000-0000-4000-8000-000000000001".parse()?,
    )?;
    let mut db = Database::open_isolated(&library)?;
    seed_stories(&db, 4)?;
    let id = committed(
        db.seed_age_facets(AgeConfig::default(), WorkCancellation::default())?
            .wait()?,
    )?;
    assert_eq!(db.tag_catalog()?.wait()?[0].definition.color, "#267D5D");
    edit(
        &db,
        Command::RecolorDefinition {
            id,
            color: "#405CF5".into(),
        },
    )?;
    assert!(
        edit(
            &db,
            Command::RenameDefinition {
                id,
                label: "Not age".into()
            }
        )
        .is_err()
    );
    assert!(value(&db, id, "other").is_err());
    assert!(
        assignment(
            &db,
            Command::ChoiceValues {
                story_ids: vec![1],
                definition_id: id,
                value_ids: vec![]
            }
        )
        .is_err()
    );
    committed(db.transaction(WorkCancellation::default(),|tx|{
 tx.execute_batch("INSERT INTO catalog_syncs(id,locale,started_at) VALUES('20000000-0000-4000-8000-000000000001','fr-FR','2026-01-01'); INSERT INTO catalog_snapshots(id,sync_id,locale,status,raw_path,raw_sha256,byte_size,record_count,fetched_at,activated_at) VALUES(1,'20000000-0000-4000-8000-000000000001','fr-FR','staged','catalog/age.json',printf('%064d',1),1,4,'2026-01-01',NULL);")?;
 for (story,min,max) in [(1,Some(3),Some(5)),(2,Some(6),Some(8)),(3,Some(3),Some(8)),(4,None,None)]{
 tx.execute("INSERT INTO official_story_metadata(snapshot_id,story_uuid,locale,title,minimum_age,maximum_age,provenance) SELECT 1,uuid,'fr-FR','Official',?2,?3,'lunii_catalog' FROM stories WHERE id=?1",rusqlite::params![story,min,max])?;
 }Ok(())})?.wait()?)?;
    let r = committed(
        db.rebuild_age_projection(1, AgeConfig::default(), WorkCancellation::default())?
            .wait()?,
    )?;
    assert_eq!(
        (r.matched_stories, r.assigned_stories, r.unassigned),
        (4, 2, 2)
    );
    committed(
        db.seed_age_facets(AgeConfig::default(), WorkCancellation::default())?
            .wait()?,
    )?;
    let ws = db.assignment_workspace(vec![1, 2, 3, 4])?.wait()?;
    assert_eq!(ws.catalog[0].definition.color, "#405CF5");
    assert_eq!(
        ws.states[0]
            .values
            .iter()
            .map(|v| v.assigned_stories)
            .collect::<Vec<_>>(),
        vec![1, 1]
    );
    db.close()?;
    Ok(())
}
#[test]
fn broken_flags_are_atomic_independent_visible_and_skipped_without_a_selection_cap_on_export()
-> TestResult {
    use librairii_core::{broken::ExportSkip, query::QueryRequest};
    let roots = TestRoots::new()?;
    let library = IsolatedLibrary::initialize_from_override(
        Some(roots.library().as_os_str()),
        "a0000000-0000-4000-8000-000000000001".parse()?,
    )?;
    let mut db = Database::open_isolated(&library)?;
    seed_stories(&db, 1001)?;
    let ordinary = create(&db, "broken", Kind::Boolean)?;
    assignment(
        &db,
        Command::Boolean {
            story_ids: vec![1],
            definition_id: ordinary,
            assigned: true,
        },
    )?;
    let ids = (1..=1000).collect::<Vec<_>>();
    let result = committed(
        db.set_broken(ids.clone(), true, WorkCancellation::default())?
            .wait()?,
    )?;
    assert_eq!(result.changed_stories, 1000);
    assert_eq!(
        db.broken_summary(vec![1, 1, 1001])?.wait()?.flagged_stories,
        1
    );
    assert!(
        committed(
            db.set_broken((1..=1001).collect(), false, WorkCancellation::default())?
                .wait()?
        )
        .is_err()
    );
    assert!(
        committed(
            db.set_broken(vec![1, 9999], false, WorkCancellation::default())?
                .wait()?
        )
        .is_err()
    );
    assert_eq!(
        committed(
            db.set_broken(vec![1, 1], true, WorkCancellation::default())?
                .wait()?
        )?
        .changed_stories,
        0
    );
    assert_eq!(
        db.count(QueryRequest::default(), "fr-FR".into())?.wait()?,
        1001
    );
    let page = db.search(QueryRequest::default(), "fr-FR".into())?.wait()?;
    assert_eq!(page.total_items, 1001);
    assert!(page.stories[0].broken);
    let skipped = db
        .read(|tx| {
            let mut skipped = 0;
            let mut ready = 0;
            librairii_storage::query::visit_scope(tx, QueryRequest::default(), "fr-FR", |s| {
                if s.manual_export_skip() == Some(ExportSkip::Broken) {
                    skipped += 1;
                } else {
                    ready += 1;
                }
                Ok(())
            })?;
            Ok((skipped, ready))
        })?
        .wait()?;
    assert_eq!(skipped, (1000, 1));
    assert_eq!(
        db.assignment_workspace(vec![1])?.wait()?.states[0].assigned_stories,
        1
    );
    db.close()?;
    let mut db = Database::open_isolated(&library)?;
    assert_eq!(db.broken_summary(ids)?.wait()?.flagged_stories, 1000);
    db.close()?;
    Ok(())
}
#[test]
fn undo_restores_only_changed_targets_and_atomically_rejects_stale_or_unavailable_batches()
-> TestResult {
    use librairii_core::broken::{FlagChange, FlagError};
    let roots = TestRoots::new()?;
    let library = IsolatedLibrary::initialize_from_override(
        Some(roots.library().as_os_str()),
        "a0000000-0000-4000-8000-000000000001".parse()?,
    )?;
    let mut db = Database::open_isolated(&library)?;
    seed_stories(&db, 3)?;
    let ordinary = create(&db, "broken", Kind::Boolean)?;
    assignment(
        &db,
        Command::Boolean {
            story_ids: vec![2],
            definition_id: ordinary,
            assigned: true,
        },
    )?;
    committed(
        db.set_broken(vec![1], true, WorkCancellation::default())?
            .wait()?,
    )?;
    let action = committed(
        db.set_broken(vec![1, 2, 3], true, WorkCancellation::default())?
            .wait()?,
    )?;
    assert_eq!(
        action
            .changes
            .iter()
            .map(|c| c.story_id)
            .collect::<Vec<_>>(),
        vec![2, 3]
    );
    let restore = committed(
        db.restore_broken(action.changes, WorkCancellation::default())?
            .wait()?,
    )?;
    assert_eq!(restore.changed_stories, 2);
    assert_eq!(db.broken_summary(vec![1, 2, 3])?.wait()?.flagged_stories, 1);
    let action = committed(
        db.set_broken(vec![1, 2, 3], true, WorkCancellation::default())?
            .wait()?,
    )?;
    committed(
        db.set_broken(vec![3], false, WorkCancellation::default())?
            .wait()?,
    )?;
    assert!(matches!(
        committed(
            db.restore_broken(action.changes.clone(), WorkCancellation::default())?
                .wait()?
        ),
        Err(StorageError::Flag(FlagError::RestoreStale))
    ));
    assert_eq!(db.broken_summary(vec![2])?.wait()?.flagged_stories, 1);
    committed(
        db.set_broken(vec![3], true, WorkCancellation::default())?
            .wait()?,
    )?;
    committed(
        db.transaction(WorkCancellation::default(), |tx| {
            tx.execute("DELETE FROM stories WHERE id=3", [])?;
            Ok(())
        })?
        .wait()?,
    )?;
    assert!(matches!(
        committed(
            db.restore_broken(action.changes, WorkCancellation::default())?
                .wait()?
        ),
        Err(StorageError::Flag(FlagError::StoryUnavailable))
    ));
    assert_eq!(db.broken_summary(vec![2])?.wait()?.flagged_stories, 1);
    assert_eq!(
        db.assignment_workspace(vec![2])?.wait()?.states[0].assigned_stories,
        1
    );
    for changes in [
        vec![],
        vec![FlagChange {
            story_id: 2,
            previous: true,
            current: true,
        }],
        vec![
            FlagChange {
                story_id: 2,
                previous: false,
                current: true
            };
            2
        ],
    ] {
        assert!(
            committed(
                db.restore_broken(changes, WorkCancellation::default())?
                    .wait()?
            )
            .is_err()
        );
    }
    db.close()?;
    Ok(())
}
fn assignment_json(v: &librairii_core::tagging::AssignmentResult) -> serde_json::Value {
    serde_json::json!({"requestedStories":v.requested_stories,"changedStories":v.changed_stories,"assignmentsAdded":v.assignments_added,"assignmentsRemoved":v.assignments_removed})
}
fn flag_json(v: &librairii_core::broken::FlagResult) -> serde_json::Value {
    serde_json::json!({"requestedStories":v.requested_stories,"changedStories":v.changed_stories,"changes":v.changes.iter().map(|c|serde_json::json!({"storyId":c.story_id,"previous":c.previous,"current":c.current})).collect::<Vec<_>>()})
}
fn workspace_json(w: &librairii_core::tagging::AssignmentWorkspace) -> serde_json::Value {
    serde_json::json!({"catalog":{"definitions":w.catalog.iter().map(|e|{let d=&e.definition;serde_json::json!({"id":d.id,"key":d.key,"normalizedKey":d.normalized_key,"label":d.label,"color":d.color,"kind":d.kind.as_str(),"source":d.source,"presentation":d.presentation,"position":d.position,"protected":d.protected,"values":e.values.iter().map(|v|serde_json::json!({"id":v.id,"definitionId":v.definition_id,"key":v.key,"normalizedKey":v.normalized_key,"label":v.label,"position":v.position})).collect::<Vec<_>>()})}).collect::<Vec<_>>()},"requestedStories":w.requested_stories,"states":w.states.iter().map(|s|serde_json::json!({"definitionId":s.definition_id,"assignedStories":s.assigned_stories,"values":s.values.iter().map(|v|serde_json::json!({"valueId":v.value_id,"assignedStories":v.assigned_stories})).collect::<Vec<_>>()})).collect::<Vec<_>>()})
}
#[test]
fn actual_go_tagging_and_broken_vertical_reference_matches_every_recorded_field() -> TestResult {
    use librairii_core::tagging::AgeConfig;
    use librairii_storage::tagging::DeleteTarget;
    use serde_json::json;
    let expected: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/tags/vertical.json"))?;
    let roots = TestRoots::new()?;
    let library = IsolatedLibrary::initialize_from_override(
        Some(roots.library().as_os_str()),
        "a0000000-0000-4000-8000-000000000001".parse()?,
    )?;
    let mut db = Database::open_isolated(&library)?;
    seed_stories(&db, 3)?;
    let age = committed(
        db.seed_age_facets(AgeConfig::default(), WorkCancellation::default())?
            .wait()?,
    )?;
    let a = match edit(
        &db,
        Command::CreateDefinition(CreateDefinition {
            key: "Été favorites".into(),
            label: "Favorite".into(),
            color: "#405CF5".into(),
            kind: Kind::Boolean,
        }),
    )? {
        Response::Definition(d) => d.id,
        _ => return Err("definition".into()),
    };
    let b = create(&db, "Genre", Kind::Choice)?;
    let v = value(&db, b, "Calm")?;
    let w = value(&db, b, "Dreamy")?;
    let mut events = vec![];
    for command in [
        Command::Boolean {
            story_ids: vec![1, 1, 2],
            definition_id: a,
            assigned: true,
        },
        Command::ChoiceValues {
            story_ids: vec![1],
            definition_id: b,
            value_ids: vec![v],
        },
        Command::ChoiceToggle {
            story_ids: vec![1, 2],
            definition_id: b,
            value_id: w,
            assigned: true,
        },
        Command::ChoiceToggle {
            story_ids: vec![1, 2],
            definition_id: b,
            value_id: w,
            assigned: true,
        },
    ] {
        events.push(assignment_json(&assignment(&db, command)?));
    }
    events.push(json!({"invalidAtomic":matches!(edit(&db,Command::Boolean{story_ids:vec![3,999],definition_id:a,assigned:true}),Err(StorageError::Tag(TagError::StoryNotFound)))}));
    let workspace = workspace_json(&db.assignment_workspace(vec![1, 2, 3])?.wait()?);
    committed(db.transaction(WorkCancellation::default(),move|tx|{let payload=json!({"anyOf":[{"name":"moon"},{"choiceFilters":[{"definitionId":b,"valueIds":[v]}]}]}).to_string();tx.execute("INSERT INTO collections(id,name,normalized_name,position,query_version,query_payload) VALUES(1,'Calm','calm',0,5,?)",[payload])?;Ok(())})?.wait()?)?;
    let plan = db.tag_deletion_plan(DeleteTarget::Value(v))?.wait()?;
    events.push(
        json!({"assignments":plan.assignment_count,"collections":plan.affected_collection_ids}),
    );
    assignment(
        &db,
        Command::ChoiceToggle {
            story_ids: vec![3],
            definition_id: b,
            value_id: v,
            assigned: true,
        },
    )?;
    events.push(json!({"staleDelete":matches!(edit(&db,Command::Delete(Box::new(plan))),Err(StorageError::Tag(TagError::DeletePlanStale)))}));
    let plan = db.tag_deletion_plan(DeleteTarget::Value(v))?.wait()?;
    edit(&db, Command::Delete(Box::new(plan)))?;
    let (state, revision) = db
        .read(|tx| {
            Ok(tx.query_row(
                "SELECT validity_state,revision FROM collections WHERE id=1",
                [],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)),
            )?)
        })?
        .wait()?;
    edit(
        &db,
        Command::RecolorDefinition {
            id: age,
            color: "#405CF5".into(),
        },
    )?;
    committed(
        db.seed_age_facets(AgeConfig::default(), WorkCancellation::default())?
            .wait()?,
    )?;
    let first = committed(
        db.set_broken(vec![1], true, WorkCancellation::default())?
            .wait()?,
    )?;
    let action = committed(
        db.set_broken(vec![1, 2, 2], true, WorkCancellation::default())?
            .wait()?,
    )?;
    let undo = committed(
        db.restore_broken(action.changes.clone(), WorkCancellation::default())?
            .wait()?,
    )?;
    let flags = db
        .read(|tx| {
            let mut stmt = tx.prepare("SELECT is_broken FROM stories ORDER BY id")?;
            Ok(stmt
                .query_map([], |r| r.get::<_, bool>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?)
        })?
        .wait()?;
    let actual = json!({"events":events,"workspace":workspace,"collectionState":state,"collectionRevision":revision,"ageColor":"#405CF5","firstFlag":flag_json(&first),"mixedFlag":flag_json(&action),"undo":flag_json(&undo),"flags":flags});
    assert_eq!(actual, expected);
    db.close()?;
    Ok(())
}
#[test]
fn precommit_cancellation_and_sql_failures_roll_back_real_tag_and_flag_writes() -> TestResult {
    let roots = TestRoots::new()?;
    let library = IsolatedLibrary::initialize_from_override(
        Some(roots.library().as_os_str()),
        "a0000000-0000-4000-8000-000000000001".parse()?,
    )?;
    let mut db = Database::open_isolated(&library)?;
    seed_stories(&db, 2)?;
    let a = create(&db, "favorite", Kind::Boolean)?;
    let cancel = WorkCancellation::default();
    let signal = cancel.clone();
    let outcome = db
        .transaction(cancel, move |tx| {
            let result = librairii_storage::tagging::execute(
                tx,
                Command::Boolean {
                    story_ids: vec![1, 2],
                    definition_id: a,
                    assigned: true,
                },
            )?;
            signal.cancel();
            Ok(result)
        })?
        .wait()?;
    assert!(matches!(
        outcome,
        TransactionOutcome::ProvenNotCommitted(StorageError::Cancelled)
    ));
    assert_eq!(
        db.assignment_workspace(vec![1, 2])?.wait()?.states[0].assigned_stories,
        0
    );
    committed(db.transaction(WorkCancellation::default(),|tx|{tx.execute_batch("CREATE TRIGGER fail_second_assignment BEFORE INSERT ON story_tag_assignments WHEN NEW.story_id=2 BEGIN SELECT RAISE(ABORT,'injected tag write failure'); END; CREATE TRIGGER fail_second_flag BEFORE UPDATE OF is_broken ON stories WHEN NEW.id=2 BEGIN SELECT RAISE(ABORT,'injected flag write failure'); END;")?;Ok(())})?.wait()?)?;
    assert!(
        edit(
            &db,
            Command::Boolean {
                story_ids: vec![1, 2],
                definition_id: a,
                assigned: true
            }
        )
        .is_err()
    );
    assert_eq!(
        db.assignment_workspace(vec![1, 2])?.wait()?.states[0].assigned_stories,
        0
    );
    assert!(
        committed(
            db.set_broken(vec![1, 2], true, WorkCancellation::default())?
                .wait()?
        )
        .is_err()
    );
    assert_eq!(db.broken_summary(vec![1, 2])?.wait()?.flagged_stories, 0);
    let cancel = WorkCancellation::default();
    let signal = cancel.clone();
    let outcome = db
        .transaction(cancel, move |tx| {
            let result = librairii_storage::broken::set(tx, &[1], true)?;
            signal.cancel();
            Ok(result)
        })?
        .wait()?;
    assert!(matches!(
        outcome,
        TransactionOutcome::ProvenNotCommitted(StorageError::Cancelled)
    ));
    assert_eq!(db.broken_summary(vec![1])?.wait()?.flagged_stories, 0);
    db.stop_ordinary()?;
    assert!(
        db.set_broken(vec![1], true, WorkCancellation::default())
            .is_err()
    );
    db.close()?;
    Ok(())
}
#[test]
fn deletion_inspects_valid_references_without_normalizing_unrelated_membership_fields() -> TestResult
{
    use librairii_storage::tagging::DeleteTarget;
    let roots = TestRoots::new()?;
    let library = IsolatedLibrary::initialize_from_override(
        Some(roots.library().as_os_str()),
        "a0000000-0000-4000-8000-000000000001".parse()?,
    )?;
    let mut db = Database::open_isolated(&library)?;
    let d = create(&db, "genre", Kind::Choice)?;
    let v = value(&db, d, "calm")?;
    committed(db.transaction(WorkCancellation::default(),move|tx|{
 for (id,validity) in [(1,"valid"),(2,"needs_attention")] {
 let payload=serde_json::json!({"name":"a".repeat(201),"compatibilities":["unsupported"],"anyOf":[{"choiceFilters":[{"definitionId":d,"valueIds":vec![v;101]}]}]}).to_string();
 tx.execute("INSERT INTO collections(id,name,normalized_name,position,query_version,query_payload,validity_state) VALUES(?1,'Source'||?1,'source'||?1,?1-1,5,?2,?3)",rusqlite::params![id,payload,validity])?;
 }Ok(())})?.wait()?)?;
    let plan = db.tag_deletion_plan(DeleteTarget::Value(v))?.wait()?;
    assert_eq!(plan.affected_collection_ids, vec![1, 2]);
    edit(&db, Command::Delete(Box::new(plan)))?;
    assert_eq!(db.read(|tx|Ok(tx.query_row("SELECT COUNT(*) FROM collections WHERE validity_state='needs_attention' AND instr(query_payload,'unsupported')>0",[],|r|r.get::<_,i64>(0))?))?.wait()?,2);
    db.close()?;
    Ok(())
}

#[test]
fn native_display_labels_preserve_boolean_choice_color_and_assignment_independence() -> TestResult {
    let roots = TestRoots::new()?;
    let library = IsolatedLibrary::initialize_from_override(
        Some(roots.library().as_os_str()),
        "e0000000-0000-4000-8000-000000000014".parse()?,
    )?;
    let mut db = Database::open_isolated(&library)?;
    seed_stories(&db, 2)?;
    let flag = create(&db, "broken", Kind::Boolean)?;
    let genre = create(&db, "Genre", Kind::Choice)?;
    let summer = value(&db, genre, "Summer")?;
    edit(
        &db,
        Command::Boolean {
            story_ids: vec![1],
            definition_id: flag,
            assigned: true,
        },
    )?;
    edit(
        &db,
        Command::ChoiceToggle {
            story_ids: vec![1, 2],
            definition_id: genre,
            value_id: summer,
            assigned: true,
        },
    )?;
    let before = db.assignment_workspace(vec![1, 2])?.wait()?;
    let labels = db.story_tag_labels(vec![1, 2])?.wait()?;
    assert_eq!(labels.get(&1).ok_or("story one")?.len(), 2);
    assert_eq!(
        labels.get(&1).ok_or("story one")?[0],
        librairii_core::tagging::DisplayTag {
            source: "user".into(),
            definition: "broken".into(),
            value: None,
            color: "#405CF5".into()
        }
    );
    assert_eq!(labels.get(&2).ok_or("story two")?[0].definition, "Genre");
    assert_eq!(
        labels.get(&2).ok_or("story two")?[0].value.as_deref(),
        Some("Summer")
    );
    assert_eq!(db.assignment_workspace(vec![1, 2])?.wait()?, before);
    assert!(db.story_tag_labels(Vec::new())?.wait()?.is_empty());
    assert!(db.story_tag_labels(vec![0]).is_err());
    db.close()?;
    Ok(())
}

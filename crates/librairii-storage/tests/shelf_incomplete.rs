#![cfg(any(target_os = "macos", target_os = "linux"))]
use librairii_core::{CollectionId, StoryUuid};
use librairii_storage::{
    IsolatedLibrary, Result, StorageError, collections,
    database::{Database, TransactionOutcome},
    shelves::{self, Action, Decision},
};
use librairii_testkit::TestRoots;

fn committed<T>(outcome: TransactionOutcome<T>) -> Result<T> {
    match outcome {
        TransactionOutcome::Committed(value) => Ok(value),
        TransactionOutcome::ProvenNotCommitted(error)
        | TransactionOutcome::CommitUncertain(error) => Err(error),
    }
}

fn uuid(id: i64) -> String {
    format!("10000000-0000-4000-8000-{id:012}")
}

fn collection(db: &Database<'_>, name: &str) -> Result<CollectionId> {
    let collections::Response::Collection(value) = committed(
        db.edit_collections(
            collections::Command::Create {
                name: name.into(),
                criteria: Default::default(),
            },
            Default::default(),
        )?
        .wait()?,
    )?
    else {
        return Err(StorageError::CompatibilityInvariant);
    };
    CollectionId::new(value.id).map_err(|_| StorageError::InvalidIdentity)
}

#[test]
fn healthy_addition_commits_while_invalid_source_keeps_all_removals_suspended_after_reopen()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    let roots = TestRoots::new()?;
    let library = IsolatedLibrary::initialize_from_override(
        Some(roots.library().as_os_str()),
        "a0000000-0000-4000-8000-000000000033".parse()?,
    )?;
    let mut db = Database::open_isolated(&library)?;
    committed(db.transaction(Default::default(), |tx| {
        for id in 1..=3 {
            tx.execute("INSERT INTO stories(id,uuid,display_name_normalized) VALUES(?1,printf('10000000-0000-4000-8000-%012d',?1),'story')", [id])?;
            tx.execute("INSERT INTO story_archives(story_id,original_filename,detected_format,sha256,byte_size,managed_path,validation_state) VALUES(?1,'story.zip','zip',printf('%064d',?1),1,'archives/'||?1,'valid')", [id])?;
        }
        Ok(())
    })?.wait()?)?;
    let healthy = collection(&db, "Healthy")?;
    let invalid = collection(&db, "Will need repair")?;
    let shelves::Response::Shelf(shelf) = committed(
        db.edit_shelves(
            shelves::Command::Create {
                name: "Mixed coverage".into(),
            },
            Default::default(),
        )?
        .wait()?,
    )?
    else {
        return Err("shelf".into());
    };
    let id = shelf.id;
    committed(
        db.attach_shelf_sources(id, vec![healthy, invalid], Default::default())?
            .wait()?,
    )?;
    let plan = db.shelf_update_preview(id, "fr-FR".into())?.wait()?;
    let mut approved = Vec::new();
    for n in [1, 2] {
        approved.push(Decision {
            uuid: uuid(n).parse::<StoryUuid>()?.into(),
            action: Action::Add,
        });
    }
    committed(
        db.apply_shelf_updates(plan, approved, Default::default())?
            .wait()?,
    )?;
    let before = db.shelf_members(id)?.wait()?;
    assert_eq!(before.len(), 2);
    assert!(
        before
            .iter()
            .all(|m| !m.manually_retained && m.origins.len() == 2)
    );
    committed(db.transaction(Default::default(), move |tx| {
        tx.execute("UPDATE stories SET display_name_normalized='old unrelated' WHERE id=1", [])?;
        tx.execute("DELETE FROM stories WHERE id=2", [])?;
        tx.execute("UPDATE stories SET display_name_normalized='match healthy' WHERE id=3", [])?;
        tx.execute("UPDATE collections SET query_payload='{\"name\":\"match\"}' WHERE id=?", [healthy.get()])?;
        tx.execute("UPDATE collections SET validity_state='needs_attention',query_payload='{}' WHERE id=?", [invalid.get()])?;
        Ok(())
    })?.wait()?)?;
    let collections::Observation::Ready(evaluated) = committed(
        db.evaluate_collection(
            healthy.get(),
            Default::default(),
            "fr-FR".into(),
            Default::default(),
        )?
        .wait()?,
    )?
    else {
        return Err("healthy source must remain evaluable".into());
    };
    assert_eq!(evaluated.page.total_items, 1);
    assert_eq!(
        evaluated
            .page
            .stories
            .into_iter()
            .map(|s| s.uuid)
            .collect::<Vec<_>>(),
        vec![uuid(3)]
    );
    let plan = db.shelf_update_preview(id, "fr-FR".into())?.wait()?;
    assert!(plan.preview().incomplete);
    assert!(!plan.preview().up_to_date);
    assert_eq!(plan.preview().removal_count(), 0);
    assert_eq!(plan.preview().addition_count(), 1);
    assert_eq!(plan.preview().additions_page(0)[0].uuid, uuid(3));
    assert_eq!(plan.preview().additions_page(0)[0].sources.len(), 1);
    assert_eq!(
        plan.preview().additions_page(0)[0].sources[0].collection_id,
        Some(healthy)
    );
    // Commit while the other source is still invalid; repairing first would
    // miss the positive incomplete-coverage acceptance contract.
    let saved = committed(
        db.apply_shelf_updates(
            plan,
            vec![Decision {
                uuid: uuid(3).parse::<StoryUuid>()?.into(),
                action: Action::Add,
            }],
            Default::default(),
        )?
        .wait()?,
    )?;
    assert_eq!(saved.count, 3);
    let after = db.shelf_members(id)?.wait()?;
    assert_eq!(&after[..2], before.as_slice());
    assert_eq!(after[2].uuid, uuid(3));
    assert!(!after[2].manually_retained);
    assert_eq!(after[2].origins.len(), 1);
    assert_eq!(after[2].origins[0].collection_id, Some(healthy));
    let page = db
        .shelf_page(id, Default::default(), "fr-FR".into())?
        .wait()?;
    assert!(page.members[0].story.is_some());
    assert!(page.members[1].story.is_none());
    db.close()?;
    drop(db);
    let mut db = Database::open_isolated(&library)?;
    assert_eq!(db.shelf_members(id)?.wait()?, after);
    let plan = db.shelf_update_preview(id, "fr-FR".into())?.wait()?;
    assert!(plan.preview().incomplete);
    assert!(!plan.preview().up_to_date);
    assert_eq!(plan.preview().removal_count(), 0);
    assert_eq!(plan.preview().addition_count(), 0);
    assert_eq!(
        db.read(move |tx| Ok(tx.query_row(
            "SELECT validity_state FROM collections WHERE id=?",
            [invalid.get()],
            |r| r.get::<_, String>(0)
        )?))?
        .wait()?,
        "needs_attention"
    );
    db.close()?;
    Ok(())
}

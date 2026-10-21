#![cfg(unix)]
use librairii_core::{
    collection_codec::{self, BooleanFilter, ChoiceFilter, Query},
    query::QueryRequest,
};
use librairii_storage::{
    IsolatedLibrary, StorageError,
    database::{Database, TransactionOutcome, WorkCancellation},
    query,
};
use librairii_testkit::TestRoots;
use rusqlite::params;
use std::error::Error;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;
fn library(roots: &TestRoots) -> TestResult<IsolatedLibrary> {
    Ok(IsolatedLibrary::initialize_from_override(
        Some(roots.library().as_os_str()),
        "a0000000-0000-4000-8000-000000000001".parse()?,
    )?)
}
fn committed<T>(outcome: TransactionOutcome<T>) -> TestResult<T> {
    match outcome {
        TransactionOutcome::Committed(v) => Ok(v),
        TransactionOutcome::ProvenNotCommitted(e) | TransactionOutcome::CommitUncertain(e) => {
            Err(e.into())
        }
    }
}
fn seed(db: &Database<'_>) -> TestResult {
    committed(db.transaction(WorkCancellation::default(),|tx| {
        for (id,title,normalized,state,broken) in [(1,"100% pure","100% pure","valid",true),(2,"under_score","under_score","missing",false),(3,"a\\b","a\\b","invalid",false),(4,"Été","ete","valid",true)] {
            tx.execute("INSERT INTO stories(id,uuid,embedded_title,display_name_normalized,is_broken,created_at) VALUES(?1,printf('10000000-0000-4000-8000-%012d',?1),?2,?3,?4,'2026-01-01 00:00:00')",params![id,title,normalized,broken])?;
            tx.execute("INSERT INTO story_archives(story_id,original_filename,detected_format,sha256,byte_size,managed_path,validation_state) VALUES(?1,'story.zip','zip',printf('%064d',?1),1,'archives/'||?1,?2)",params![id,state])?;
        }
        tx.execute_batch("INSERT INTO tag_definitions(id,key,normalized_key,label,color,kind,source,position,is_protected) VALUES(10,'flag','flag','Flag','#405CF5','boolean','user',0,0),(11,'color','color','Color','#405CF5','choice','user',1,0),(12,'age','age','Age','#405CF5','choice','derived',0,1); INSERT INTO tag_values(id,definition_id,key,normalized_key,label,position) VALUES(20,11,'blue','blue','Blue',0),(21,12,'young','young','Young',0); INSERT INTO story_tag_assignments(story_id,definition_id,value_id,source) VALUES(1,10,NULL,'manual'),(4,11,20,'manual'),(4,12,21,'derived');")?;
        Ok(())
    })?.wait()?)?;
    Ok(())
}
fn named(name: &str) -> QueryRequest {
    let mut q = Query::default();
    q.name = name.into();
    QueryRequest {
        criteria: q,
        ..QueryRequest::default()
    }
}
fn ids(page: &librairii_core::browsing::Page) -> Vec<i64> {
    page.stories.iter().map(|s| s.id).collect()
}

#[test]
fn literal_unicode_search_pages_sort_ties_and_huge_page_numbers() -> TestResult {
    let roots = TestRoots::new()?;
    let library = library(&roots)?;
    let mut db = Database::open_isolated(&library)?;
    seed(&db)?;
    for (name, expected) in [
        ("%", vec![1]),
        ("_", vec![2]),
        ("\\", vec![3]),
        ("ETE", vec![4]),
        ("' OR 1=1--", vec![]),
    ] {
        let page = db.search(named(name), "fr-FR".into())?.wait()?;
        assert_eq!(ids(&page), expected, "{name}");
        assert_eq!(page.total_items, expected.len() as i64);
    }
    let mut request = QueryRequest {
        page: 2,
        page_size: 2,
        ..QueryRequest::default()
    };
    let page = db.search(request.clone(), "en-GB".into())?.wait()?;
    assert_eq!(ids(&page), vec![4, 2]);
    assert_eq!(page.total_pages, 2);
    assert!(page.stories[0].broken);
    request.page = i64::MAX;
    assert!(
        db.search(request.clone(), "en-GB".into())?
            .wait()?
            .stories
            .is_empty()
    );
    assert_eq!(db.count(request, "en-GB".into())?.wait()?, 4);
    let page = db
        .search(
            QueryRequest {
                sort: "imported_desc".into(),
                ..QueryRequest::default()
            },
            "en-GB".into(),
        )?
        .wait()?;
    assert_eq!(ids(&page), vec![4, 3, 2, 1]);
    db.close()?;
    Ok(())
}

#[test]
fn boolean_choice_derived_compatibility_and_live_ownership_filters() -> TestResult {
    let roots = TestRoots::new()?;
    let library = library(&roots)?;
    let mut db = Database::open_isolated(&library)?;
    seed(&db)?;
    for (payload, expected) in [
        (
            r#"{"booleanFilters":[{"definitionId":10,"state":"false"}]}"#,
            vec![3, 4, 2],
        ),
        (
            r#"{"choiceFilters":[{"definitionId":12,"valueIds":[21]}]}"#,
            vec![4],
        ),
        (r#"{"compatibilities":["missing","invalid"]}"#, vec![3, 2]),
    ] {
        let criteria = collection_codec::decode(5, payload.as_bytes(), 0)?;
        assert_eq!(
            ids(&db
                .search(
                    QueryRequest {
                        criteria,
                        ..QueryRequest::default()
                    },
                    "en-GB".into()
                )?
                .wait()?),
            expected
        );
    }
    let mut boolean = Query::default();
    boolean.boolean_filters.push(BooleanFilter {
        definition_id: 99,
        state: "ignored".into(),
    });
    let mut choice = Query::default();
    choice.choice_filters.push(ChoiceFilter {
        definition_id: 11,
        value_ids: vec![21],
    });
    for criteria in [boolean, choice] {
        assert!(matches!(
            db.count(
                QueryRequest {
                    criteria,
                    ..QueryRequest::default()
                },
                "en-GB".into()
            )?
            .wait(),
            Err(StorageError::QueryInput(_))
        ));
    }
    db.close()?;
    Ok(())
}

#[test]
fn metadata_language_is_independent_of_display_locale_and_uses_active_snapshots() -> TestResult {
    let roots = TestRoots::new()?;
    let library = library(&roots)?;
    let mut db = Database::open_isolated(&library)?;
    seed(&db)?;
    committed(db.transaction(WorkCancellation::default(),|tx| {
        tx.execute_batch("INSERT INTO catalog_syncs(id,locale,status,started_at) VALUES('10000000-0000-4000-8000-000000000001','fr-FR','running','2000'); INSERT INTO catalog_snapshots(id,sync_id,locale,raw_path,raw_sha256,byte_size,record_count,fetched_at) VALUES(1,'10000000-0000-4000-8000-000000000001','fr-FR','catalog/one',printf('%064d',1),1,1,'2000'); INSERT INTO official_story_metadata(snapshot_id,story_uuid,locale,title,title_normalized,language,provenance) VALUES(1,'10000000-0000-4000-8000-000000000004','fr-FR','Le Soleil','le soleil','de-DE','lunii_catalog'); UPDATE catalog_snapshots SET status='active',activated_at='2001' WHERE id=1;")?;Ok(())
    })?.wait()?)?;
    assert_eq!(db.count(named("soleil"), "fr-FR".into())?.wait()?, 1);
    assert_eq!(db.count(named("soleil"), "en-GB".into())?.wait()?, 0);
    let criteria = collection_codec::decode(5, br#"{"languages":["de-DE"]}"#, 0)?;
    assert_eq!(
        db.count(
            QueryRequest {
                criteria,
                ..QueryRequest::default()
            },
            "en-GB".into()
        )?
        .wait()?,
        1
    );
    let page = db.search(named("soleil"), "fr-FR".into())?.wait()?;
    assert_eq!(page.stories[0].title, "Le Soleil");
    assert_eq!(page.stories[0].sources.title, "official");
    db.close()?;
    Ok(())
}

#[test]
fn filter_plans_use_preserved_assignment_indexes() -> TestResult {
    let roots = TestRoots::new()?;
    let library = library(&roots)?;
    let mut db = Database::open_isolated(&library)?;
    seed(&db)?;
    let plan = db
        .read(|tx| {
            let mut criteria = Query::default();
            criteria.boolean_filters.push(BooleanFilter {
                definition_id: 10,
                state: "false".into(),
            });
            criteria.choice_filters.push(ChoiceFilter {
                definition_id: 11,
                value_ids: vec![20],
            });
            Ok(query::explain_search(
                tx,
                QueryRequest {
                    criteria,
                    ..QueryRequest::default()
                },
                "en-GB",
            )?
            .join("\n"))
        })?
        .wait()?;
    assert!(
        plan.contains("idx_story_tag_boolean_assignment")
            || plan.contains("idx_story_tag_assignments_filter")
    );
    assert!(plan.contains("idx_story_tag_choice_assignment"));
    db.close()?;
    Ok(())
}

#[test]
fn nested_any_of_is_and_with_local_predicates_and_empty_branch_matches_all() -> TestResult {
    let roots = TestRoots::new()?;
    let library = library(&roots)?;
    let mut db = Database::open_isolated(&library)?;
    seed(&db)?;
    let criteria = collection_codec::decode(5, br#"{"compatibilities":["valid"]}"#, 0);
    assert!(criteria.is_err());
    let mut criteria = Query::default();
    criteria.any_of = vec![named("ete").criteria, Query::default()];
    let request = QueryRequest {
        criteria,
        ..QueryRequest::default()
    };
    assert_eq!(db.count(request.clone(), "en-GB".into())?.wait()?, 4);
    assert_eq!(
        db.read(move |tx| query::count(tx, request, "en-GB"))?
            .wait()?,
        4
    );
    db.close()?;
    Ok(())
}

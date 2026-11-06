use super::*;
use crate::{
    imports::PreparedImport,
    workflows::{exports, operations},
};
use librairii_app::metadata::RefreshSession;
use librairii_storage::{IsolatedLibrary, metadata::MetadataStorage};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

#[test]
fn export_commands_retain_import_and_catalog_effects_consumed_by_forced_polls()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    let roots = librairii_testkit::TestRoots::new()?;
    let id = "a0000000-0000-4000-8000-000000000066";
    let library =
        IsolatedLibrary::initialize_from_override(Some(roots.library().as_os_str()), id.parse()?)?;
    let navigation = Navigation::new(
        id.parse()?,
        1,
        Location {
            scope: Scope::Library,
            request: QueryRequest::default(),
        },
    )?;
    let mut session = LibrarySession::isolated(library);
    session.drive_override(&WorkCancellation::default(), |runtime| {
        let mut operations = operations::Controller::default();
        let mut exports = exports::Controller::default();
        let generation = runtime
            .generation()
            .map_err(|_| StorageError::RuntimeFenced)?;
        let (collection, shelf) = runtime.command(|db, _, _| {
            use librairii_storage::{collections, database::TransactionOutcome, shelves};
            let TransactionOutcome::Committed(collections::Response::Collection(collection)) = db
                .edit_collections(
                    collections::Command::Create {
                        name: "All stories".into(),
                        criteria: Default::default(),
                    },
                    Default::default(),
                )?
                .wait()?
            else {
                return Err(StorageError::CompatibilityInvariant);
            };
            let TransactionOutcome::Committed(shelves::Response::Shelf(shelf)) = db
                .edit_shelves(
                    shelves::Command::Create {
                        name: "Approved stories".into(),
                    },
                    Default::default(),
                )?
                .wait()?
            else {
                return Err(StorageError::CompatibilityInvariant);
            };
            Ok((collection.id, shelf.id))
        })?;
        let mut group_rows = groups(runtime)?;
        let mut query_facets = facets(runtime)?;
        let mut workflows = crate::workflows::Snapshot::default();
        let mut collections = crate::workflows::collections::Controller::default();
        collections.handle(
            runtime,
            generation.clone(),
            crate::workflows::collections::Action::PreviewUnion(vec![collection]),
            1,
            &mut navigation.clone(),
        );
        assert_eq!(
            collections.snapshot.union.as_ref().map(|u| u.unique),
            Some(0)
        );
        let mut reviews = crate::workflows::shelf_reviews::Controller::default();
        reviews.handle(
            runtime,
            &crate::preferences::Memory::default(),
            crate::workflows::shelf_reviews::Request {
                generation: generation.clone(),
                action: crate::workflows::shelf_reviews::Action::Review(shelf),
            },
            1,
            &mut navigation.clone(),
        );
        assert!(reviews.snapshot.preview.is_some());
        assert!(!reviews.snapshot.stale);
        assert!(!operations.poll(runtime, true)?.1);
        let operation = "a0000000-0000-4000-8000-000000000067";
        let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../preservation/corpus/archives/clockwork-forest-studio.zip");
        let import = PreparedImport::new(
            operation
                .parse()
                .map_err(|_| StorageError::InvalidIdentity)?,
            vec![source],
        )
        .map_err(|_| StorageError::CompatibilityInvariant)?;
        runtime
            .offer_import(import)
            .map_err(|_| StorageError::QueueFull)?;
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let succeeded = runtime.command(|db, _, _| {
                Ok(db
                    .operation_snapshot(operation.into())?
                    .wait()?
                    .is_some_and(|s| {
                        s.operation.state == librairii_core::OperationState::Succeeded
                    }))
            })?;
            if succeeded {
                break;
            }
            if Instant::now() >= deadline {
                return Err(StorageError::RuntimeFenced);
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        let effects = export_command(
            runtime,
            &navigation,
            &mut operations,
            &mut exports,
            runtime
                .generation()
                .map_err(|_| StorageError::RuntimeFenced)?,
            exports::Action::Open(Vec::new()),
            1,
        )?;
        assert!(effects, "export command swallowed the completed import");
        if effects {
            refresh_after_operations(
                runtime,
                &navigation,
                &mut workflows,
                &mut group_rows,
                &mut query_facets,
                &mut collections,
                &mut reviews,
            )?;
        }
        assert!(group_rows.iter().any(
            |g| matches!(g.scope, Scope::Collection(id) if id.get() == collection) && g.count == 1
        ));
        assert_eq!(
            workflows.collections.union.as_ref().map(|u| u.unique),
            Some(1)
        );
        assert!(workflows.shelf_reviews.stale);
        assert!(
            !operations.poll(runtime, true)?.1,
            "import effects should already be consumed"
        );
        let imported = runtime.command(|db, _, _| {
            db.search(Default::default(), "en-GB".into())?
                .wait()?
                .stories
                .into_iter()
                .next()
                .ok_or(StorageError::SnapshotChanged)
        })?;
        workflows.details = Some(crate::workflows::inspect(
            runtime,
            &navigation,
            crate::workflows::StoryTarget {
                generation,
                query: navigation.stamp(),
                id: imported.id,
                uuid: imported.uuid,
            },
        )?);
        assert_eq!(
            workflows
                .details
                .as_ref()
                .map(|d| d.story.summary.title.as_str()),
            Some("The Clockwork Forest")
        );
        assert!(query_facets.languages.is_empty());
        runtime.command(|db, _, _| {
            let store = MetadataStorage::new(db);
            let payload = include_str!("../../../librairii-app/tests/fixtures/lunii-catalog.json")
                .replace(
                    "123e4567-e89b-42d3-a456-426614174000",
                    "00112233-4455-4677-8899-aabbccddeeff",
                );
            let job = "a0000000-0000-4000-8000-000000000068"
                .parse()
                .map_err(|_| StorageError::InvalidIdentity)?;
            let refresh = RefreshSession::begin(&store, job, "en-GB", "2026-10-28T12:00:00Z")
                .map_err(|_| StorageError::CompatibilityInvariant)?;
            refresh
                .finish(Ok(payload.into_bytes()), "2026-10-28T12:00:01Z", || false)
                .map_err(|_| StorageError::CompatibilityInvariant)?;
            Ok(())
        })?;
        let effects = export_command(
            runtime,
            &navigation,
            &mut operations,
            &mut exports,
            runtime
                .generation()
                .map_err(|_| StorageError::RuntimeFenced)?,
            exports::Action::Close,
            2,
        )?;
        assert!(effects, "export command swallowed the activated catalog");
        if effects {
            refresh_after_operations(
                runtime,
                &navigation,
                &mut workflows,
                &mut group_rows,
                &mut query_facets,
                &mut collections,
                &mut reviews,
            )?;
        }
        assert_eq!(
            workflows
                .details
                .as_ref()
                .map(|d| d.story.summary.title.as_str()),
            Some("The Clockwork Mountain")
        );
        assert_eq!(
            workflows
                .details
                .as_ref()
                .map(|d| d.story.summary.description.as_str()),
            Some("A sanitized catalog fixture.")
        );
        assert!(!query_facets.languages.is_empty());
        assert!(
            !operations.poll(runtime, true)?.1,
            "catalog effects should already be consumed"
        );
        Ok(Action::finish(()))
    })?;
    session.finish()?;
    Ok(())
}

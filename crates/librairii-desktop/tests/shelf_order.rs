#![cfg(any(target_os = "macos", target_os = "linux"))]
use librairii_app::localization::Language;
use librairii_core::{
    StoryUuid,
    collection_codec::url::{Location, Scope},
    navigation::Navigation,
    query::QueryRequest,
};
use librairii_desktop::{
    library::{Action as SessionAction, LibrarySession},
    workflows::shelves::{Action, Controller, Feedback},
};
use librairii_storage::{
    Result, StorageError,
    database::{Database, TransactionOutcome, WorkCancellation},
    native::NativeLibrary,
    profile::{NativeProfileStore, ProfileLocations},
    settings::TargetPreview,
    shelves::{self, Curation},
};
use librairii_testkit::TestRoots;
use std::fs;

fn committed<T>(outcome: TransactionOutcome<T>) -> Result<T> {
    match outcome {
        TransactionOutcome::Committed(value) => Ok(value),
        TransactionOutcome::ProvenNotCommitted(error)
        | TransactionOutcome::CommitUncertain(error) => Err(error),
    }
}

#[test]
fn controller_reorders_beyond_one_hundred_gapped_members_across_pages_and_reopen()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    let roots = TestRoots::new()?;
    let library_id = "a0000000-0000-4000-8000-000000000029".parse()?;
    let locations = ProfileLocations::for_host(
        &roots.profile().join("home"),
        Some(&roots.profile().join("config")),
        Some(&roots.library().join("data")),
    )?;
    fs::create_dir_all(&locations.data)?;
    fs::create_dir_all(locations.native.parent().ok_or("profile parent")?)?;
    let profile = NativeProfileStore::create_fresh(&locations.native)?;
    let library = NativeLibrary::initialize_confirmed(
        TargetPreview::inspect(&roots.library().join("native"), &[], &Default::default())?,
        &profile,
        &locations,
        library_id,
        Language::French,
        &Default::default(),
    )
    .map_err(|e| e.to_string())?;
    let mut session = LibrarySession::new(library);
    let (shelf, original) = session.drive(&locations, &WorkCancellation::default(), |runtime| {
        let shelf = runtime.command(|db, _, _| {
            committed(db.transaction(Default::default(), |tx| {
                for id in 1..=145 {
                    tx.execute("INSERT INTO stories(id,uuid,display_name_normalized) VALUES(?1,printf('10000000-0000-4000-8000-%012d',?1),'story')", [id])?;
                    tx.execute("INSERT INTO story_archives(story_id,original_filename,detected_format,sha256,byte_size,managed_path,validation_state) VALUES(?1,'story.zip','zip',printf('%064d',?1),1,'archives/'||?1,'valid')", [id])?;
                }
                Ok(())
            })?.wait()?)?;
            let shelves::Response::Shelf(shelf) = committed(db.edit_shelves(
                shelves::Command::Create { name: "More than one hundred".into() },
                Default::default(),
            )?.wait()?)? else {
                return Err(StorageError::CompatibilityInvariant);
            };
            let members = (1..=145).map(|id| format!("10000000-0000-4000-8000-{id:012}").parse::<StoryUuid>().map_err(|_| StorageError::InvalidIdentity)).collect::<Result<Vec<_>>>()?;
            committed(db.curate_shelf(shelf.id, Curation::Add(members), "fr-FR".into(), Default::default())?.wait()?)?;
            Ok(shelf.id)
        })?;
        let order = || runtime.command(|db, _, _| Ok(db.shelf_members(shelf)?.wait()?.into_iter().map(|m| m.uuid).collect::<Vec<_>>()));
        let original = order()?;
        assert_eq!(original.len(), 145);
        let generation = runtime.generation().map_err(|_| StorageError::RuntimeFenced)?;
        let mut navigation = Navigation::new(library_id, 1, Location {
            scope: Scope::Shelf { id: shelf, manual_order: true },
            request: QueryRequest { page: 5, page_size: 24, ..Default::default() },
        }).map_err(|_| StorageError::InvalidNavigation)?;
        let mut controller = Controller::default();
        for index in [100, 120] {
            runtime.command(|db, _, _| committed(db.transaction(Default::default(), move |tx| {
                tx.execute("UPDATE shelf_stories SET position=position+1000000 WHERE shelf_id=?", [shelf.get()])?;
                tx.execute("UPDATE shelf_stories SET position=(position-1000000)*7 WHERE shelf_id=?", [shelf.get()])?;
                Ok(())
            })?.wait()?))?;
            let mut frame = navigation.current().clone();
            frame.location.request.page = (index / 24 + 1) as i64;
            frame.selection = vec![(index + 1) as i64];
            navigation.replace(frame).map_err(|_| StorageError::InvalidNavigation)?;
            let selected = original[index].clone();
            controller.handle(runtime, generation.clone(), Action::MoveMember {
                id: shelf, uuid: selected.clone(), offset: -1,
            }, 1, &mut navigation);
            assert!(matches!(controller.snapshot.feedback, Some(Feedback::Saved(id)) if id == shelf));
            let mut expected = original.clone();
            expected.swap(index, index - 1);
            assert_eq!(order()?, expected, "move must retain all 145 UUIDs in exact order");
            assert_eq!(controller.snapshot.focus_member.as_ref(), Some(&selected));
            assert_eq!(navigation.current().location.request.page, ((index - 1) / 24 + 1) as i64);
            assert_eq!(navigation.current().selection, vec![(index + 1) as i64]);
            controller.handle(runtime, generation.clone(), Action::MoveMember {
                id: shelf, uuid: selected.clone(), offset: 1,
            }, 2, &mut navigation);
            assert!(matches!(controller.snapshot.feedback, Some(Feedback::Saved(id)) if id == shelf));
            assert_eq!(order()?, original);
            assert_eq!(controller.snapshot.focus_member.as_ref(), Some(&selected));
            assert_eq!(navigation.current().location.request.page, (index / 24 + 1) as i64);
        }
        // A temporary refinement cannot edit stored manual membership order.
        let mut frame = navigation.current().clone();
        frame.location.request.criteria.name = "story".into();
        navigation.replace(frame).map_err(|_| StorageError::InvalidNavigation)?;
        controller.handle(runtime, generation, Action::MoveMember {
            id: shelf, uuid: original[120].clone(), offset: -1,
        }, 3, &mut navigation);
        assert!(matches!(controller.snapshot.feedback, Some(Feedback::Error { .. })));
        assert_eq!(order()?, original);
        Ok(SessionAction::finish((shelf, original)))
    }).map_err(|e| e.to_string())?;
    session.finish().map_err(|e| e.to_string())?;
    let library = NativeLibrary::reopen(&profile, &locations, &Default::default())?;
    let mut database = Database::open_native(&library)?;
    let reopened = database.shelf_members(shelf)?.wait()?;
    assert_eq!(
        reopened.into_iter().map(|m| m.uuid).collect::<Vec<_>>(),
        original
    );
    database.close()?;
    drop(database);
    library.finish().map_err(|e| e.to_string())?;
    Ok(())
}

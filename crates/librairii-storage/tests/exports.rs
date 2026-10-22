#![cfg(unix)]
use librairii_core::{
    CollectionId, ShelfId, collection_codec::Query, export::Scope, query::QueryRequest,
};
use librairii_storage::{
    IsolatedLibrary,
    database::{Database, TransactionOutcome, WorkCancellation},
};
use librairii_testkit::TestRoots;
use std::error::Error;
type TestResult<T = ()> = Result<T, Box<dyn Error>>;
#[test]
fn unicode_suffix_and_unsafe_sources_keep_per_item_preflight_reasons() -> TestResult {
    use librairii_core::export::{Disposition, Issue};
    let roots = TestRoots::new()?;
    let library = IsolatedLibrary::initialize_from_override(
        Some(roots.library().as_os_str()),
        "a0000000-0000-4000-8000-000000000001".parse()?,
    )?;
    let mut db = Database::open_isolated(&library)?;
    seed(&db, 3)?;
    files(&db, &library, 3)?;
    committed(db.transaction(WorkCancellation::default(),|tx|{
        tx.execute_batch("UPDATE stories SET is_broken=0; UPDATE story_archives SET original_filename='story.ZİP' WHERE story_id=3;")?;Ok(())
    })?.wait()?)?;
    let destination = tempfile::tempdir()?;
    let cancel = WorkCancellation::default();
    let mut exports = librairii_storage::export::Exports::new(&db);
    let unicode = exports.prepare(
        Scope::Selection(vec![3]),
        destination.path(),
        "fr-FR".into(),
        &cancel,
    )?;
    assert_eq!(unicode.items[0].disposition, Disposition::Ready);
    std::fs::remove_file(library.layout().archives().join("1"))?;
    std::fs::write(destination.path().join("outside"), b"1")?;
    std::os::unix::fs::symlink(
        destination.path().join("outside"),
        library.layout().archives().join("1"),
    )?;
    std::fs::create_dir(library.layout().archives().join("real"))?;
    std::fs::rename(
        library.layout().archives().join("2"),
        library.layout().archives().join("real/two"),
    )?;
    std::os::unix::fs::symlink("real", library.layout().archives().join("link"))?;
    committed(
        db.transaction(WorkCancellation::default(), |tx| {
            tx.execute(
                "UPDATE story_archives SET managed_path='archives/link/two' WHERE story_id=2",
                [],
            )?;
            Ok(())
        })?
        .wait()?,
    )?;
    let preview = exports.prepare(
        Scope::Selection(vec![1, 2, 3]),
        destination.path(),
        "fr-FR".into(),
        &cancel,
    )?;
    assert_eq!(
        preview
            .items
            .iter()
            .map(|i| i.disposition)
            .collect::<Vec<_>>(),
        [
            Disposition::Skipped(Issue::ArchiveMissing),
            Disposition::Skipped(Issue::ArchiveMissing),
            Disposition::Ready
        ]
    );
    assert_eq!(preview.ready, 1);
    assert_eq!(std::fs::read(destination.path().join("outside"))?, b"1");
    drop(exports);
    db.close()?;
    Ok(())
}
fn files(db: &Database<'_>, library: &IsolatedLibrary, count: i64) -> TestResult {
    use sha2::{Digest, Sha256};
    let mut hashes = Vec::new();
    for id in 1..=count {
        let bytes = id.to_string();
        std::fs::write(library.layout().archives().join(&bytes), bytes.as_bytes())?;
        hashes.push((
            id,
            librairii_core::Checksum::from_bytes(Sha256::digest(bytes.as_bytes()).into())
                .to_string(),
        ));
    }
    committed(
        db.transaction(WorkCancellation::default(), move |tx| {
            for (id, hash) in hashes {
                tx.execute(
                    "UPDATE story_archives SET sha256=? WHERE story_id=?",
                    rusqlite::params![hash, id],
                )?;
            }
            Ok(())
        })?
        .wait()?,
    )?;
    Ok(())
}
fn committed<T>(v: TransactionOutcome<T>) -> TestResult<T> {
    match v {
        TransactionOutcome::Committed(v) => Ok(v),
        TransactionOutcome::ProvenNotCommitted(e) | TransactionOutcome::CommitUncertain(e) => {
            Err(e.into())
        }
    }
}
fn seed(db: &Database<'_>, count: i64) -> TestResult {
    committed(db.transaction(WorkCancellation::default(),move|tx|{
        for id in 1..=count {
            tx.execute("INSERT INTO stories(id,uuid,embedded_title,display_name_normalized,is_broken) VALUES(?1,printf('10000000-0000-4000-8000-%012d',?1),'Story'||?1,printf('%08d',?1),?1=2)",[id])?;
            tx.execute("INSERT INTO story_archives(story_id,original_filename,detected_format,sha256,byte_size,managed_path,validation_state) VALUES(?1,?1||'.zip','zip',printf('%064d',?1),1,'archives/'||?1,'valid')",[id])?;
        }
        tx.execute_batch("INSERT INTO shelves(id,name,normalized_name,position) VALUES(1,'Approved','approved',0),(2,'Second','second',1);
            INSERT INTO shelf_stories(shelf_id,story_uuid,position,protected,cached_title) VALUES
            (1,'10000000-0000-4000-8000-000000000002',0,0,'Two'),
            (1,'10000000-0000-4000-8000-000000009999',1,0,'Unavailable'),
            (1,'10000000-0000-4000-8000-000000000001',2,0,'One'),
            (2,'10000000-0000-4000-8000-000000000001',0,0,'One');")?;
        Ok(())
    })?.wait()?)?;
    Ok(())
}
#[test]
fn full_query_selection_collection_and_approved_shelf_unions_keep_identity_order_and_overlap()
-> TestResult {
    let roots = TestRoots::new()?;
    let library = IsolatedLibrary::initialize_from_override(
        Some(roots.library().as_os_str()),
        "a0000000-0000-4000-8000-000000000001".parse()?,
    )?;
    let mut db = Database::open_isolated(&library)?;
    seed(&db, 1001)?;
    let all = db
        .resolve_export(
            Scope::CurrentQuery(QueryRequest {
                page: 99,
                page_size: 1,
                ..Default::default()
            }),
            "fr-FR".into(),
        )?
        .wait()?;
    assert_eq!(all.stories.len(), 1001);
    let selection = db
        .resolve_export(Scope::Selection(vec![2, 1, 2]), "fr-FR".into())?
        .wait()?;
    assert_eq!(
        selection.stories.iter().map(|s| s.id).collect::<Vec<_>>(),
        [2, 1]
    );
    assert!(selection.stories[0].broken);
    let shelves = db
        .resolve_export(
            Scope::Shelves(vec![ShelfId::new(1)?, ShelfId::new(2)?]),
            "fr-FR".into(),
        )?
        .wait()?;
    assert_eq!(
        shelves.stories.iter().map(|s| s.id).collect::<Vec<_>>(),
        [2, 0, 1]
    );
    assert_eq!(shelves.stories[1].title, "Unavailable");
    assert_eq!(shelves.collapsed_overlap, 1);
    let selected = db
        .resolve_export(
            Scope::ShelfSelection {
                shelf: ShelfId::new(1)?,
                uuids: vec![shelves.stories[1].uuid.clone()],
            },
            "fr-FR".into(),
        )?
        .wait()?;
    assert_eq!(selected.stories[0].id, 0);
    assert!(
        db.resolve_export(
            Scope::ShelfSelection {
                shelf: ShelfId::new(1)?,
                uuids: vec!["unknown".into()]
            },
            "fr-FR".into()
        )?
        .wait()
        .is_err()
    );
    let mut filter = Query::default();
    filter.name = "00000001".into();
    let refined = db
        .resolve_export(
            Scope::ShelfResult {
                shelf: ShelfId::new(1)?,
                query: QueryRequest {
                    criteria: filter,
                    ..Default::default()
                },
            },
            "fr-FR".into(),
        )?
        .wait()?;
    assert_eq!(
        refined.source.kind,
        librairii_core::ExportScopeKind::RefinedShelf
    );
    assert_eq!(refined.source.ids, vec![1]);
    assert_eq!(refined.source.names, vec![b"Approved".to_vec()]);
    assert_eq!(
        refined.stories.iter().map(|s| s.id).collect::<Vec<_>>(),
        [1]
    );
    for name in ["First", "Second"] {
        committed(
            db.edit_collections(
                librairii_storage::collections::Command::Create {
                    name: name.into(),
                    criteria: Query::default(),
                },
                WorkCancellation::default(),
            )?
            .wait()?,
        )?;
    }
    let union = db
        .resolve_export(
            Scope::Collections(vec![CollectionId::new(1)?, CollectionId::new(2)?]),
            "fr-FR".into(),
        )?
        .wait()?;
    assert_eq!(union.stories.len(), 1001);
    assert_eq!(union.collapsed_overlap, 1001);
    assert!(
        db.resolve_export(
            Scope::Collections(vec![CollectionId::new(1)?, CollectionId::new(1)?]),
            "fr-FR".into()
        )?
        .wait()
        .is_err()
    );
    db.close()?;
    Ok(())
}

#[test]
fn preflight_verifies_bytes_and_names_without_reserving_skipped_output_names() -> TestResult {
    use librairii_core::export::{Disposition, Issue};
    use sha2::{Digest, Sha256};
    let roots = TestRoots::new()?;
    let library = IsolatedLibrary::initialize_from_override(
        Some(roots.library().as_os_str()),
        "a0000000-0000-4000-8000-000000000001".parse()?,
    )?;
    let mut db = Database::open_isolated(&library)?;
    seed(&db, 5)?;
    for id in 1..=5 {
        std::fs::write(
            library.layout().archives().join(id.to_string()),
            id.to_string().as_bytes(),
        )?;
    }
    committed(db.transaction(WorkCancellation::default(),move|tx|{
        for id in 1..=5 {
            let digest=librairii_core::Checksum::from_bytes(Sha256::digest(id.to_string().as_bytes()).into()).to_string();
            tx.execute("UPDATE story_archives SET sha256=? WHERE story_id=?",rusqlite::params![digest,id])?;
        }
        tx.execute_batch("UPDATE story_archives SET original_filename='same.zip' WHERE story_id IN (1,2,3); UPDATE story_archives SET original_filename='bad.pk' WHERE story_id=4; UPDATE story_archives SET sha256=printf('%064d',0) WHERE story_id=5;")?;Ok(())
    })?.wait()?)?;
    let destination = tempfile::tempdir()?;
    let mut exports = librairii_storage::export::Exports::new(&db);
    let report = exports.prepare(
        Scope::Selection(vec![2, 1, 3, 4, 5]),
        destination.path(),
        "fr-FR".into(),
        &WorkCancellation::default(),
    )?;
    assert_eq!(
        report
            .items
            .iter()
            .map(|i| i.disposition)
            .collect::<Vec<_>>(),
        [
            Disposition::Skipped(Issue::StoryMarkedBroken),
            Disposition::Ready,
            Disposition::Conflicted,
            Disposition::Skipped(Issue::UnsupportedExtension),
            Disposition::Skipped(Issue::ArchiveChanged)
        ]
    );
    assert_eq!(report.ready, 1);
    assert_eq!(report.bytes, 1);
    assert!(report.can_export());
    assert!(report.preparation.is_some());
    assert_eq!(std::fs::read_dir(destination.path())?.count(), 0);
    std::fs::create_dir(library.layout().archives().join("nested"))?;
    std::fs::rename(
        library.layout().archives().join("1"),
        library.layout().archives().join("nested/one"),
    )?;
    committed(db.transaction(WorkCancellation::default(),|tx|{
        tx.execute_batch("UPDATE story_archives SET managed_path='archives/nested/one',original_filename='İ.ZIP' WHERE story_id=1; UPDATE story_archives SET original_filename='i.zip' WHERE story_id=3;")?;Ok(())
    })?.wait()?)?;
    let unicode = exports.prepare(
        Scope::Selection(vec![1, 3]),
        destination.path(),
        "fr-FR".into(),
        &WorkCancellation::default(),
    )?;
    assert_eq!(unicode.items[0].output_name, "İ.ZIP");
    assert_eq!(unicode.items[0].disposition, Disposition::Ready);
    assert_eq!(unicode.items[1].disposition, Disposition::Conflicted);
    committed(
        db.transaction(WorkCancellation::default(), |tx| {
            tx.execute(
                "UPDATE story_archives SET original_filename='same.zip' WHERE story_id=1",
                [],
            )?;
            Ok(())
        })?
        .wait()?,
    )?;
    std::fs::write(destination.path().join("same.zip"), b"existing")?;
    let conflict = exports.prepare(
        Scope::Selection(vec![1]),
        destination.path(),
        "fr-FR".into(),
        &WorkCancellation::default(),
    )?;
    assert_eq!(conflict.items[0].disposition, Disposition::Conflicted);
    assert!(!conflict.can_export());
    assert!(conflict.preparation.is_none());
    assert!(
        exports
            .prepare(
                Scope::Selection(vec![1]),
                &library.layout().archives(),
                "fr-FR".into(),
                &WorkCancellation::default()
            )
            .is_err()
    );
    let cancel = WorkCancellation::default();
    cancel.cancel();
    assert!(
        exports
            .prepare(
                Scope::Selection(vec![1]),
                destination.path(),
                "fr-FR".into(),
                &cancel
            )
            .is_err()
    );
    drop(exports);
    db.close()?;
    Ok(())
}

#[test]
fn prepared_ids_evict_consume_invalidate_and_recheck_each_queued_eligibility_boundary() -> TestResult
{
    use librairii_core::export::{Disposition, Issue, PreparationId};
    let roots = TestRoots::new()?;
    let library = IsolatedLibrary::initialize_from_override(
        Some(roots.library().as_os_str()),
        "a0000000-0000-4000-8000-000000000001".parse()?,
    )?;
    let mut db = Database::open_isolated(&library)?;
    seed(&db, 3)?;
    files(&db, &library, 3)?;
    let destination = tempfile::tempdir()?;
    let cancel = WorkCancellation::default();
    let mut exports = librairii_storage::export::Exports::new(&db);
    let mut ids = Vec::new();
    for _ in 0..9 {
        ids.push(
            exports
                .prepare(
                    Scope::Selection(vec![1, 2, 3]),
                    destination.path(),
                    "fr-FR".into(),
                    &cancel,
                )?
                .preparation
                .ok_or("id")?,
        );
    }
    assert_eq!(exports.preparation_count(), 8);
    assert!(exports.preview(&ids[0]).is_err());
    assert!(exports.start(&PreparationId::fresh(), &cancel).is_err());
    let mut foreign = librairii_storage::export::Exports::new(&db);
    assert!(foreign.start(&ids[8], &cancel).is_err());
    committed(
        db.transaction(WorkCancellation::default(), |tx| {
            tx.execute("UPDATE stories SET is_broken=1 WHERE id=1", [])?;
            Ok(())
        })?
        .wait()?,
    )?;
    assert!(exports.start(&ids[8], &cancel).is_err());
    assert!(exports.preview(&ids[8]).is_err());
    let fresh = exports
        .prepare(
            Scope::Selection(vec![1, 2, 3]),
            destination.path(),
            "fr-FR".into(),
            &cancel,
        )?
        .preparation
        .ok_or("id")?;
    let running = exports.start(&fresh, &cancel)?;
    committed(db.transaction(WorkCancellation::default(),|tx|{tx.execute_batch("UPDATE stories SET is_broken=0 WHERE id IN (1,2); UPDATE stories SET is_broken=1 WHERE id=3;")?;Ok(())})?.wait()?)?;
    assert_eq!(
        running.check_item(0)?,
        Disposition::Skipped(Issue::StoryMarkedBroken)
    );
    assert_eq!(
        running.check_item(1)?,
        Disposition::Skipped(Issue::StoryMarkedBroken)
    );
    assert_eq!(
        running.check_item(2)?,
        Disposition::Skipped(Issue::StoryMarkedBroken)
    );
    drop(running);
    assert!(exports.start(&fresh, &cancel).is_err());
    exports.invalidate();
    assert_eq!(exports.preparation_count(), 0);
    assert!(exports.preview(&ids[7]).is_err());
    drop(exports);
    drop(foreign);
    db.close()?;
    Ok(())
}

#[test]
fn passthrough_exports_all_seven_frozen_formats_with_exact_original_names_and_bytes() -> TestResult
{
    use librairii_core::{Checksum, export::CopyStatus};
    use sha2::{Digest, Sha256};
    let fixture_root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../preservation/corpus/archives");
    let cases = [
        ("clockwork-forest.plain.pk", "plain_pk"),
        ("00112233445546778899aabbccddeeff.v1.pk", "v1_pk"),
        ("00112233445546778899aabbccddeeff.v2.pk", "v2_pk"),
        ("clockwork-forest.pk", "generic_pk"),
        ("clockwork-forest.zip", "zip"),
        ("clockwork-forest-studio.zip", "studio_zip"),
        ("generic.7z", "seven_zip"),
    ];
    let destination = tempfile::tempdir()?;
    let cancel = WorkCancellation::default();
    for (name, format) in cases {
        // Some frozen suffix variants intentionally contain identical ZIP bytes;
        // independent libraries preserve the unique-checksum import invariant.
        let roots = TestRoots::new()?;
        let library = IsolatedLibrary::initialize_from_override(
            Some(roots.library().as_os_str()),
            "a0000000-0000-4000-8000-000000000001".parse()?,
        )?;
        let mut db = Database::open_isolated(&library)?;
        let bytes = std::fs::read(fixture_root.join(name))?;
        let hash = Checksum::from_bytes(Sha256::digest(&bytes).into()).to_string();
        let managed = format!("archives/{hash}");
        std::fs::write(library.layout().root().join(&managed), &bytes)?;
        let size = bytes.len() as i64;
        committed(db.transaction(WorkCancellation::default(),move|tx|{
            tx.execute("INSERT INTO stories(id,uuid,embedded_title,display_name_normalized) VALUES(1,'10000000-0000-4000-8000-000000000001','Synthetic','synthetic')",[])?;
            tx.execute("INSERT INTO story_archives(story_id,original_filename,detected_format,sha256,byte_size,managed_path,validation_state) VALUES(1,?,?,?,?,?,'valid')",rusqlite::params![name,format,hash,size,managed])?;Ok(())
        })?.wait()?)?;
        let mut exports = librairii_storage::export::Exports::new(&db);
        let preview = exports.prepare(
            Scope::Selection(vec![1]),
            destination.path(),
            "fr-FR".into(),
            &cancel,
        )?;
        assert_eq!(preview.ready, 1);
        let id = preview.preparation.ok_or("id")?;
        let mut running = exports.start(&id, &cancel)?;
        let mut progress = 0;
        let item = running
            .copy_next(&cancel, |value| progress = value)?
            .ok_or("item")?;
        assert_eq!(item.status, CopyStatus::Exported);
        assert_eq!(item.output_name, name);
        assert_eq!(item.bytes, bytes.len() as u64);
        assert_eq!(progress, bytes.len() as u64);
        assert_eq!(std::fs::read(destination.path().join(name))?, bytes);
        assert_eq!(std::fs::read(fixture_root.join(name))?, bytes);
        assert!(running.copy_next(&cancel, |_| {})?.is_none());
        drop(running);
        assert_eq!(exports.retained_outputs(), 0);
        drop(exports);
        db.close()?;
    }
    assert_eq!(std::fs::read_dir(destination.path())?.count(), 7);
    Ok(())
}

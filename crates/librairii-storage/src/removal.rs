//! Durable removal on a blocking worker. The caller retains the real database
//! and filesystem owners until success or explicit rollback has settled.
use crate::{
    Result, StorageError,
    database::{Database, TransactionOutcome, WorkCancellation},
    platform::unix::{Custody, LibraryFiles, removal::RemovalFile},
};
use librairii_app::ports::operations::OperationPath;
use rusqlite::{OptionalExtension, Transaction, params};
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Scope {
    id: i64,
    uuid: String,
    pub(crate) managed: OperationPath,
    pub(crate) hash: String,
    pub(crate) bytes: i64,
}
#[derive(Clone, Debug, Eq, PartialEq)]
struct Intent {
    id: String,
    scope: Scope,
    trash: OperationPath,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Removed {
    pub story_id: i64,
    pub uuid: String,
}
#[derive(Debug)]
struct PendingRemoval<'files, 'library> {
    intent: Intent,
    file: RemovalFile<'files, 'library>,
    _custody: Custody<'library>,
}
#[derive(Debug)]
pub struct Removals<'db, 'files, 'library> {
    database: &'db Database<'library>,
    files: &'files LibraryFiles<'library>,
    pending: Option<PendingRemoval<'files, 'library>>,
}
impl<'db, 'files, 'library> Removals<'db, 'files, 'library> {
    pub fn new(
        database: &'db Database<'library>,
        files: &'files LibraryFiles<'library>,
    ) -> Result<Self> {
        files.matches_database(database)?;
        Ok(Self {
            database,
            files,
            pending: None,
        })
    }
    pub fn has_unsettled_removal(&self) -> bool {
        self.pending.is_some()
    }
    pub fn remove(&mut self, story: i64, cancel: &WorkCancellation) -> Result<Removed> {
        self.remove_with(story, cancel, |_| Ok(()))
    }
    fn remove_with(
        &mut self,
        story: i64,
        cancel: &WorkCancellation,
        mut checkpoint: impl FnMut(&'static str) -> Result<()>,
    ) -> Result<Removed> {
        if self.pending.is_some() {
            return Err(StorageError::CustodyRecoveryRequired);
        }
        if story <= 0 {
            return Err(StorageError::InvalidManagedPath);
        }
        if cancel.is_cancelled() {
            return Err(StorageError::Cancelled);
        }
        let custody = self.files.try_custody()?;
        let (scope, id) = self
            .database
            .read(move |tx| {
                let scope = scope(tx, story)?.ok_or(StorageError::SnapshotChanged)?;
                let raw: String =
                    tx.query_row("SELECT lower(hex(randomblob(16)))", [], |r| r.get(0))?;
                let id = format!(
                    "{}-{}-{}-{}-{}",
                    &raw[..8],
                    &raw[8..12],
                    &raw[12..16],
                    &raw[16..20],
                    &raw[20..]
                );
                Ok((scope, id))
            })?
            .wait()?;
        let basename = scope
            .managed
            .file_name()
            .ok_or(StorageError::InvalidManagedPath)?;
        let intent = Intent {
            trash: std::path::PathBuf::from(format!("trash/removals/{id}"))
                .join(basename)
                .into(),
            id,
            scope,
        };
        let file = RemovalFile::open(self.files, &custody, &intent.scope, &intent.trash, false)?;
        self.pending = Some(PendingRemoval {
            intent,
            file,
            _custody: custody,
        });
        let result = (|| {
            let intent = self
                .pending
                .as_ref()
                .ok_or(StorageError::CompatibilityInvariant)?
                .intent
                .clone();
            let create = intent.clone();
            committed(self.database.transaction(cancel.clone(),move|tx|{
                require_scope(tx,&create.scope)?;
                tx.execute("INSERT INTO removal_intents(id,story_id,managed_path,trash_path) VALUES(?,?,?,?)",params![create.id,create.scope.id,crate::native_path::SqlPath(&create.scope.managed),crate::native_path::SqlPath(&create.trash)])?; Ok(())
            })?.wait()?)?;
            checkpoint("intent")?;
            if cancel.is_cancelled() {
                return Err(StorageError::Cancelled);
            }
            let pending = self
                .pending
                .as_mut()
                .ok_or(StorageError::CompatibilityInvariant)?;
            pending.file.move_to_trash(&pending._custody)?;
            checkpoint("trash")?;
            if cancel.is_cancelled() {
                return Err(StorageError::Cancelled);
            }
            let delete = intent.clone();
            committed(
                self.database
                    .transaction(cancel.clone(), move |tx| {
                        require_scope(tx, &delete.scope)?;
                        require_intent(tx, &delete)?;
                        if tx.execute("DELETE FROM stories WHERE id=?", [delete.scope.id])? != 1 {
                            return Err(StorageError::SnapshotChanged);
                        }
                        Ok(())
                    })?
                    .wait()?,
            )?;
            checkpoint("deleted")?;
            Ok(())
        })();
        if let Err(cause) = result {
            match self.retry_rollback() {
                Ok(Some(committed)) => return Ok(committed),
                Ok(None) => return Err(cause),
                Err(cleanup) => {
                    return Err(StorageError::OwnedTempCleanupFailed {
                        work: Box::new(cause),
                        cleanup: Box::new(cleanup),
                    });
                }
            }
        }
        let pending = self
            .pending
            .take()
            .ok_or(StorageError::CompatibilityInvariant)?;
        Ok(Removed {
            story_id: pending.intent.scope.id,
            uuid: pending.intent.scope.uuid,
        })
    }
    /// Uncancelled repair stays available after ordinary intake stops. If the
    /// delete committed despite a lost acknowledgement, report that success.
    pub fn retry_rollback(&mut self) -> Result<Option<Removed>> {
        let result = self.rollback();
        if result.is_err() {
            self.database.fence_external_effect();
        }
        result
    }
    fn rollback(&mut self) -> Result<Option<Removed>> {
        let Some(pending) = self.pending.as_mut() else {
            return Ok(None);
        };
        let intent = pending.intent.clone();
        let check = intent.clone();
        let deleted = self
            .database
            .barrier_read(move |tx| {
                let current = scope(tx, check.scope.id)?;
                let saved = read_intent(tx, &check.id)?;
                match (current, saved) {
                    (None, None) => Ok(true),
                    (Some(current), saved)
                        if current == check.scope && saved.as_ref().is_none_or(|i| i == &check) =>
                    {
                        Ok(false)
                    }
                    _ => Err(StorageError::SnapshotChanged),
                }
            })?
            .wait()?;
        if deleted {
            let actual = self
                .pending
                .take()
                .ok_or(StorageError::CompatibilityInvariant)?;
            return Ok(Some(Removed {
                story_id: actual.intent.scope.id,
                uuid: actual.intent.scope.uuid,
            }));
        }
        pending.file.restore()?;
        loop {
            let remove = intent.clone();
            let attempt = self.database.repair_transaction(move |tx| {
                require_scope(tx, &remove.scope)?;
                if let Some(saved) = read_intent(tx, &remove.id)? {
                    if saved != remove {
                        return Err(StorageError::SnapshotChanged);
                    }
                    tx.execute("DELETE FROM removal_intents WHERE id=?", [remove.id])?;
                }
                Ok(())
            });
            match attempt {
                Ok(p) => match p.wait()? {
                    TransactionOutcome::Committed(()) => break,
                    TransactionOutcome::ProvenNotCommitted(e) => return Err(e),
                    TransactionOutcome::CommitUncertain(_) => {
                        let check = intent.clone();
                        if self
                            .database
                            .barrier_read(move |tx| {
                                require_scope(tx, &check.scope)?;
                                Ok(read_intent(tx, &check.id)?.is_none())
                            })?
                            .wait()?
                        {
                            break;
                        }
                    }
                },
                Err(StorageError::QueueFull) => {}
                Err(e) => return Err(e),
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        self.pending = None;
        Ok(None)
    }
}
impl Drop for Removals<'_, '_, '_> {
    fn drop(&mut self) {
        if self.pending.is_some() {
            self.database.fence_external_effect();
        }
    }
}
fn committed(outcome: TransactionOutcome<()>) -> Result<()> {
    match outcome {
        TransactionOutcome::Committed(()) => Ok(()),
        TransactionOutcome::ProvenNotCommitted(e) | TransactionOutcome::CommitUncertain(e) => {
            Err(e)
        }
    }
}
fn scope(tx: &Transaction<'_>, id: i64) -> Result<Option<Scope>> {
    let mut stmt=tx.prepare("SELECT s.id,s.uuid,a.managed_path,a.sha256,a.byte_size FROM stories s JOIN story_archives a ON a.story_id=s.id WHERE s.id=?")?;
    let mut rows = stmt.query([id])?;
    let Some(r) = rows.next()? else {
        return Ok(None);
    };
    for col in [1, 2, 3] {
        if r.get_ref(col)?
            .as_bytes()
            .map_err(|_| StorageError::CompatibilityInvariant)?
            .len()
            > 4096
        {
            return Err(StorageError::CompatibilityInvariant);
        }
    }
    Ok(Some(Scope {
        id: r.get(0)?,
        uuid: r.get(1)?,
        managed: crate::native_path::optional_sql_path(r, 2)?
            .ok_or(StorageError::CompatibilityInvariant)?,
        hash: r.get(3)?,
        bytes: r.get(4)?,
    }))
}
fn require_scope(tx: &Transaction<'_>, expected: &Scope) -> Result<()> {
    if scope(tx, expected.id)?.as_ref() != Some(expected) {
        return Err(StorageError::SnapshotChanged);
    }
    Ok(())
}
fn read_intent(tx: &Transaction<'_>, id: &str) -> Result<Option<Intent>> {
    let row = tx
        .query_row(
            "SELECT story_id,managed_path,trash_path FROM removal_intents WHERE id=?",
            [id],
            |r| {
                for col in [1, 2] {
                    if r.get_ref(col)?.as_bytes()?.len() > 4096 {
                        return Err(rusqlite::Error::InvalidQuery);
                    }
                }
                Ok((
                    r.get::<_, i64>(0)?,
                    crate::native_path::optional_sql_path(r, 1)?
                        .ok_or(rusqlite::Error::InvalidQuery)?,
                    crate::native_path::optional_sql_path(r, 2)?
                        .ok_or(rusqlite::Error::InvalidQuery)?,
                ))
            },
        )
        .optional()?;
    row.map(|(story, managed, trash)| {
        let scope = scope(tx, story)?.ok_or(StorageError::SnapshotChanged)?;
        if scope.managed != managed {
            return Err(StorageError::SnapshotChanged);
        }
        Ok(Intent {
            id: id.into(),
            scope,
            trash,
        })
    })
    .transpose()
}
fn require_intent(tx: &Transaction<'_>, expected: &Intent) -> Result<()> {
    if read_intent(tx, &expected.id)?.as_ref() != Some(expected) {
        return Err(StorageError::SnapshotChanged);
    }
    Ok(())
}
impl Database<'_> {
    pub(crate) fn recover_removals(&self) -> Result<()> {
        if !self
            .barrier_read(|tx| {
                Ok(
                    tx.query_row("SELECT EXISTS(SELECT 1 FROM removal_intents)", [], |r| {
                        r.get::<_, bool>(0)
                    })?,
                )
            })?
            .wait()?
        {
            return Ok(());
        }
        let files = self.metadata_files()?;
        let mut removals = Removals::new(self, &files)?;
        loop {
            // Bounded one-intent-at-a-time scan, before startup accepts work.
            let next = self
                .barrier_read(|tx| {
                    let id = tx
                        .query_row(
                            "SELECT id FROM removal_intents ORDER BY created_at,id LIMIT 1",
                            [],
                            |r| {
                                if r.get_ref(0)?.as_bytes()?.len() != 36 {
                                    return Err(rusqlite::Error::InvalidQuery);
                                }
                                r.get::<_, String>(0)
                            },
                        )
                        .optional()?;
                    id.map(|id| read_intent(tx, &id)?.ok_or(StorageError::SnapshotChanged))
                        .transpose()
                })?
                .wait()?;
            let Some(intent) = next else {
                return Ok(());
            };
            let custody = files.try_custody()?;
            let file = RemovalFile::open(&files, &custody, &intent.scope, &intent.trash, true)?;
            removals.pending = Some(PendingRemoval {
                intent,
                file,
                _custody: custody,
            });
            removals.retry_rollback()?;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        native::NativeLibrary,
        profile::{NativeProfileStore, ProfileLocations},
        test_support::native_copy,
    };
    use std::{
        error::Error,
        fs,
        path::Path,
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    type TestResult = std::result::Result<(), Box<dyn Error>>;
    fn story(db: &Database<'_>) -> Result<Scope> {
        db.read(|tx| {
            let id = tx.query_row("SELECT id FROM stories ORDER BY id LIMIT 1", [], |r| {
                r.get(0)
            })?;
            scope(tx, id)?.ok_or(StorageError::SnapshotChanged)
        })?
        .wait()
    }
    #[test]
    fn native_removal_preserves_sources_shelf_decisions_and_compensates_uncancelled() -> TestResult
    {
        for mode in [
            "intent",
            "trash",
            "delete_failure",
            "success",
            "corrupt_success",
            "corrupt_trash",
        ] {
            let (roots, copy, locations) = native_copy()?;
            let root = copy.root().to_owned();
            let profile = NativeProfileStore::create_fresh(&locations.native)?;
            let native =
                NativeLibrary::bind_independent(copy, &profile, &locations, &Default::default())
                    .map_err(|e| e.to_string())?;
            let mut db = Database::open_native(&native)?;
            let s = story(&db)?;
            if mode.starts_with("corrupt") {
                fs::write(
                    root.join(&s.managed),
                    b"corrupt managed bytes, with a changed size",
                )?;
                committed(
                    db.transaction(Default::default(), |tx| {
                        tx.execute("UPDATE story_archives SET validation_state='invalid'", [])?;
                        Ok(())
                    })?
                    .wait()?,
                )?;
            }
            let original = fs::read(root.join(&s.managed))?;
            let source = roots.library().join("closed-source").join(&s.managed);
            let source_bytes = fs::read(&source)?;
            let seed = s.clone();
            committed(db.transaction(Default::default(),move|tx|{
                tx.execute("INSERT INTO tag_definitions(id,key,normalized_key,label,color,kind,source,position) VALUES(999,'Removal test','removal test','Removal test','#112233','boolean','user',999)",[])?;
                tx.execute("INSERT INTO story_tag_assignments(story_id,definition_id,source) VALUES(?,999,'manual')",[seed.id])?;
                tx.execute("INSERT INTO shelves(id,name,normalized_name,position) VALUES(999,'Removal test','removal test',999)",[])?;
                tx.execute("INSERT INTO shelf_stories(shelf_id,story_uuid,position,protected,cached_title) VALUES(999,?,0,0,'Kept decision')",[seed.uuid])?;
                Ok(())
            })?.wait()?)?;
            if mode == "delete_failure" {
                committed(db.transaction(Default::default(),|tx|{
                    tx.execute_batch("CREATE TRIGGER refuse_removal BEFORE DELETE ON stories BEGIN SELECT RAISE(ABORT,'injected removal delete failure'); END;")?;Ok(())
                })?.wait()?)?;
            }
            {
                let files = LibraryFiles::open_native(&db)?;
                let mut service = Removals::new(&db, &files)?;
                let cancel = WorkCancellation::default();
                let result = service.remove_with(s.id, &cancel, |at| {
                    if at == mode || (mode == "corrupt_trash" && at == "trash") {
                        // SQL and file repair must drain through stopped intake.
                        db.stop_ordinary()?;
                        cancel.cancel();
                    }
                    Ok(())
                });
                if mode.ends_with("success") {
                    assert_eq!(result?.uuid, s.uuid);
                } else {
                    assert!(result.is_err());
                }
                assert!(!service.has_unsettled_removal());
            }
            let id = s.id;
            let (present,tags,membership,intents)=db.barrier_read(move|tx|Ok((
                tx.query_row("SELECT count(*) FROM stories WHERE id=?",[id],|r|r.get::<_,i64>(0))?,
                tx.query_row("SELECT count(*) FROM story_tag_assignments WHERE story_id=? AND definition_id=999",[id],|r|r.get::<_,i64>(0))?,
                tx.query_row("SELECT count(*) FROM shelf_stories WHERE shelf_id=999",[],|r|r.get::<_,i64>(0))?,
                tx.query_row("SELECT count(*) FROM removal_intents",[],|r|r.get::<_,i64>(0))?
            )))?.wait()?;
            assert_eq!(membership, 1);
            assert_eq!(intents, 0);
            if mode.ends_with("success") {
                assert_eq!((present, tags), (0, 0));
                assert!(!root.join(&s.managed).exists());
                let mut found = false;
                for directory in fs::read_dir(root.join("trash/removals"))? {
                    for archived in fs::read_dir(directory?.path())? {
                        if fs::read(archived?.path())? == original {
                            found = true;
                        }
                    }
                }
                assert!(found, "completed removal did not retain exact bytes");
            } else {
                assert_eq!((present, tags), (1, 1));
                assert_eq!(fs::read(root.join(&s.managed))?, original);
            }
            assert_eq!(fs::read(source)?, source_bytes);
            db.close()?;
            drop(db);
            native.finish().map_err(|e| e.to_string())?;
        }
        Ok(())
    }
    #[test]
    fn native_removal_intent_invalidates_prepared_and_queued_export() -> TestResult {
        use crate::export::Exports;
        use librairii_core::export::{Disposition, ExportError, Issue, Scope as ExportScope};
        let (roots, copy, locations) = native_copy()?;
        let profile = NativeProfileStore::create_fresh(&locations.native)?;
        let native =
            NativeLibrary::bind_independent(copy, &profile, &locations, &Default::default())
                .map_err(|e| e.to_string())?;
        let mut db = Database::open_native(&native)?;
        let story = story(&db)?;
        committed(
            db.transaction(Default::default(), move |tx| {
                tx.execute("UPDATE stories SET is_broken=0", [])?;
                Ok(())
            })?
            .wait()?,
        )?;
        let destination = roots.profile().join("exports");
        fs::create_dir(&destination)?;
        {
            let mut queued = Exports::new(&db);
            let mut prepared = Exports::new(&db);
            let first = queued.prepare(
                ExportScope::Selection(vec![story.id]),
                &destination,
                "en".into(),
                &Default::default(),
            )?;
            let second = prepared.prepare(
                ExportScope::Selection(vec![story.id]),
                &destination,
                "en".into(),
                &Default::default(),
            )?;
            let running = queued.start(
                first.preparation.as_ref().ok_or("preparation")?,
                &Default::default(),
            )?;
            let files = LibraryFiles::open_native(&db)?;
            let mut service = Removals::new(&db, &files)?;
            let cancel = WorkCancellation::default();
            assert!(
                service
                    .remove_with(story.id, &cancel, |at| {
                        if at == "intent" {
                            assert_eq!(
                                running.check_item(0)?,
                                Disposition::Skipped(Issue::StoryUnavailable)
                            );
                            assert!(matches!(
                                prepared.start(
                                    second
                                        .preparation
                                        .as_ref()
                                        .ok_or(StorageError::SnapshotChanged)?,
                                    &Default::default()
                                ),
                                Err(StorageError::Export(ExportError::Stale))
                            ));
                            cancel.cancel();
                        }
                        Ok(())
                    })
                    .is_err()
            );
        }
        db.close()?;
        drop(db);
        native.finish().map_err(|e| e.to_string())?;
        Ok(())
    }
    #[test]
    fn native_removal_refuses_foreign_restore_collision_and_retains_owner() -> TestResult {
        let (_roots, copy, locations) = native_copy()?;
        let root = copy.root().to_owned();
        let profile = NativeProfileStore::create_fresh(&locations.native)?;
        let native =
            NativeLibrary::bind_independent(copy, &profile, &locations, &Default::default())
                .map_err(|e| e.to_string())?;
        let mut db = Database::open_native(&native)?;
        let story = story(&db)?;
        let managed = root.join(&story.managed);
        let original = fs::read(&managed)?;
        {
            let files = LibraryFiles::open_native(&db)?;
            let mut service = Removals::new(&db, &files)?;
            let cancel = WorkCancellation::default();
            assert!(
                service
                    .remove_with(story.id, &cancel, |at| {
                        if at == "trash" {
                            fs::write(&managed, b"foreign restore collision")
                                .map_err(|e| StorageError::io("inject restore collision", e))?;
                            cancel.cancel();
                        }
                        Ok(())
                    })
                    .is_err()
            );
            assert!(service.has_unsettled_removal());
            assert_eq!(fs::read(&managed)?, b"foreign restore collision");
            fs::remove_file(&managed)?; // test-created foreign object only
            assert!(service.retry_rollback()?.is_none());
            assert!(!service.has_unsettled_removal());
            assert_eq!(fs::read(&managed)?, original);
        }
        assert!(db.close().is_err());
        drop(db);
        drop(
            native
                .finish()
                .err()
                .ok_or("uncertain generation closed cleanly")?,
        );
        drop(profile);
        let profile = NativeProfileStore::open_native(&locations.native)?;
        let recovery =
            NativeLibrary::recover_interrupted(&profile, &locations, &Default::default())?;
        let mut db = recovery.open()?;
        db.close()?;
        drop(db);
        recovery.finish().map_err(|e| e.to_string())?;
        Ok(())
    }
    #[test]
    fn native_removal_kill_at_intent_move_and_delete_recovers_before_intake() -> TestResult {
        const CHILD: &str = "LIBRAIRII_REMOVAL_CHILD";
        if let Some(context) = std::env::var_os(CHILD) {
            let context: serde_json::Value = serde_json::from_slice(&fs::read(context)?)?;
            let home = Path::new(context["home"].as_str().ok_or("home")?);
            let config = Path::new(context["config"].as_str().ok_or("config")?);
            let data = Path::new(context["data"].as_str().ok_or("data")?);
            let locations = ProfileLocations::for_host(home, Some(config), Some(data))?;
            let profile = NativeProfileStore::open_native(&locations.native)?;
            let native = NativeLibrary::reopen(&profile, &locations, &Default::default())?;
            let db = Database::open_native(&native)?;
            let files = LibraryFiles::open_native(&db)?;
            let mut service = Removals::new(&db, &files)?;
            service.remove_with(story(&db)?.id, &Default::default(), |at| {
                if Some(at) == context["at"].as_str() {
                    fs::write(
                        context["ready"]
                            .as_str()
                            .ok_or(StorageError::CompatibilityInvariant)?,
                        b"ready",
                    )
                    .map_err(|e| StorageError::io("signal removal crash checkpoint", e))?;
                    loop {
                        std::thread::park();
                    }
                }
                Ok(())
            })?;
            return Err("crash checkpoint missed".into());
        }
        for (at, difficult_name) in ["intent", "trash", "deleted", "corrupt_trash"]
            .into_iter()
            .flat_map(|at| [(at, false), (at, true)])
        {
            let (roots, copy, locations) = native_copy()?;
            let root = copy.root().to_owned();
            let profile = NativeProfileStore::create_fresh(&locations.native)?;
            let native =
                NativeLibrary::bind_independent(copy, &profile, &locations, &Default::default())
                    .map_err(|e| e.to_string())?;
            let mut db = Database::open_native(&native)?;
            let s = if difficult_name {
                #[cfg(target_os = "linux")]
                use std::os::unix::ffi::OsStringExt;
                #[cfg(target_os = "linux")]
                let name = std::ffi::OsString::from_vec(b"managed \xe1\x80\xff.ZIP".to_vec());
                #[cfg(target_os = "macos")]
                let name = std::ffi::OsString::from("Managed Été 日本語.ZIP");
                let before = story(&db)?;
                let relative = std::path::PathBuf::from("archives").join(name);
                fs::rename(root.join(&before.managed), root.join(&relative))?;
                committed(
                    db.transaction(Default::default(), move |tx| {
                        tx.execute(
                            "UPDATE story_archives SET managed_path=? WHERE story_id=?",
                            params![crate::native_path::SqlPath(&relative), before.id],
                        )?;
                        Ok(())
                    })?
                    .wait()?,
                )?;
                story(&db)?
            } else {
                story(&db)?
            };
            if at == "corrupt_trash" {
                fs::write(root.join(&s.managed), b"corrupt managed bytes after import")?;
                committed(
                    db.transaction(Default::default(), |tx| {
                        tx.execute("UPDATE story_archives SET validation_state='invalid'", [])?;
                        Ok(())
                    })?
                    .wait()?,
                )?;
            }
            let original = fs::read(root.join(&s.managed))?;
            db.close()?;
            drop(db);
            native.finish().map_err(|e| e.to_string())?;
            drop(profile);
            let context = roots.profile().join("removal-context.json");
            let ready = roots.profile().join("removal-ready");
            fs::write(
                &context,
                serde_json::to_vec(
                    &serde_json::json!({"home":roots.profile().join("home"),"config":roots.profile().join("config"),"data":roots.library().join("data"),"ready":ready,"at":if at=="corrupt_trash" {"trash"} else {at}}),
                )?,
            )?;
            let mut child=Command::new(std::env::current_exe()?).args(["--exact","removal::tests::native_removal_kill_at_intent_move_and_delete_recovers_before_intake"]).env(CHILD,&context).stdout(Stdio::null()).stderr(Stdio::inherit()).spawn()?;
            let deadline = Instant::now() + Duration::from_secs(20);
            while !ready.exists() {
                if let Some(exit) = child.try_wait()? {
                    return Err(format!("child exited before {at}: {exit}").into());
                }
                if Instant::now() > deadline {
                    child.kill()?;
                    child.wait()?;
                    return Err(format!("timeout at {at}").into());
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            child.kill()?;
            child.wait()?;
            let profile = NativeProfileStore::open_native(&locations.native)?;
            let recovery =
                NativeLibrary::recover_interrupted(&profile, &locations, &Default::default())?;
            let mut db = recovery.open()?;
            let id = s.id;
            let (present, intents) = db
                .read(move |tx| {
                    Ok((
                        tx.query_row("SELECT count(*) FROM stories WHERE id=?", [id], |r| {
                            r.get::<_, i64>(0)
                        })?,
                        tx.query_row("SELECT count(*) FROM removal_intents", [], |r| {
                            r.get::<_, i64>(0)
                        })?,
                    ))
                })?
                .wait()?;
            assert_eq!(intents, 0);
            if at == "deleted" {
                assert_eq!(present, 0);
                assert!(!root.join(&s.managed).exists());
                let mut retained = false;
                for directory in fs::read_dir(root.join("trash/removals"))? {
                    for file in fs::read_dir(directory?.path())? {
                        let file = file?;
                        if Some(file.file_name().as_os_str()) == s.managed.file_name() {
                            assert_eq!(fs::read(file.path())?, original);
                            retained = true;
                        }
                    }
                }
                assert!(
                    retained,
                    "committed removal lost original native name/bytes"
                );
            } else {
                assert_eq!(present, 1);
                assert_eq!(fs::read(root.join(&s.managed))?, original);
            }
            db.close()?;
            drop(db);
            recovery.finish().map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

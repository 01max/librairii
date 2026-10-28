//! A supervisor owns these roots until the measured child has joined its workers.
use super::Result;
use image::ImageEncoder;
use librairii_app::localization::Language;
use librairii_storage::{
    database::{Database, TransactionOutcome},
    native::NativeLibrary,
    profile::{NativeProfileStore, ProfileLocations},
    settings::TargetPreview,
};
use librairii_testkit::TestRoots;
use serde::{Deserialize, Serialize};
use std::{
    ffi::OsString,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Configuration {
    pub owner: PathBuf,
    pub home: PathBuf,
    pub config: PathBuf,
    pub data: PathBuf,
    pub imports: Vec<PathBuf>,
}
impl librairii_desktop::startup::StartupEnvironment for Configuration {
    fn variable(&self, name: &str) -> Option<OsString> {
        match name {
            "HOME" => Some(self.home.clone().into_os_string()),
            "XDG_CONFIG_HOME" => Some(self.config.clone().into_os_string()),
            "XDG_DATA_HOME" => Some(self.data.clone().into_os_string()),
            _ => None,
        }
    }
}
pub fn read(path: &Path) -> Result<Configuration> {
    let config: Configuration = serde_json::from_slice(&fs::read(path)?)?;
    if !config.owner.is_absolute()
        || fs::canonicalize(&config.owner)? != config.owner
        || fs::read(config.owner.join(".native-scale-owner"))? != b"native-scale-16\n"
        || [&config.home, &config.config, &config.data]
            .iter()
            .any(|p| !p.starts_with(&config.owner))
        || config.imports.len() != 1000
        || config.imports.iter().any(|p| !p.starts_with(&config.owner))
    {
        return Err("measured child requires the supervisor's exact owned configuration".into());
    }
    Ok(config)
}
pub fn prepare(destination: &Path) -> Result {
    if destination.exists() {
        return Err("fresh supervisor configuration required".into());
    }
    let roots = TestRoots::new()?;
    roots.authorize(Some(roots.library()), Some(roots.profile()))?;
    let owner = roots.library().parent().ok_or("fixture parent")?.to_owned();
    fs::write(owner.join(".native-scale-owner"), b"native-scale-16\n")?;
    let home = roots.profile().join("home");
    let config = roots.profile().join("config");
    let data = roots.library().join("data");
    let locations = ProfileLocations::for_host(&home, Some(&config), Some(&data))?;
    fs::create_dir_all(locations.native.parent().ok_or("profile parent")?)?;
    fs::create_dir_all(&locations.data)?;
    let store = NativeProfileStore::create_fresh(&locations.native)?;
    let native = NativeLibrary::initialize_confirmed(
        TargetPreview::inspect(&roots.library().join("native"), &[], &Default::default())?,
        &store,
        &locations,
        "a0000000-0000-4000-8000-000000000163".parse()?,
        Language::French,
        &Default::default(),
    )
    .map_err(|e| e.to_string())?;
    let root = native.layout().root().to_owned();
    fs::create_dir_all(root.join("catalog/embedded"))?;
    for n in 1..=1000_u32 {
        let mut rgb = vec![0u8; 320 * 400 * 3];
        for (i, p) in rgb.as_chunks_mut::<3>().0.iter_mut().enumerate() {
            p.copy_from_slice(&[(n % 256) as u8, (n / 256) as u8, (i / 320 % 256) as u8]);
        }
        let file = fs::File::create(root.join(format!("catalog/embedded/{n}.png")))?;
        image::codecs::png::PngEncoder::new(file).write_image(
            &rgb,
            320,
            400,
            image::ExtendedColorType::Rgb8,
        )?;
    }
    let mut db = Database::open_native(&native)?;
    let archive = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../preservation/corpus/archives/clockwork-forest.plain.pk");
    let executor = librairii_desktop::imports::ImportExecutor::new(&db);
    let repository = librairii_storage::operations_repository::TerminalRepository::new(&db, 1);
    librairii_app::native_jobs::with_runtime(
        &repository,
        &executor,
        Default::default(),
        |handle| -> Result {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
            let id = "a0000000-0000-4000-8000-000000000164";
            let mut prepared = librairii_desktop::imports::PreparedImport::new(
                id.parse()?,
                vec![archive.clone()],
            )?;
            loop {
                match executor.offer(&handle, prepared) {
                    Ok(()) => break,
                    Err((librairii_desktop::imports::PrepareError::Busy, original))
                        if std::time::Instant::now() < deadline =>
                    {
                        prepared = original;
                        std::thread::sleep(std::time::Duration::from_millis(2));
                    }
                    Err((e, _)) => return Err(e.into()),
                }
            }
            loop {
                if let Some(operation) = db.operation_snapshot(id.into())?.wait()?
                    && !matches!(
                        operation.operation.state,
                        librairii_core::OperationState::Queued
                            | librairii_core::OperationState::Running
                    )
                {
                    if operation.operation.state != librairii_core::OperationState::Succeeded {
                        return Err("fixture payload import failed".into());
                    }
                    break;
                }
                if std::time::Instant::now() >= deadline {
                    return Err("fixture payload import deadline".into());
                }
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            Ok(())
        },
    )??;
    drop(executor);
    let outcome=db.transaction(Default::default(),|tx|{
        for id in 1..=1000_i64 {
            let title=format!("Histoire {id:04} — Été, lune et forêt : une très longue aventure à la bibliothèque");
            if id==1 {tx.execute("UPDATE stories SET embedded_title=?1,display_name_normalized=?2,embedded_artwork_path='catalog/embedded/1.png' WHERE id=1",rusqlite::params![title,title.to_lowercase()])?;continue;}
            tx.execute("INSERT INTO stories(id,uuid,embedded_title,display_name_normalized,embedded_artwork_path) VALUES(?1,printf('16000000-0000-4000-8000-%012d',?1),?2,?3,?4)",rusqlite::params![id,title,title.to_lowercase(),format!("catalog/embedded/{id}.png")])?;
            tx.execute("INSERT INTO story_archives(story_id,original_filename,detected_format,sha256,byte_size,managed_path,validation_state) VALUES(?1,'story.zip','zip',printf('%064d',?1),1,'archives/'||?1,'valid')",[id])?;
        }Ok(())
    })?.wait()?;
    if !matches!(outcome, TransactionOutcome::Committed(())) {
        return Err(format!("fixture seed {outcome:?}").into());
    }
    db.close()?;
    drop(db);
    native.finish().map_err(|e| e.to_string())?;
    drop(store);
    let archive = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../preservation/corpus/archives/clockwork-forest.plain.pk");
    let imports = (0..1000)
        .map(|n| {
            let path = roots.profile().join(format!("input-{n:04}.plain.pk"));
            fs::copy(&archive, &path)?;
            Ok(path)
        })
        .collect::<Result<Vec<_>>>()?;
    let configuration = Configuration {
        owner: owner.clone(),
        home,
        config,
        data,
        imports,
    };
    fs::write(destination, serde_json::to_vec(&configuration)?)?;
    println!("{}", serde_json::json!({"prepared":true,"owner":owner}));
    std::io::stdout().flush()?;
    // EOF is sent only after the child has positively joined; retain custody on
    // an active child rather than automatically retiring underneath its workers.
    let mut confirmation = String::new();
    std::io::stdin().read_to_string(&mut confirmation)?;
    if confirmation != "workers_joined\n" {
        return Err("supervisor requires child shutdown confirmation".into());
    }
    drop(roots);
    if owner.exists() {
        return Err("fixture retirement failed".into());
    }
    println!("{{\"fixture_retired\":true}}");
    Ok(())
}

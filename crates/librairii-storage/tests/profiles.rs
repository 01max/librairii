#[cfg(unix)]
use librairii_storage::profile::{NativeProfileStore, Profile};
use librairii_storage::{
    StorageError,
    profile::{self, ProfileDocument, ProfileLocations},
};
use librairii_testkit::TestRoots;
use std::{error::Error, fs, path::Path};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn strict_profile_wire_limit_unknown_version_and_trailing_values() -> TestResult {
    let valid = br#"{"version":1,"language":"fr"}"#;
    let mut boundary = valid.to_vec();
    boundary.resize(profile::MAX_PROFILE_BYTES, b' ');
    assert_eq!(
        ProfileDocument::decode(&boundary)?.original_bytes(),
        boundary
    );
    boundary.push(b' ');
    assert!(matches!(
        ProfileDocument::decode(&boundary),
        Err(StorageError::InvalidProfile)
    ));
    for invalid in [
        b"{".as_slice(),
        b"null",
        b"[]",
        b"{}",
        br#"{"version":2,"language":"en"}"#,
        br#"{"version":1,"language":"de"}"#,
        br#"{"version":1,"language":"en","unknown":null}"#,
        br#"{"version":1,"language":"en"} {}"#,
        br#"{"version":1,"language":"en"} null"#,
        br#"{"version":1,"language":"en","complete":true}"#,
        br#"{"version":1,"language":"en","root":"relative","libraryId":"raw","recovering":true}"#,
        br#"{"version":1,"language":"en","complete":"false"}"#,
    ] {
        assert!(
            matches!(
                ProfileDocument::decode(invalid),
                Err(StorageError::InvalidProfile)
            ),
            "{invalid:?}"
        );
    }
    Ok(())
}

#[test]
fn profile_decoder_retains_go_scalar_assignment_and_raw_identity() -> TestResult {
    let root = if cfg!(windows) {
        r"C:\Library\Été"
    } else {
        "/Library/Été"
    };
    let bytes = format!(
        r#"{{"verſion":1,"LANGUAGE":"en","language":"fr","language":null,"root":{},"libraryId":"A0000000-0000-4000-8000-000000000001","complete":true}}"#,
        serde_json::to_string(root)?
    );
    let document = ProfileDocument::decode(bytes.as_bytes())?;
    assert_eq!(document.profile().language, "fr");
    assert_eq!(document.profile().root, root);
    assert_eq!(
        document.profile().library_id,
        "A0000000-0000-4000-8000-000000000001"
    );
    assert_eq!(document.original_bytes(), bytes.as_bytes());
    // Go Profile.Validate does not UUID-parse libraryId. It is data until the
    // independently verified runtime binding checks the exact marker spelling.
    let mut profile = document.profile().clone();
    profile.library_id = "legacy non-UUID spelling".into();
    profile.validate()?;
    Ok(())
}

#[test]
fn explicit_legacy_profile_read_preserves_invalid_inputs_without_initialization() -> TestResult {
    let roots = TestRoots::new()?;
    let authority = roots.authorize(Some(roots.library()), Some(roots.profile()))?;
    let directory = authority.profile().join("legacy");
    assert!(profile::read_legacy(&directory)?.is_none());
    assert!(!directory.exists());
    fs::create_dir(&directory)?;
    for bytes in [
        br#"{"version":1,"language":"fr"}"#.as_slice(),
        br#"{"version":99,"language":"fr"}"#,
        b"{broken",
    ] {
        fs::write(directory.join("settings.json"), bytes)?;
        let outcome = profile::read_legacy(&directory);
        if bytes.starts_with(br#"{"version":1,"#) {
            assert_eq!(outcome?.ok_or("missing settings")?.original_bytes(), bytes);
        } else {
            assert!(matches!(outcome, Err(StorageError::InvalidProfile)));
        }
        assert_eq!(fs::read(directory.join("settings.json"))?, bytes);
        assert_eq!(fs::read_dir(&directory)?.count(), 1);
    }
    assert_eq!(fs::read_dir(authority.library())?.count(), 0);
    Ok(())
}

#[test]
fn host_profile_locations_are_explicit_separate_and_do_not_create_paths() -> TestResult {
    let roots = TestRoots::new()?;
    let home = roots.profile().join("home");
    let config = roots.profile().join("config");
    let data = roots.profile().join("data");
    let locations = ProfileLocations::for_host(&home, Some(&config), Some(&data))?;
    #[cfg(target_os = "macos")]
    assert_eq!(
        locations,
        ProfileLocations {
            legacy: Some(home.join("Library/Preferences/io.github.librairii.app")),
            native: home.join("Library/Preferences/io.github.librairii.native"),
            data: home.join("Library/Application Support/LibrairiiNative"),
        }
    );
    #[cfg(target_os = "linux")]
    {
        assert_eq!(
            locations,
            ProfileLocations {
                legacy: Some(config.join("librairii")),
                native: config.join("librairii-native"),
                data: data.join("librairii-native"),
            }
        );
        let defaults = ProfileLocations::for_host(&home, None, None)?;
        assert_eq!(defaults.legacy, Some(home.join(".config/librairii")));
        assert_eq!(defaults.native, home.join(".config/librairii-native"));
        assert_eq!(defaults.data, home.join(".local/share/librairii-native"));
    }
    #[cfg(target_os = "windows")]
    {
        assert_eq!(
            locations,
            ProfileLocations {
                legacy: None,
                native: config.join("LibrairiiNative"),
                data: data.join("LibrairiiNative/Data"),
            }
        );
        assert!(ProfileLocations::for_host(&home, None, Some(&data)).is_err());
        assert!(ProfileLocations::for_host(&home, Some(&config), None).is_err());
    }
    assert!(ProfileLocations::for_host(Path::new("relative"), None, None).is_err());
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    assert!(ProfileLocations::for_host(&home, Some(Path::new("relative")), Some(&data)).is_err());
    assert!(!home.exists() && !config.exists() && !data.exists());
    Ok(())
}

#[test]
#[cfg(unix)]
fn fresh_native_namespace_keeps_legacy_read_only_and_uses_private_permissions() -> TestResult {
    use std::os::unix::fs::PermissionsExt;
    let roots = TestRoots::new()?;
    let authority = roots.authorize(Some(roots.library()), Some(roots.profile()))?;
    let legacy = authority.profile().join("legacy");
    fs::create_dir(&legacy)?;
    let bytes = include_bytes!("../../../preservation/corpus/profiles/settings.json");
    fs::write(legacy.join("settings.json"), bytes)?;
    assert_eq!(
        profile::read_legacy(&legacy)?
            .ok_or("legacy absent")?
            .original_bytes(),
        bytes
    );
    assert!(NativeProfileStore::open_native(&legacy).is_err());
    assert!(matches!(
        NativeProfileStore::create_fresh(&legacy),
        Err(StorageError::RootNotFresh)
    ));
    let native = authority.profile().join("native");
    let store = NativeProfileStore::create_fresh(&native)?;
    assert!(store.load()?.is_none());
    let saved = store.update(|profile| {
        profile.language = "fr".into();
        Ok(())
    })?;
    assert_eq!(
        saved,
        Profile {
            language: "fr".into(),
            ..Profile::setup()
        }
    );
    assert_eq!(store.recovery_directory(), native.join("recovery"));
    assert!(!store.recovery_directory().exists());
    assert_eq!(fs::read(legacy.join("settings.json"))?, bytes);
    assert_eq!(fs::read_dir(&legacy)?.count(), 1);
    for path in [
        &native,
        &native.join("settings.json"),
        &native.join(".librairii-native-profile"),
    ] {
        assert_eq!(fs::metadata(path)?.permissions().mode() & 0o077, 0);
    }
    assert_eq!(fs::read_dir(authority.library())?.count(), 0);
    drop(store);
    assert_eq!(
        NativeProfileStore::open_native(&native)?
            .load()?
            .ok_or("native absent")?
            .profile(),
        &saved
    );
    Ok(())
}

#[test]
#[cfg(unix)]
fn serialized_native_updates_keep_language_and_root_without_lost_snapshots() -> TestResult {
    use std::{sync::Arc, thread};
    let roots = TestRoots::new()?;
    let store = Arc::new(NativeProfileStore::create_fresh(
        &roots.profile().join("native"),
    )?);
    let change = |index, profile: &mut Profile| {
        if index % 2 == 0 {
            profile.language = "fr".into();
        } else {
            profile.root = "/independently-verified-later".into();
            profile.library_id = "Raw-ID".into();
            profile.complete = true;
        }
        Ok(())
    };
    let workers = (0..20)
        .map(|index| {
            let store = Arc::clone(&store);
            thread::spawn(move || (index, store.update(|p| change(index, p))))
        })
        .collect::<Vec<_>>();
    // Bounded refusal retains the caller's command. Once every racing caller
    // has returned, the coordinator may explicitly resubmit each refused copy.
    let mut refused = Vec::new();
    for worker in workers {
        let (index, result) = worker.join().map_err(|_| "update worker panicked")?;
        match result {
            Ok(_) => (),
            Err(StorageError::ProfileBusy) => refused.push(index),
            Err(StorageError::ProfileBindingActive) if index % 2 != 0 => (),
            Err(error) => return Err(error.into()),
        }
    }
    for index in refused {
        let result = store.update(|p| change(index, p));
        if index % 2 == 0 {
            result?;
        } else {
            assert!(matches!(result, Err(StorageError::ProfileBindingActive)));
        }
    }
    let final_profile = store.load()?.ok_or("missing profile")?.profile().clone();
    assert_eq!(final_profile.language, "fr");
    assert_eq!(final_profile.root, "");
    assert_eq!(final_profile.library_id, "");
    assert!(!final_profile.complete);
    let directory = store.directory().to_path_buf();
    drop(store);
    assert_eq!(
        NativeProfileStore::open_native(&directory)?
            .load()?
            .ok_or("missing restart")?
            .profile(),
        &final_profile
    );
    Ok(())
}

#[test]
#[cfg(unix)]
fn invalid_update_and_cancel_preserve_effective_profile_and_staged_cleanup() -> TestResult {
    let roots = TestRoots::new()?;
    let store = NativeProfileStore::create_fresh(&roots.profile().join("native"))?;
    let prior = store.update(|_| Ok(()))?;
    let prior_bytes = fs::read(store.directory().join("settings.json"))?;
    assert!(matches!(
        store.update(|p| {
            p.language = "de".into();
            Ok(())
        }),
        Err(StorageError::InvalidProfile)
    ));
    assert!(matches!(
        store.update(|p| {
            p.language = "fr".into();
            Err(StorageError::Cancelled)
        }),
        Err(StorageError::Cancelled)
    ));
    assert!(matches!(
        store.update(|p| {
            p.root = "x".repeat(profile::MAX_PROFILE_BYTES);
            Ok(())
        }),
        Err(StorageError::InvalidProfile)
    ));
    assert_eq!(store.load()?.ok_or("missing profile")?.profile(), &prior);
    assert_eq!(
        fs::read(store.directory().join("settings.json"))?,
        prior_bytes
    );
    assert_eq!(fs::read_dir(store.directory())?.count(), 2);
    Ok(())
}

#[test]
#[cfg(unix)]
fn invalid_native_settings_are_preserved_and_never_reinitialized() -> TestResult {
    let roots = TestRoots::new()?;
    let directory = roots.profile().join("native");
    drop(NativeProfileStore::create_fresh(&directory)?);
    for bytes in [
        br#"{"version":99,"language":"en"}"#.as_slice(),
        br#"{"version":1,"language":"en","future":true}"#,
        b"{invalid",
    ] {
        fs::write(directory.join("settings.json"), bytes)?;
        let store = NativeProfileStore::open_native(&directory)?;
        assert!(matches!(store.load(), Err(StorageError::InvalidProfile)));
        assert!(matches!(
            store.update(|p| {
                p.language = "fr".into();
                Ok(())
            }),
            Err(StorageError::InvalidProfile)
        ));
        assert_eq!(fs::read(directory.join("settings.json"))?, bytes);
        assert_eq!(fs::read_dir(&directory)?.count(), 2);
    }
    Ok(())
}

#[test]
#[cfg(unix)]
fn native_store_refuses_substituted_identical_settings_and_namespace() -> TestResult {
    let roots = TestRoots::new()?;
    let directory = roots.profile().join("native");
    let store = NativeProfileStore::create_fresh(&directory)?;
    store.update(|_| Ok(()))?;
    let bytes = fs::read(directory.join("settings.json"))?;
    fs::rename(
        directory.join("settings.json"),
        directory.join("old-settings"),
    )?;
    fs::write(directory.join("settings.json"), &bytes)?;
    assert!(matches!(
        store.update(|p| {
            p.language = "fr".into();
            Ok(())
        }),
        Err(StorageError::ProfileChanged)
    ));
    assert_eq!(fs::read(directory.join("settings.json"))?, bytes);
    fs::rename(&directory, roots.profile().join("retained-old-native"))?;
    let replacement = NativeProfileStore::create_fresh(&directory)?;
    assert!(matches!(store.load(), Err(StorageError::ProfileChanged)));
    assert!(replacement.load()?.is_none());
    Ok(())
}

#[test]
#[cfg(unix)]
fn native_and_legacy_profile_aliases_are_refused_without_following_effects() -> TestResult {
    use std::os::unix::fs::symlink;
    let roots = TestRoots::new()?;
    let directory = roots.profile().join("native");
    let store = NativeProfileStore::create_fresh(&directory)?;
    store.update(|_| Ok(()))?;
    drop(store);
    symlink(&directory, roots.profile().join("alias"))?;
    assert!(matches!(
        NativeProfileStore::open_native(&roots.profile().join("alias")),
        Err(StorageError::PathEscape)
    ));
    let legacy = roots.profile().join("legacy");
    fs::create_dir(&legacy)?;
    symlink(
        directory.join("settings.json"),
        legacy.join("settings.json"),
    )?;
    assert!(matches!(
        profile::read_legacy(&legacy),
        Err(StorageError::PathEscape)
    ));
    fs::hard_link(
        directory.join(".librairii-native-profile"),
        roots.profile().join("marker-alias"),
    )?;
    assert!(matches!(
        NativeProfileStore::open_native(&directory),
        Err(StorageError::PathEscape)
    ));
    assert_eq!(
        profile::read_legacy(&directory)?
            .ok_or("profile absent")?
            .profile()
            .language,
        "en"
    );
    Ok(())
}

#[test]
#[cfg(unix)]
fn native_profile_lock_excludes_other_store_and_process_until_close() -> TestResult {
    let roots = TestRoots::new()?;
    let directory = roots.profile().join("native");
    let store = NativeProfileStore::create_fresh(&directory)?;
    assert!(matches!(
        NativeProfileStore::open_native(&directory),
        Err(StorageError::ProfileInUse)
    ));
    let child = |locked: &str| -> Result<bool, Box<dyn Error>> {
        Ok(std::process::Command::new(std::env::current_exe()?)
            .args(["--exact", "profile_lock_child_entry", "--nocapture"])
            .env("LIBRAIRII_TEST_PROFILE_LOCK_DIR", &directory)
            .env("LIBRAIRII_TEST_PROFILE_EXPECT_LOCK", locked)
            .status()?
            .success())
    };
    assert!(child("1")?);
    drop(store);
    assert!(child("0")?);
    Ok(())
}

#[test]
#[cfg(unix)]
fn profile_lock_child_entry() -> TestResult {
    let Some(directory) = std::env::var_os("LIBRAIRII_TEST_PROFILE_LOCK_DIR") else {
        return Ok(());
    };
    let result = NativeProfileStore::open_native(Path::new(&directory));
    if std::env::var("LIBRAIRII_TEST_PROFILE_EXPECT_LOCK")?.as_str() == "1" {
        assert!(matches!(result, Err(StorageError::ProfileInUse)));
    } else {
        result?;
    }
    Ok(())
}

#[test]
#[cfg(windows)]
fn native_windows_profile_effects_refuse_until_directory_durability_is_qualified() -> TestResult {
    use librairii_storage::profile::NativeProfileStore;
    let roots = TestRoots::new()?;
    let directory = roots.profile().join("native");
    assert!(matches!(
        NativeProfileStore::create_fresh(&directory),
        Err(StorageError::DirectoryDurabilityUnavailable)
    ));
    assert!(!directory.exists());
    Ok(())
}

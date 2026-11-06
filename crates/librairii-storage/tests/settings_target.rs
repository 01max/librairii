#![cfg(any(target_os = "macos", target_os = "linux"))]
use librairii_storage::{database::WorkCancellation, settings::TargetPreview};
use librairii_testkit::TestRoots;
use std::{
    error::Error,
    fs,
    os::unix::fs::{PermissionsExt, symlink},
};
type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn native_case_and_decomposition_aliases_cannot_bypass_excluded_root_ancestry() -> TestResult {
    use std::os::unix::fs::MetadataExt;
    let roots = TestRoots::new()?;
    for (name, alternative) in [("NativeRoot", "nativeroot"), ("Café", "Cafe\u{301}")] {
        let source = roots.library().join(name);
        let alias = roots.library().join(alternative);
        fs::create_dir(&source)?;
        let metadata = fs::metadata(&source)?;
        match fs::symlink_metadata(&alias) {
            Ok(_) => (),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => fs::create_dir(&alias)?,
            Err(error) => return Err(error.into()),
        }
        let other = fs::metadata(&alias)?;
        let target = alias.join("uninitialized");
        let preview = TargetPreview::inspect(
            &target,
            &[&source, roots.profile()],
            &WorkCancellation::default(),
        );
        if metadata.dev() == other.dev() && metadata.ino() == other.ino() {
            assert!(
                preview.is_err(),
                "native alias must refuse before writable probing"
            );
        } else {
            let mut preview = preview?;
            preview.check_writable(&WorkCancellation::default())?;
            assert_eq!(fs::read_dir(&alias)?.count(), 0);
        }
        assert!(!target.exists());
        assert_eq!(fs::read_dir(&source)?.count(), 0);
    }
    Ok(())
}

#[test]
fn target_preview_preserves_exact_empty_or_missing_selection_and_probes_actual_writes() -> TestResult
{
    let roots = TestRoots::new()?;
    let target = roots.library().join("Exact chosen été");
    fs::create_dir(&target)?;
    let mut preview =
        TargetPreview::inspect(&target, &[roots.profile()], &WorkCancellation::default())?;
    assert_eq!(preview.requested(), target);
    assert_eq!(preview.resolved(), fs::canonicalize(&target)?);
    preview.check_writable(&WorkCancellation::default())?;
    assert_eq!(fs::read_dir(&target)?.count(), 0);
    let missing = roots.library().join("new-parent/library");
    let mut absent =
        TargetPreview::inspect(&missing, &[roots.profile()], &WorkCancellation::default())?;
    absent.check_writable(&WorkCancellation::default())?;
    assert_eq!(
        absent.resolved(),
        fs::canonicalize(roots.library())?.join("new-parent/library")
    );
    assert!(!missing.parent().ok_or("parent")?.exists());
    assert_eq!(fs::read_dir(roots.library())?.count(), 1);
    Ok(())
}

#[test]
fn target_preview_refuses_overlap_aliases_links_nonempty_and_special_files() -> TestResult {
    let roots = TestRoots::new()?;
    let source = roots.library().join("source");
    let target = roots.library().join("target");
    fs::create_dir(&source)?;
    fs::create_dir(&target)?;
    symlink(&source, roots.library().join("alias"))?;
    for path in [
        source.join("nested"),
        roots.library().to_path_buf(),
        roots.profile().join("nested"),
        roots.library().join("alias/child"),
        roots.library().join("alias"),
    ] {
        assert!(
            TargetPreview::inspect(
                &path,
                &[&source, roots.profile()],
                &WorkCancellation::default()
            )
            .is_err()
        );
    }
    fs::write(target.join("sentinel"), b"preserved")?;
    assert!(
        TargetPreview::inspect(
            &target,
            &[&source, roots.profile()],
            &WorkCancellation::default()
        )
        .is_err()
    );
    let special = roots.library().join("socket");
    let _listener = std::os::unix::net::UnixListener::bind(&special)?;
    assert!(
        TargetPreview::inspect(&special, &[roots.profile()], &WorkCancellation::default()).is_err()
    );
    assert_eq!(fs::read(target.join("sentinel"))?, b"preserved");
    Ok(())
}

#[test]
fn target_revalidation_detects_replacement_new_files_and_changed_ancestors() -> TestResult {
    let roots = TestRoots::new()?;
    let target = roots.library().join("target");
    fs::create_dir(&target)?;
    let preview =
        TargetPreview::inspect(&target, &[roots.profile()], &WorkCancellation::default())?;
    fs::rename(&target, roots.library().join("retained"))?;
    fs::create_dir(&target)?;
    assert!(preview.revalidate(&WorkCancellation::default()).is_err());
    let preview =
        TargetPreview::inspect(&target, &[roots.profile()], &WorkCancellation::default())?;
    fs::write(target.join("arrived"), b"new file")?;
    assert!(preview.revalidate(&WorkCancellation::default()).is_err());
    let missing = roots.library().join("missing/target");
    let preview =
        TargetPreview::inspect(&missing, &[roots.profile()], &WorkCancellation::default())?;
    fs::create_dir(missing.parent().ok_or("missing parent")?)?;
    assert!(preview.revalidate(&WorkCancellation::default()).is_err());
    assert_eq!(fs::read(target.join("arrived"))?, b"new file");
    let configuration = roots.profile().join("configuration");
    let alias = roots.profile().join("configuration-alias");
    let other = roots.library().join("other-target");
    fs::create_dir(&configuration)?;
    fs::create_dir(&other)?;
    symlink(&configuration, &alias)?;
    let preview = TargetPreview::inspect(&other, &[&alias], &WorkCancellation::default())?;
    fs::remove_file(&alias)?;
    symlink(&other, &alias)?;
    assert!(preview.revalidate(&WorkCancellation::default()).is_err());
    assert_eq!(fs::read_dir(&other)?.count(), 0);
    Ok(())
}

#[test]
fn target_preflight_observes_readonly_storage_and_cancellation_without_initialization() -> TestResult
{
    let roots = TestRoots::new()?;
    let target = roots.library().join("readonly");
    fs::create_dir(&target)?;
    fs::set_permissions(&target, fs::Permissions::from_mode(0o500))?;
    let result = (|| -> TestResult {
        let mut preview =
            TargetPreview::inspect(&target, &[roots.profile()], &WorkCancellation::default())?;
        assert!(
            preview
                .check_writable(&WorkCancellation::default())
                .is_err()
        );
        assert_eq!(fs::read_dir(&target)?.count(), 0);
        Ok(())
    })();
    fs::set_permissions(&target, fs::Permissions::from_mode(0o700))?;
    result?;
    let cancellation = WorkCancellation::default();
    cancellation.cancel();
    assert!(TargetPreview::inspect(&target, &[roots.profile()], &cancellation).is_err());
    assert_eq!(fs::read_dir(&target)?.count(), 0);
    Ok(())
}

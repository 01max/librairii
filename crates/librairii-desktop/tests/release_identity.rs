use librairii_testkit::TestRoots;
use std::process::{Command, Output};

fn identity_output(argument: &str) -> Result<Output, Box<dyn std::error::Error>> {
    let roots = TestRoots::new()?;
    let output = Command::new(env!("CARGO_BIN_EXE_librairii-desktop"))
        .env_clear()
        // If identity handling regresses, startup must refuse before any window
        // or normal profile access instead of launching the application.
        .env("LIBRAIRII_DATA_ROOT", "relative-startup-is-refused")
        .current_dir(roots.profile())
        .arg(argument)
        .output()?;
    assert!(output.status.success(), "{:?}", output.stderr);
    assert_eq!(std::fs::read_dir(roots.profile())?.count(), 0);
    assert_eq!(std::fs::read_dir(roots.library())?.count(), 0);
    Ok(output)
}

#[test]
fn product_version_is_available_without_profile_or_display()
-> Result<(), Box<dyn std::error::Error>> {
    let output = identity_output("--version")?;
    assert_eq!(
        String::from_utf8(output.stdout)?,
        format!("Librairii {}\n", include_str!("../../../VERSION").trim())
    );
    Ok(())
}

#[test]
fn package_build_identity_is_available_without_profile_or_display()
-> Result<(), Box<dyn std::error::Error>> {
    let output = identity_output("--build-info")?;
    let info: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(info["format"], "librairii.native-build");
    assert_eq!(info["version"], include_str!("../../../VERSION").trim());
    assert_eq!(info["arch"], std::env::consts::ARCH);
    assert_eq!(info["os"], std::env::consts::OS);
    assert_eq!(info["runtime"], "Rust/Slint");
    assert_eq!(info["debug_assertions"], cfg!(debug_assertions));
    Ok(())
}

#[test]
fn packaged_acceptance_without_explicit_roots_refuses_before_menu_or_profile_startup()
-> Result<(), Box<dyn std::error::Error>> {
    let roots = TestRoots::new()?;
    let output = Command::new(env!("CARGO_BIN_EXE_librairii-desktop"))
        .env_clear()
        .env("LIBRAIRII_PACKAGED_ACCEPTANCE", "1")
        .current_dir(roots.profile())
        .output()?;
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("packaged acceptance requires explicit owned isolated roots")
    );
    assert!(!String::from_utf8_lossy(&output.stderr).contains("native_startup"));
    assert_eq!(std::fs::read_dir(roots.profile())?.count(), 0);
    assert_eq!(std::fs::read_dir(roots.library())?.count(), 0);
    Ok(())
}

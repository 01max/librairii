use librairii_testkit::TestRoots;
#[test]
fn unimplemented_desktop_refuses_launch_without_touching_isolated_roots() -> std::io::Result<()> {
    let roots = TestRoots::new()?;
    let result = std::process::Command::new(env!("CARGO_BIN_EXE_librairii-desktop"))
        .env("LIBRAIRII_DATA_ROOT", roots.library())
        .env("XDG_CONFIG_HOME", roots.profile())
        .env("APPDATA", roots.profile())
        .env("HOME", roots.profile())
        .output()?;
    assert_eq!(result.status.code(), Some(1));
    assert!(result.stdout.is_empty());
    assert!(String::from_utf8_lossy(&result.stderr).contains("not implemented"));
    assert_eq!(std::fs::read_dir(roots.library())?.count(), 0);
    assert_eq!(std::fs::read_dir(roots.profile())?.count(), 0);
    Ok(())
}

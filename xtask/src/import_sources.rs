use crate::{Result, output, root};

pub fn inventory() -> Result {
    let python = if cfg!(windows) { "python" } else { "python3" };
    eprintln!("Checking private studio JSONv1 source provenance (not parity)");
    let report = output(
        python,
        &[
            "-I",
            "-B",
            "-X",
            "utf8",
            "xtask/check_import_sources.py",
            "--inventory",
        ],
    )?;
    let _: serde_json::Value = serde_json::from_str(&report)?;
    std::fs::create_dir_all(root().join("target/quality"))?;
    std::fs::write(
        root().join("target/quality/import-source-inventory.json"),
        report,
    )?;
    Ok(())
}

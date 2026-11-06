use crate::{Result, output, root};
pub fn inventory() -> Result<serde_json::Value> {
    let report = output("python3", &["-I", "-B", "xtask/check_archive_patch.py"])?;
    let value: serde_json::Value = serde_json::from_str(&report)?;
    std::fs::create_dir_all(root().join("target/quality"))?;
    std::fs::write(
        root().join("target/quality/archive-patch-inventory.json"),
        report,
    )?;
    Ok(value)
}

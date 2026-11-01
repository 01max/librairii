use crate::{Result, root};
use std::process::Command;

/// OS packaging uses the platform tools through one source-controlled facade.
pub fn run(arguments: &[String]) -> Result {
    let status = Command::new("python3")
        .args(["-I", "-B", "scripts/native-release.py"])
        .args(arguments)
        .current_dir(root())
        .status()?;
    if !status.success() {
        return Err(format!("native release orchestration failed: {status}").into());
    }
    Ok(())
}

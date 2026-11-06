use crate::{Result, output, root};

fn python() -> &'static str {
    if cfg!(windows) { "python" } else { "python3" }
}

pub fn check() -> Result {
    eprintln!(
        "Checking the admitted synthetic preservation corpus (read-only; no Go regeneration)"
    );
    let report = output(
        python(),
        &["-I", "-B", "-X", "utf8", "xtask/check_preservation.py"],
    )?;
    let _: serde_json::Value = serde_json::from_str(&report)?;
    std::fs::create_dir_all(root().join("target/quality"))?;
    std::fs::write(root().join("target/quality/preservation.json"), report)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command;

    #[test]
    fn admitted_fixture_pins_reject_missing_or_changed_inputs() -> Result {
        command(
            python(),
            &["-I", "-B", "-X", "utf8", "xtask/run_preservation_tests.py"],
        )
    }
}

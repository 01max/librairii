use crate::{Result, cargo, output, root};
use serde::Deserialize;
use serde_json::Value;
use std::fs;
#[derive(Debug, Deserialize)]
struct Baseline {
    required: Vec<Required>,
}
#[derive(Debug, Deserialize)]
struct Required {
    suite: String,
    name: String,
    targets: Option<Vec<String>>,
}
fn validate(baseline: &Baseline, discovered: &Value, target: &str) -> Result {
    let suites = discovered["rust-suites"]
        .as_object()
        .ok_or("missing test discovery suites")?;
    for entry in &baseline.required {
        if entry
            .targets
            .as_ref()
            .is_some_and(|targets| !targets.iter().any(|t| t == target))
        {
            continue;
        }
        let test = &discovered["rust-suites"][&entry.suite]["testcases"][&entry.name];
        if test.is_null() {
            return Err(format!(
                "required test disappeared: {} / {}",
                entry.suite, entry.name
            )
            .into());
        }
    }
    for (suite, value) in suites {
        for (name, test) in value["testcases"]
            .as_object()
            .ok_or("missing discovered testcases")?
        {
            if test["ignored"] != false || test["filter-match"]["status"] != "matches" {
                return Err(
                    format!("ignored or filtered check is incomplete: {suite} / {name}").into(),
                );
            }
        }
    }
    Ok(())
}
pub fn check(target: &str) -> Result {
    let baseline: Baseline = serde_json::from_str(&fs::read_to_string(
        root().join(".config/rust-test-baseline.json"),
    )?)?;
    let data = output(
        &cargo(),
        &[
            "nextest",
            "list",
            "--workspace",
            "--locked",
            "--target",
            target,
            "--profile",
            "ci",
            "--message-format",
            "json",
        ],
    )?;
    validate(&baseline, &serde_json::from_str(&data)?, target)?;
    fs::create_dir_all(root().join("target/quality"))?;
    fs::write(root().join("target/quality/test-discovery.json"), data)?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn removed_ignored_or_filtered_required_tests_cannot_pass_discovery() {
        let baseline = Baseline {
            required: vec![Required {
                suite: "fixture".into(),
                name: "preserves_bytes".into(),
                targets: None,
            }],
        };
        let mut data = json!({"rust-suites":{"fixture":{"testcases":{"preserves_bytes":{"ignored":false,"filter-match":{"status":"matches"}}}}}});
        assert!(validate(&baseline, &data, "synthetic").is_ok());
        data["rust-suites"]["fixture"]["testcases"]["preserves_bytes"]["ignored"] = json!(true);
        assert!(validate(&baseline, &data, "synthetic").is_err());
        data["rust-suites"]["fixture"]["testcases"]["preserves_bytes"]["ignored"] = json!(false);
        data["rust-suites"]["fixture"]["testcases"]["preserves_bytes"]["filter-match"]["status"] =
            json!("mismatch");
        assert!(validate(&baseline, &data, "synthetic").is_err());
        data["rust-suites"]["fixture"]["testcases"] = json!({});
        assert!(validate(&baseline, &data, "synthetic").is_err());
    }
}

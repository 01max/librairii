use crate::{Result, cargo, check_tool, command, output, root};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::BTreeSet, fs, path::Path};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Exceptions {
    exceptions: Vec<Exception>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Exception {
    kind: String,
    selector: String,
    owner: String,
    reason: String,
    tracking: String,
    expires: String,
}

fn date_valid(date: &str) -> bool {
    let parts: Vec<_> = date.split('-').collect();
    if parts.len() != 3 || date.len() != 10 {
        return false;
    }
    let parse = |index: usize| parts[index].parse::<u32>().ok();
    match (parse(0), parse(1), parse(2)) {
        (Some(year), Some(month @ 1..=12), Some(day)) if year >= 2026 => {
            let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
            let max = match month {
                2 if leap => 29,
                2 => 28,
                4 | 6 | 9 | 11 => 30,
                _ => 31,
            };
            (1..=max).contains(&day)
        }
        _ => false,
    }
}
fn validate(
    exceptions: &Exceptions,
    today: &str,
    suppressions: &BTreeSet<(String, String)>,
) -> Result {
    let mut records = BTreeSet::new();
    for entry in &exceptions.exceptions {
        if !matches!(entry.kind.as_str(), "advisory" | "duplicate" | "license")
            || entry.selector.trim().is_empty()
            || entry.owner.trim().is_empty()
            || entry.reason.trim().is_empty()
            || !entry.tracking.starts_with("https://")
            || !date_valid(&entry.expires)
            || entry.expires.as_str() <= today
            || !records.insert((entry.kind.clone(), entry.selector.clone()))
        {
            return Err(format!(
                "invalid, duplicate or expired dependency exception: {}",
                entry.selector
            )
            .into());
        }
    }
    if &records != suppressions {
        return Err("dependency suppressions and expiring records must match exactly".into());
    }
    Ok(())
}
pub fn check() -> Result {
    check_tool("cargo-deny")?;
    check_tool("cargo-audit")?;
    let records: Exceptions = toml::from_str(&fs::read_to_string(
        root().join(".config/dependency-exceptions.toml"),
    )?)?;
    let config: toml::Value = toml::from_str(&fs::read_to_string(root().join("deny.toml"))?)?;
    let mut suppressions = BTreeSet::new();
    for (kind, section, key) in [
        ("advisory", "advisories", "ignore"),
        ("duplicate", "bans", "skip"),
        ("license", "licenses", "exceptions"),
    ] {
        for entry in config[section][key]
            .as_array()
            .ok_or("missing dependency suppression list")?
        {
            let selector = entry
                .as_str()
                .or_else(|| entry.get("id").and_then(toml::Value::as_str))
                .or_else(|| entry.get("crate").and_then(toml::Value::as_str))
                .ok_or("invalid dependency suppression")?;
            if kind != "advisory" && !selector.contains('@') {
                return Err("exception must select an exact crate@version".into());
            }
            suppressions.insert((kind.to_owned(), selector.to_owned()));
        }
    }
    if !config["bans"]["skip-tree"]
        .as_array()
        .ok_or("missing skip-tree policy")?
        .is_empty()
    {
        return Err("broad dependency subtree suppressions are forbidden".into());
    }
    validate(
        &records,
        output("date", &["-u", "+%Y-%m-%d"])?.trim(),
        &suppressions,
    )?;
    command(&cargo(), &["deny", "--locked", "check"])?;
    let mut args = vec!["audit", "--file", "Cargo.lock", "--deny", "warnings"];
    for entry in &records.exceptions {
        if entry.kind == "advisory" {
            args.extend(["--ignore", &entry.selector]);
        }
    }
    command(&cargo(), &args)?;
    inventory()
}

fn write_notices(package: &Value, notices: &Path, text: &mut String) -> Result {
    let name = package["name"].as_str().ok_or("missing dependency name")?;
    let version = package["version"]
        .as_str()
        .ok_or("missing dependency version")?;
    let manifest = Path::new(
        package["manifest_path"]
            .as_str()
            .ok_or("missing dependency manifest")?,
    );
    let parent = manifest.parent().ok_or("missing dependency directory")?;
    let directory = notices.join(format!("{name}-{version}"));
    fs::create_dir_all(&directory)?;
    let mut count = 0;
    for entry in fs::read_dir(parent)? {
        let entry = entry?;
        let filename = entry.file_name();
        let upper = filename.to_string_lossy().to_ascii_uppercase();
        if entry.file_type()?.is_file()
            && (upper.starts_with("LICENSE")
                || upper.starts_with("COPYING")
                || upper.starts_with("NOTICE"))
        {
            fs::copy(entry.path(), directory.join(&filename))?;
            count += 1;
        }
    }
    if count == 0 {
        return Err(format!("no upstream license/notice files collected for {name}@{version}; review before redistribution").into());
    }
    text.push_str(&format!(
        "{name} {version}: {}\n{}\nFiles: {name}-{version}/\n\n",
        package["license"], package["repository"]
    ));
    Ok(())
}
pub fn inventory() -> Result {
    let destination = root().join("target/quality");
    let notices = destination.join("notices");
    fs::create_dir_all(&notices)?;
    let lock: toml::Value = toml::from_str(&fs::read_to_string(root().join("Cargo.lock"))?)?;
    let targets: toml::Value = toml::from_str(&fs::read_to_string(
        root().join(".config/rust-checks.toml"),
    )?)?;
    let mut inventory = Vec::new();
    let mut notice_text = String::from(
        "Upstream Rust dependency notices. Product license remains undecided.\nIncludes production and test/tooling dependencies; not release qualification.\n\n",
    );
    let mut noticed = BTreeSet::new();
    for target in targets["targets"]
        .as_table()
        .ok_or("missing target configurations")?
        .keys()
    {
        let metadata: Value = serde_json::from_str(&output(
            &cargo(),
            &[
                "metadata",
                "--locked",
                "--format-version",
                "1",
                "--filter-platform",
                target,
            ],
        )?)?;
        let packages = metadata["packages"]
            .as_array()
            .ok_or("missing metadata packages")?;
        let nodes = metadata["resolve"]["nodes"]
            .as_array()
            .ok_or("missing metadata nodes")?;
        for node in nodes {
            let package = packages
                .iter()
                .find(|p| p["id"] == node["id"])
                .ok_or("missing resolved package")?;
            if package["source"].is_null() {
                continue;
            }
            let checksum = lock["package"]
                .as_array()
                .ok_or("missing locked packages")?
                .iter()
                .find(|p| {
                    p["name"].as_str() == package["name"].as_str()
                        && p["version"].as_str() == package["version"].as_str()
                })
                .and_then(|p| p.get("checksum"))
                .and_then(toml::Value::as_str)
                .ok_or("registry checksum missing")?;
            inventory.push(json!({"target":target,"name":package["name"],"version":package["version"],
                "source":package["source"],"checksum":checksum,"license":package["license"],
                "repository":package["repository"],"features":node["features"],"dependencies":node["deps"]}));
            if noticed.insert(
                package["id"]
                    .as_str()
                    .ok_or("missing dependency id")?
                    .to_owned(),
            ) {
                write_notices(package, &notices, &mut notice_text)?;
            }
        }
    }
    fs::write(
        destination.join("dependency-inventory.json"),
        serde_json::to_vec_pretty(&json!({
            "schema":1,"source_commit":output("git", &["rev-parse","HEAD"] )?.trim(),
            "rustc":output("rustc", &["-vV"] )?,"scope":"resolved production and tooling graphs; not a release SBOM",
            "dependencies":inventory
        }))?,
    )?;
    fs::write(notices.join("NOTICES.txt"), notice_text)?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unrecorded_suppressions_fail_and_expiry_is_not_optional() {
        let mut suppressions = BTreeSet::new();
        suppressions.insert(("advisory".into(), "RUSTSEC-2026-0000".into()));
        assert!(
            validate(
                &Exceptions { exceptions: vec![] },
                "2026-10-20",
                &suppressions
            )
            .is_err()
        );
        let mut records = Exceptions {
            exceptions: vec![Exception {
                kind: "advisory".into(),
                selector: "RUSTSEC-2026-0000".into(),
                owner: "synthetic owner".into(),
                reason: "synthetic policy test".into(),
                tracking: "https://example.invalid/review".into(),
                expires: "2026-10-20".into(),
            }],
        };
        assert!(validate(&records, "2026-10-20", &suppressions).is_err());
        records.exceptions[0].expires = "2026-10-21".into();
        assert!(validate(&records, "2026-10-20", &suppressions).is_ok());
        records.exceptions[0].expires = "2026-02-31".into();
        assert!(validate(&records, "2026-01-01", &suppressions).is_err());
    }
}

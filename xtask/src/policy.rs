use crate::{Result, cargo, output};
use serde::Deserialize;
#[derive(Debug, Deserialize)]
struct Metadata {
    workspace_members: Vec<String>,
    packages: Vec<Package>,
}
#[derive(Debug, Deserialize)]
struct Package {
    id: String,
    name: String,
    dependencies: Vec<Dependency>,
}
#[derive(Debug, Deserialize)]
struct Dependency {
    name: String,
    kind: Option<String>,
}
fn allowed(from: &str, to: &str, dev: bool) -> bool {
    if dev && to == "librairii-testkit" {
        return true;
    }
    let allowed: &[&str] = match from {
        "librairii-core" => &["uuid", "thiserror"],
        "librairii-app" => &["librairii-core"],
        "librairii-storage" | "librairii-adapters" | "librairii-ui" => {
            &["librairii-app", "librairii-core"]
        }
        "librairii-desktop" => &[
            "librairii-app",
            "librairii-storage",
            "librairii-adapters",
            "librairii-ui",
        ],
        "librairii-testkit" => return !to.starts_with("librairii-"),
        "xtask" => return !to.starts_with("librairii-"),
        _ => return false,
    };
    // Core production dependencies are deliberately effect-free and explicitly bounded.
    // Other layers may add reviewed external effects without reversing workspace edges.
    allowed.contains(&to)
        || (from != "librairii-core" && !to.starts_with("librairii-"))
        || (dev && !to.starts_with("librairii-"))
}
pub fn boundaries() -> Result {
    let metadata: Metadata = serde_json::from_str(&output(
        &cargo(),
        &["metadata", "--format-version", "1", "--locked", "--no-deps"],
    )?)?;
    for package in metadata
        .packages
        .iter()
        .filter(|p| metadata.workspace_members.contains(&p.id))
    {
        for dep in &package.dependencies {
            if !allowed(&package.name, &dep.name, dep.kind.as_deref() == Some("dev")) {
                return Err(format!(
                    "forbidden dependency edge: {} -> {} ({:?})",
                    package.name, dep.name, dep.kind
                )
                .into());
            }
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn production_direction_rejects_toolkit_database_and_testkit_in_core() {
        for dependency in [
            "rusqlite",
            "slint",
            "reqwest",
            "librairii-app",
            "librairii-testkit",
        ] {
            assert!(!allowed("librairii-core", dependency, false));
        }
        assert!(!allowed("librairii-app", "librairii-ui", false));
        assert!(!allowed("librairii-ui", "librairii-storage", false));
        assert!(allowed("librairii-ui", "librairii-app", false));
        assert!(allowed("librairii-core", "librairii-testkit", true));
    }
}

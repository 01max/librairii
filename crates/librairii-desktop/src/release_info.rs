//! Embedded product identity, available before native profile or UI initialization.
use serde_json::{Value, json};

pub const VERSION: &str = include_str!("../../../VERSION");

/// Release orchestration supplies and verifies these compile-time identities.
/// Ordinary development builds honestly report absent release fingerprints.
pub fn build_info() -> Value {
    json!({
        "format": "librairii.native-build",
        "version": VERSION.trim(),
        "source_commit": option_env!("LIBRAIRII_RELEASE_COMMIT"),
        "source_tree": option_env!("LIBRAIRII_RELEASE_TREE"),
        "cargo_lock_sha256": option_env!("LIBRAIRII_RELEASE_LOCK_SHA256"),
        "target": option_env!("LIBRAIRII_RELEASE_TARGET"),
        "arch": std::env::consts::ARCH,
        "os": std::env::consts::OS,
        "debug_assertions": cfg!(debug_assertions),
        "runtime": "Rust/Slint",
    })
}

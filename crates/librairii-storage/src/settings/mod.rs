//! Explicit setup/relocation inputs; profile data alone never grants root authority.
pub(crate) mod target;
pub use target::TargetPreview;
pub(crate) mod manifest;
pub use manifest::{ManifestSummary, RelocationManifest};
mod startup;
pub use startup::{
    AdoptionSetupFailure, Startup, confirm_adoption, default_legacy_root, inspect_startup,
};

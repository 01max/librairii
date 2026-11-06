//! Closed synthetic native library fixture, used only by storage tests.
use std::path::Path;
pub(crate) fn native_copy() -> std::result::Result<
    (
        librairii_testkit::TestRoots,
        crate::adoption::IndependentCopy,
        crate::profile::ProfileLocations,
    ),
    Box<dyn std::error::Error>,
> {
    use crate::{
        adoption::{ClosedLibraryInput, ClosedManifest},
        profile::{Profile, ProfileDocument, ProfileLocations},
    };
    let roots = librairii_testkit::TestRoots::new()?;
    let source = roots.library().join("closed-source");
    std::fs::create_dir(&source)?;
    let bytes = include_bytes!("../../../preservation/corpus/profiles/library-manifest.json");
    let corpus =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../preservation/corpus/libraries/schema018");
    let manifest: serde_json::Value = serde_json::from_slice(bytes)?;
    for entry in manifest.as_array().ok_or("manifest")? {
        let name = entry["path"].as_str().ok_or("path")?;
        if entry["directory"].as_bool() == Some(true) {
            std::fs::create_dir(source.join(name))?;
        } else {
            std::fs::copy(corpus.join(name), source.join(name))?;
        }
    }
    let profile = Profile {
        version: 1,
        language: "fr".into(),
        root: source.to_str().ok_or("source")?.into(),
        library_id: "a0000000-0000-4000-8000-000000000001".into(),
        complete: true,
        recovering: false,
    };
    let input = ClosedLibraryInput::verify(
        &source,
        ProfileDocument::decode(&serde_json::to_vec(&profile)?)?,
        ClosedManifest::decode(bytes)?,
        &Default::default(),
    )?;
    let locations = ProfileLocations::for_host(
        &roots.profile().join("home"),
        Some(&roots.profile().join("config")),
        Some(&roots.library().join("data")),
    )?;
    std::fs::create_dir_all(&locations.data)?;
    std::fs::create_dir_all(locations.native.parent().ok_or("native parent")?)?;
    let copy = input.copy_to_new_root(&locations.data.join("copy"), &Default::default())?;
    Ok((roots, copy, locations))
}

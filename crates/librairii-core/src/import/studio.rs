use super::{
    FailureCause, ImportIdentity, Inventory, Limits, ValidationCode, ValidationError,
    validate_entry_name,
};

/// Borrowed, supplied decoded stage fields in source order, not parsed JSON.
/// Empty individual nodes and repeated references are permitted by frozen Go.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StageReferences<'a> {
    pub image: &'a str,
    pub audio: &'a str,
}

/// Validate supplied STUdio UUID, ordered references and regular-thumbnail
/// presence only. The caller supplies the same valid limits as its inventory.
/// This is downstream of supplied dispatch/decoded facts; it does not select
/// STUdio when story.json is absent or interpret any JSON or image bytes.
/// Success is identity/policy data, never an inspection or custody receipt.
pub fn validate_studio_policy(
    inventory: &Inventory,
    raw_uuid: &str,
    nodes: &[StageReferences<'_>],
    limits: Limits,
) -> Result<ImportIdentity, ValidationError> {
    // Real inspection validates limits before reading any metadata. No actual
    // metadata/artwork byte cap or pre-allocation bound is enforced here.
    limits.validate()?;
    let identity = ImportIdentity::parse_text(raw_uuid.as_bytes()).map_err(|mut error| {
        error.entry = Some(b"story.json".to_vec());
        // The existing parser does not model google/uuid's diagnostic text.
        // Do not invent a source cause for either malformed or nil UUIDs.
        error
    })?;
    validate_references(inventory, nodes, limits)?;
    if !inventory.has_file("thumbnail.png") {
        return Err(ValidationError::at(
            ValidationCode::MissingEntry,
            b"thumbnail.png",
        ));
    }
    Ok(identity)
}

fn validate_references(
    inventory: &Inventory,
    nodes: &[StageReferences<'_>],
    limits: Limits,
) -> Result<(), ValidationError> {
    let mut has_reference = false;
    for node in nodes {
        for reference in [node.image, node.audio] {
            if reference.is_empty() {
                continue;
            }
            has_reference = true;
            let clean = validate_entry_name(reference.as_bytes(), limits).map_err(|cause| {
                ValidationError::at(ValidationCode::UnsafePath, reference)
                    .with_cause(FailureCause::Inspection(cause))
            })?;
            if !clean.starts_with("assets/") {
                return Err(ValidationError::at(ValidationCode::UnsafePath, reference));
            }
            // Go validates the cleaned assets/ prefix but calls has(reference)
            // with the ORIGINAL string against the cleaned archive index.
            if !inventory.has_file(reference) {
                return Err(ValidationError::at(ValidationCode::MissingAsset, reference));
            }
        }
    }
    if !has_reference {
        return Err(ValidationError::at(
            ValidationCode::MissingAsset,
            b"stageNodes",
        ));
    }
    Ok(())
}

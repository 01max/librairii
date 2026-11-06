use super::{ArchiveFormat, ValidationCode, ValidationError, suffix_matches};

/// Supplied container observation, not a constructor or magic-byte verifier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContainerKind {
    Zip,
    SevenZip,
    Other,
}

/// Supplied root-file presence. The future adapter must validate the inventory
/// before constructing these facts; this structure itself certifies nothing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContentFacts {
    pub container: ContainerKind,
    pub has_story_json: bool,
    pub has_uuid_bin: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PackStructure {
    Plain,
    UuidRoot,
    Studio,
}

/// Required future verification, never evidence that verification already ran.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntegrityRequirement {
    AllZipPayloads,
    SelectedSevenZipPayloads,
}

/// A policy plan over facts only. It conveys no stage, decoded payload, import
/// commitment, publication, compensation or cleanup authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArchivePlan {
    pub format: ArchiveFormat,
    pub structure: PackStructure,
    pub integrity: IntegrityRequirement,
}

/// Preserve frozen suffix and content precedence without opening a container.
pub fn dispatch(filename: &[u8], facts: ContentFacts) -> Result<ArchivePlan, ValidationError> {
    let seven = suffix_matches(filename, ".7z");
    let zip = suffix_matches(filename, ".zip");
    if !seven && !zip && !suffix_matches(filename, ".pk") {
        return Err(ValidationError::new(ValidationCode::UnsupportedFormat));
    }
    let expected = if seven {
        ContainerKind::SevenZip
    } else {
        ContainerKind::Zip
    };
    if facts.container != expected {
        return Err(ValidationError::new(ValidationCode::InvalidContainer));
    }
    let integrity = if seven {
        IntegrityRequirement::SelectedSevenZipPayloads
    } else {
        IntegrityRequirement::AllZipPayloads
    };
    let (format, structure) = if facts.has_story_json {
        if !seven && !zip {
            return Err(ValidationError::new(ValidationCode::UnsupportedFormat));
        }
        (
            if seven {
                ArchiveFormat::SevenZip
            } else {
                ArchiveFormat::StudioZip
            },
            PackStructure::Studio,
        )
    } else if !seven && (suffix_matches(filename, ".plain.pk") || facts.has_uuid_bin) {
        (ArchiveFormat::PlainPk, PackStructure::Plain)
    } else {
        let format = if seven {
            ArchiveFormat::SevenZip
        } else if suffix_matches(filename, ".v1.pk") {
            ArchiveFormat::V1Pk
        } else if suffix_matches(filename, ".v2.pk") {
            ArchiveFormat::V2Pk
        } else if zip {
            ArchiveFormat::Zip
        } else {
            ArchiveFormat::GenericPk
        };
        (format, PackStructure::UuidRoot)
    };
    Ok(ArchivePlan {
        format,
        structure,
        integrity,
    })
}

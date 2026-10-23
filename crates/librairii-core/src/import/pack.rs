use super::{ArchiveFormat, Inventory, ValidationCode, ValidationError};
use crate::StoryUuid;
use std::collections::BTreeMap;

/// Exact supplied UUID representation, kept apart from canonical diagnostic text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UuidSource {
    Text(Vec<u8>),
    PlainBytes([u8; 16]),
}

/// Import-local nonnil UUID. This is only validation of supplied identity bytes;
/// it proves neither their archive origin nor a committed/custody relationship.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportIdentity {
    raw: UuidSource,
    typed: StoryUuid,
}
impl ImportIdentity {
    /// Match frozen google/uuid v1.6.0 Parse, not shared StoryUuid parsing.
    /// Its 38-byte form examines only the middle36 bytes, without validating
    /// delimiter characters. Preserve this legacy behavior and original bytes.
    pub fn parse_text(raw: &[u8]) -> Result<Self, ValidationError> {
        let invalid = || ValidationError::new(ValidationCode::InvalidUuid);
        let text = match raw.len() {
            32 | 36 => raw,
            38 => &raw[1..37],
            45 if raw[..9].eq_ignore_ascii_case(b"urn:uuid:") => &raw[9..],
            _ => return Err(invalid()),
        };
        let mut bytes = [0u8; 16];
        if text.len() == 32 {
            for (byte, pair) in bytes.iter_mut().zip(text.as_chunks::<2>().0) {
                *byte = hex_pair(pair[0], pair[1]).ok_or_else(invalid)?;
            }
        } else {
            if [8, 13, 18, 23].iter().any(|i| text[*i] != b'-') {
                return Err(invalid());
            }
            for (byte, position) in bytes
                .iter_mut()
                .zip([0, 2, 4, 6, 9, 11, 14, 16, 19, 21, 24, 26, 28, 30, 32, 34])
            {
                *byte = hex_pair(text[position], text[position + 1]).ok_or_else(invalid)?;
            }
        }
        Self::new(UuidSource::Text(raw.to_vec()), bytes)
    }
    pub fn from_plain_bytes(raw: &[u8]) -> Result<Self, ValidationError> {
        let bytes: [u8; 16] = raw
            .try_into()
            .map_err(|_| ValidationError::at(ValidationCode::InvalidUuid, b"uuid.bin"))?;
        Self::new(UuidSource::PlainBytes(bytes), bytes)
            .map_err(|_| ValidationError::at(ValidationCode::InvalidUuid, b"uuid.bin"))
    }
    fn new(raw: UuidSource, bytes: [u8; 16]) -> Result<Self, ValidationError> {
        if bytes == [0; 16] {
            return Err(ValidationError::new(ValidationCode::InvalidUuid));
        }
        Ok(Self {
            raw,
            typed: StoryUuid::from_bytes(bytes),
        })
    }
    pub const fn raw(&self) -> &UuidSource {
        &self.raw
    }
    pub const fn typed(&self) -> StoryUuid {
        self.typed
    }
    /// Canonical Go inspection text, not a replacement for stored/raw spelling.
    pub fn canonical(&self) -> String {
        self.typed.to_string()
    }
}

fn hex_pair(high: u8, low: u8) -> Option<u8> {
    fn nibble(value: u8) -> Option<u8> {
        match value {
            b'0'..=b'9' => Some(value - b'0'),
            b'a'..=b'f' => Some(value - b'a' + 10),
            b'A'..=b'F' => Some(value - b'A' + 10),
            _ => None,
        }
    }
    Some(nibble(high)? * 16 + nibble(low)?)
}

/// Caller-supplied candidate data, not an inspection result or committed receipt.
/// Metadata presence/raw payload is preserved without JSON/image interpretation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CandidateData {
    pub original_filename: Vec<u8>,
    pub identity: ImportIdentity,
    pub format: ArchiveFormat,
    pub metadata_payload: Option<Vec<u8>>,
}

/// Check required plain entries only; UUID payload reading/integrity is external.
pub fn validate_plain_entries(inventory: &Inventory) -> Result<(), ValidationError> {
    for name in ["uuid.bin", "ni", "li.plain", "ri.plain", "si.plain"] {
        if !inventory.has_file(name) {
            return Err(ValidationError::at(ValidationCode::MissingEntry, name));
        }
    }
    for prefix in ["rf", "sf"] {
        if !inventory.has_file_below(prefix) {
            return Err(ValidationError::at(
                ValidationCode::MissingEntry,
                format!("{prefix}/"),
            ));
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackRoot {
    pub raw_name: String,
    pub identity: ImportIdentity,
}

/// Detect roots from supplied regular-file names with frozen precedence. Two
/// raw root spellings remain ambiguous even when their typed UUIDs are equal.
pub fn validate_uuid_root_entries(inventory: &Inventory) -> Result<PackRoot, ValidationError> {
    let mut roots = BTreeMap::new();
    for name in inventory.file_names() {
        if let Some((root, _)) = name.split_once('/')
            && let Ok(identity) = ImportIdentity::parse_text(root.as_bytes())
        {
            roots.insert(root, identity);
        }
    }
    if roots.is_empty() {
        return Err(ValidationError::new(ValidationCode::InvalidUuid));
    }
    if roots.len() != 1 {
        return Err(ValidationError::new(ValidationCode::AmbiguousStructure));
    }
    let Some((root, identity)) = roots.into_iter().next() else {
        return Err(ValidationError::new(ValidationCode::InvalidUuid));
    };
    for leaf in ["ni", "li", "ri", "si"] {
        let name = format!("{root}/{leaf}");
        if !inventory.has_file(&name) {
            return Err(ValidationError::at(ValidationCode::MissingEntry, name));
        }
    }
    for leaf in ["rf", "sf"] {
        let prefix = format!("{root}/{leaf}");
        if !inventory.has_file_below(&prefix) {
            return Err(ValidationError::at(
                ValidationCode::MissingEntry,
                format!("{prefix}/"),
            ));
        }
    }
    Ok(PackRoot {
        raw_name: root.to_owned(),
        identity,
    })
}

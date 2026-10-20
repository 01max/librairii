// Copyright 2010 The Go Authors. All rights reserved.
// Use of this source code is governed by a BSD-style
// license that can be found in the LICENSE file.
//
// Changed representation: fixed STUdio string fields, ordered members, saved
// type error and initialized cells/logical length, without Go reflection/runtime.
// Original-to-derivative selections/headers/notices are in source-manifest.json.

//! Private supplied-byte JSONv1 decoder, not an archive or generic JSON API.

mod scanner;
mod strings;
mod text;

pub use text::MetadataText;

use super::{
    ImportIdentity, Inventory, Limits, StageReferences, ValidationCode, ValidationError,
    validate_studio_policy,
};
use scanner::{Cursor, Kind, Span};

#[derive(Clone, Debug, Eq, PartialEq)]
enum DecodeFailure {
    Syntax { offset: usize },
    Type { field: &'static str },
    Trailing { offset: usize },
}

impl std::fmt::Display for DecodeFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Task-local structured diagnostics, NOT Go error text/offset parity.
        match self {
            Self::Syntax { offset } => write!(f, "JSON syntax at byte {offset}"),
            Self::Type { field } => write!(f, "JSON type at {field}"),
            Self::Trailing { offset } => write!(f, "JSON trailing input at byte {offset}"),
        }
    }
}

impl std::error::Error for DecodeFailure {}

#[derive(Debug, Default)]
struct Node {
    image: String,
    audio: String,
}

#[derive(Debug, Default)]
struct Story {
    uuid: String,
    title: String,
    description: String,
    cells: Vec<Node>,
    length: usize,
}

/// Policy/text observations over supplied data. Even regular thumbnail presence
/// does not establish readable PNG bytes, accepted inspection or custody.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StudioProjection {
    pub identity: ImportIdentity,
    pub text: MetadataText,
}

fn save_type(error: &mut Option<DecodeFailure>, field: &'static str) {
    error.get_or_insert(DecodeFailure::Type { field });
}

fn string_field(
    value: Span<'_>,
    target: &mut String,
    field: &'static str,
    error: &mut Option<DecodeFailure>,
) {
    match value.kind {
        Kind::String => *target = strings::unquote(value.raw),
        Kind::Null => {} // Go null into a string is a no-op, even after a duplicate.
        _ => save_type(error, field),
    }
}

fn node(
    value: Span<'_>,
    target: &mut Node,
    error: &mut Option<DecodeFailure>,
) -> Result<(), DecodeFailure> {
    match value.kind {
        Kind::Null => return Ok(()),
        Kind::Object => {}
        _ => {
            save_type(error, "stageNodes");
            return Ok(());
        }
    }
    let mut members = Cursor::new(value);
    while let Some((key, value)) = members.member()? {
        match strings::field(&strings::unquote(key), ["image", "audio"]) {
            Some(0) => string_field(value, &mut target.image, "stageNodes.image", error),
            Some(1) => string_field(value, &mut target.audio, "stageNodes.audio", error),
            _ => {}
        }
    }
    Ok(())
}

fn nodes(
    value: Span<'_>,
    story: &mut Story,
    error: &mut Option<DecodeFailure>,
) -> Result<(), DecodeFailure> {
    match value.kind {
        Kind::Null => {
            story.cells.clear();
            story.length = 0;
            return Ok(());
        }
        Kind::Array => {}
        _ => {
            save_type(error, "stageNodes");
            return Ok(());
        }
    }
    let mut elements = Cursor::new(value);
    let mut length = 0;
    while let Some(value) = elements.element()? {
        if length == story.cells.len() {
            story.cells.push(Node::default());
        }
        node(value, &mut story.cells[length], error)?;
        length += 1;
    }
    // A nonempty shorter duplicate only changes logical length. Go retains
    // initialized hidden cells across later SetLen/Grow. Empty [] resets them.
    if length == 0 {
        story.cells.clear();
    }
    story.length = length;
    Ok(())
}

fn decode(input: &[u8]) -> Result<Story, DecodeFailure> {
    // Decoder.Decode scans COMPLETE FIRST VALUE syntax before reducing types.
    // It does not inspect trailing input before returning a first type error.
    let (first, end) = scanner::value(input, 0)?;
    let mut story = Story::default();
    let mut error = None;
    match first.kind {
        Kind::Null => {}
        Kind::Object => {
            let mut members = Cursor::new(first);
            while let Some((key, value)) = members.member()? {
                match strings::field(
                    &strings::unquote(key),
                    ["uuid", "title", "description", "stageNodes"],
                ) {
                    Some(0) => string_field(value, &mut story.uuid, "uuid", &mut error),
                    Some(1) => string_field(value, &mut story.title, "title", &mut error),
                    Some(2) => {
                        string_field(value, &mut story.description, "description", &mut error)
                    }
                    Some(3) => nodes(value, &mut story, &mut error)?,
                    _ => {}
                }
            }
        }
        _ => save_type(&mut error, "story"),
    }
    if let Some(error) = error {
        return Err(error);
    }
    // Product ensureJSONEnd runs ONLY after successful first Decode. Any
    // second value/garbage is malformed metadata; its exact Go cause is not
    // claimed here (including second-any float overflow versus multiple values).
    let trailing = scanner::whitespace(input, end);
    if trailing != input.len() {
        return Err(DecodeFailure::Trailing { offset: trailing });
    }
    Ok(story)
}

/// Validate already-materialized STUdio JSON bytes followed by supplied entry
/// policy. This is downstream of dispatch; it does not open/read an archive.
/// The byte-length refusal is not a stream limit, decoded-heap/CPU bound, CRC,
/// PNG check, inspection receipt or custody/commit authority.
pub fn validate_studio_json(
    inventory: &Inventory,
    metadata: &[u8],
    limits: Limits,
) -> Result<ImportIdentity, ValidationError> {
    project_studio_json(inventory, metadata, limits).map(|projection| projection.identity)
}

/// Project bounded supplied STUdio bytes after mandatory UUID, ordered references
/// and regular thumbnail PRESENCE policy. Text is boundary-trimmed with frozen
/// Go Unicode15 rules; it has no title fallback or image/inspection authority.
pub fn project_studio_json(
    inventory: &Inventory,
    metadata: &[u8],
    limits: Limits,
) -> Result<StudioProjection, ValidationError> {
    limits.validate()?;
    if i64::try_from(metadata.len()).map_or(true, |length| length > limits.max_metadata_bytes) {
        return Err(ValidationError::at(
            ValidationCode::MetadataLimit,
            b"story.json",
        ));
    }
    let story = decode(metadata).map_err(|error| {
        ValidationError::at(ValidationCode::MalformedMetadata, b"story.json")
            .caused_by(error.to_string())
    })?;
    let references: Vec<_> = story.cells[..story.length]
        .iter()
        .map(|node| StageReferences {
            image: &node.image,
            audio: &node.audio,
        })
        .collect();
    let identity = validate_studio_policy(inventory, &story.uuid, &references, limits)?;
    Ok(StudioProjection {
        identity,
        text: MetadataText::from_fields(&story.title, &story.description),
    })
}

#[cfg(test)]
mod tests;

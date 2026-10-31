// Copyright 2010 The Go Authors. All rights reserved.
// Use of this source code is governed by a BSD-style
// license that can be found in the LICENSE file.
//
// Changed representation: separate fixed three-string optional metadata reducer
// over the SAME private lexical spans/unquote/fold and Decode/EOF ordering.
// No Studio node reducer, Go reflection/runtime or archive/image IO is used.
// Original-to-derivative selections/headers/notices are in source-manifest.json.

use super::{
    Cursor, DecodeFailure, ImportIdentity, Inventory, Kind, Limits, MetadataText, ValidationCode,
    ValidationError, finish_decode, save_type, scanner, string_field, strings,
};

const ENTRY: &str = "_metadata.json";

#[derive(Debug, Default)]
struct OptionalFields {
    uuid: String,
    title: String,
    description: String,
}

fn decode(input: &[u8]) -> Result<OptionalFields, DecodeFailure> {
    let (first, end) = scanner::value(input, 0)?;
    let mut fields = OptionalFields::default();
    let mut error = None;
    match first.kind {
        Kind::Null => {}
        Kind::Object => {
            let mut members = Cursor::new(first);
            while let Some((key, value)) = members.member()? {
                match strings::field(&strings::unquote(key), ["uuid", "title", "description"]) {
                    Some(0) => string_field(value, &mut fields.uuid, "uuid", &mut error),
                    Some(1) => string_field(value, &mut fields.title, "title", &mut error),
                    Some(2) => {
                        string_field(value, &mut fields.description, "description", &mut error)
                    }
                    // stageNodes/image/audio are unknown in THIS shape. Their
                    // values were syntax-checked but impose no Studio types,
                    // asset rules, hidden backing cells or numeric decoding.
                    _ => {}
                }
            }
        }
        _ => save_type(&mut error, "metadata"),
    }
    finish_decode(input, end, error)?;
    Ok(fields)
}

/// Project selected, already-materialized optional _metadata.json bytes against
/// a supplied resolved nonnil identity. None means no selected payload; missing
/// or directory inventory entries also select nothing, even if bytes are given.
/// Some(empty bytes) for a regular entry is malformed, while Some(null/{}) yields
/// Some(empty text). This never substitutes the primary identity/raw spelling.
///
/// Inventory, payload presence and byte length are supplied observations, not
/// proof of a successful archive read, LimitReader/heap bound, CRC, image parse,
/// metadata persistence, inspection or custody. No artwork branch runs here.
pub fn project_optional_metadata(
    inventory: &Inventory,
    payload: Option<&[u8]>,
    primary: &ImportIdentity,
    limits: Limits,
) -> Result<Option<MetadataText>, ValidationError> {
    limits.validate()?;
    if !inventory.has_file(ENTRY) {
        return Ok(None);
    }
    let Some(payload) = payload else {
        return Ok(None);
    };
    if i64::try_from(payload.len()).map_or(true, |length| length > limits.max_metadata_bytes) {
        return Err(ValidationError::at(ValidationCode::MetadataLimit, ENTRY));
    }
    let fields = decode(payload).map_err(|error| {
        ValidationError::at(ValidationCode::MalformedMetadata, ENTRY).caused_by(error.to_string())
    })?;
    if !fields.uuid.is_empty() {
        let invalid = || ValidationError::at(ValidationCode::InvalidUuid, ENTRY);
        // Optional UUID is never trimmed. Local import parsing retains frozen
        // wire forms; nil cannot equal the supplied nonnil primary identity.
        let parsed = ImportIdentity::parse_text(fields.uuid.as_bytes()).map_err(|_| invalid())?;
        if parsed.typed() != primary.typed() {
            return Err(invalid());
        }
    }
    Ok(Some(MetadataText::from_fields(
        &fields.title,
        &fields.description,
    )))
}

#[cfg(test)]
mod tests;

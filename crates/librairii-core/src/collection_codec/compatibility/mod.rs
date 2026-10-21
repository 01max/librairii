//! Private frozen normalization/language implementation of the single codec.
//! Source identities, derived bytes and extraction recipes: source-manifest.json.
mod language;
mod language_data;
mod normalization;
mod normalization_data;
mod unicode_properties;

// Core-private bridge for the frozen wire codec. These are not shared
// ports and do not activate schema-open projection.
pub(super) use language::{LanguageError, canonical as canonical_language};
pub(super) use normalization::{NormalizeError, ascii_field_key, lowercase_key, normalize};

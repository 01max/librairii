//! Private transitional compatibility implementation. Future section7 extracts
//! this SINGLE implementation through owner5; it must not grow a second codec.
//! Source identities, derived bytes and extraction recipes: source-manifest.json.
mod language;
mod language_data;
mod normalization;
mod normalization_data;
mod unicode_properties;

// Storage-private bridge for the transitional wire codec. These are not shared
// ports and do not activate schema-open projection.
pub(super) use language::{LanguageError, canonical as canonical_language};
pub(super) use normalization::{NormalizeError, ascii_field_key, lowercase_key, normalize};

// Copyright 2013 The Go Authors. All rights reserved.
// Copyright 2015 The Go Authors. All rights reserved.
// Copyright 2018 The Go Authors. All rights reserved.
// Use of this source code is governed by a BSD-style
// license that can be found in the LICENSE file.
//
// PRIVATE DERIVATIVE: frozen CLDR32 Default=Deprecated|Legacy, within the
// product's35-byte trimmed language boundary. No SuppressScript/Macro/CLDR,
// maximizer, country/TLD API or ccTLD bit is implemented. Precise source hashes,
// changed representation, recipe and notices are in source-manifest.json.
use super::{language_data as data, normalization::trim_space};

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub(super) enum LanguageError {
    #[error("saved query language is invalid")]
    Invalid,
    #[error("frozen language data/algorithm invariant failed")]
    FrozenData,
}
type Result<T> = std::result::Result<T, LanguageError>;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Core {
    language: u16,
    script: u16,
    region: u16,
}

fn language_id(value: &str) -> Result<u16> {
    let i = data::LANGUAGE_IDS
        .binary_search_by_key(&value, |&(key, _)| key)
        .map_err(|_| LanguageError::Invalid)?;
    Ok(data::LANGUAGE_IDS[i].1)
}

fn slot(index: &[u8], id: u16) -> Result<&str> {
    let offset = usize::from(id) * 4;
    std::str::from_utf8(
        index
            .get(offset..offset + 4)
            .ok_or(LanguageError::FrozenData)?,
    )
    .map_err(|_| LanguageError::FrozenData)
}

fn language_name(id: u16) -> Result<String> {
    if id == 0 {
        return Ok("und".to_owned());
    }
    if id >= data::LANG_NO_INDEX_OFFSET {
        let number = id - data::LANG_NO_INDEX_OFFSET;
        return Ok([676, 26, 1]
            .map(|divisor| char::from(b'a' + (number / divisor % 26) as u8))
            .iter()
            .collect());
    }
    let value = slot(data::LANG, id)?;
    Ok(value[..if value.as_bytes()[3] == 0 { 3 } else { 2 }].to_owned())
}

fn script_id(value: &str) -> Result<u16> {
    if value.len() != 4 || !value.bytes().all(|c| c.is_ascii_alphabetic()) {
        return Err(LanguageError::Invalid);
    }
    let mut key = value.to_ascii_lowercase();
    key[..1].make_ascii_uppercase();
    data::SCRIPTS
        .as_chunks::<4>()
        .0
        .iter()
        .position(|entry| entry == key.as_bytes())
        .and_then(|i| u16::try_from(i).ok())
        .ok_or(LanguageError::Invalid)
}

fn region_id(value: &str) -> Result<u16> {
    if value.len() == 3 && value.bytes().all(|c| c.is_ascii_digit()) {
        let number = value.parse::<u16>().map_err(|_| LanguageError::Invalid)?;
        let i = data::NUMERIC_REGIONS
            .binary_search_by_key(&number, |&(number, _)| number)
            .map_err(|_| LanguageError::Invalid)?;
        return Ok(data::NUMERIC_REGIONS[i].1);
    }
    if !value.bytes().all(|c| c.is_ascii_alphabetic()) {
        return Err(LanguageError::Invalid);
    }
    let key = value.to_ascii_uppercase();
    if value.len() == 2 {
        let i = data::REGION_ISO
            .as_chunks::<4>()
            .0
            .iter()
            .position(|entry| entry[..2] == *key.as_bytes())
            .ok_or(LanguageError::Invalid)?;
        return u16::try_from(i + 32).map_err(|_| LanguageError::FrozenData);
    }
    if value.len() == 3 {
        if let Some(i) = data::REGION_ISO
            .as_chunks::<4>()
            .0
            .iter()
            .position(|entry| [entry[0], entry[2], entry[3]] == *key.as_bytes())
        {
            return u16::try_from(i + 32).map_err(|_| LanguageError::FrozenData);
        }
        if let Some(i) = data::ALT_REGION_ISO3
            .as_chunks::<3>()
            .0
            .iter()
            .position(|entry| entry == key.as_bytes())
        {
            return data::ALT_REGION_IDS
                .get(i)
                .copied()
                .ok_or(LanguageError::FrozenData);
        }
    }
    Err(LanguageError::Invalid)
}

fn region_name(id: u16) -> Result<String> {
    if id == 0 {
        return Ok("ZZ".to_owned());
    }
    if id < 32 {
        return Ok(format!(
            "{:03}",
            data::REGION_M49
                .get(usize::from(id))
                .ok_or(LanguageError::FrozenData)?
        ));
    }
    Ok(slot(data::REGION_ISO, id - 32)?[..2].to_owned())
}

impl Core {
    fn render(self) -> Result<String> {
        let mut output = language_name(self.language)?;
        if self.script != 0 {
            output.push('-');
            output.push_str(slot(data::SCRIPTS, self.script)?);
        }
        if self.region != 0 {
            output.push('-');
            output.push_str(&region_name(self.region)?);
        }
        Ok(output)
    }

    fn canonicalize_default(&mut self) {
        while let Ok(i) =
            data::LANGUAGE_ALIASES.binary_search_by_key(&self.language, |&(id, _, _)| id)
        {
            let (_, replacement, kind) = data::LANGUAGE_ALIASES[i];
            match kind {
                0 => {
                    if self.language == 0x310 && self.region == 0 {
                        // frozen _mo
                        self.region = 189; // frozen _MD
                    }
                    self.language = replacement;
                    continue;
                }
                2 => {
                    if self.language == 0x407 && self.script == 0 {
                        // frozen _sh
                        self.script = 91; // frozen _Latn
                    }
                    self.language = replacement;
                }
                // Macro canonicalization is not in Go Default.
                _ => (),
            }
            break;
        }
        if self.script == 157 {
            // Qaai
            self.script = 255; // Zinh
        }
        if let Ok(i) = data::REGION_ALIASES.binary_search_by_key(&self.region, |&(id, _)| id) {
            self.region = data::REGION_ALIASES[i].1;
        }
    }

    fn exact_compact(self) -> Option<Self> {
        if self.language > data::LANG_NO_INDEX_OFFSET {
            return None;
        }
        let key = (u32::from(self.language) << 20)
            | (u32::from(self.script) << 12)
            | u32::from(self.region);
        let index = data::COMPACT_CORES.binary_search(&key).ok()?;
        let encoded = data::COMPACT_CORES[index];
        // Reproduce the selected Go compact representation's exact bit
        // widths; do not assume the original scriptID fits its eight bits.
        Some(Self {
            language: (encoded >> 20) as u16,
            script: ((encoded >> 12) & 255) as u16,
            region: (encoded & 1023) as u16,
        })
    }
}

/// Validate when the frozen scanner actually visits a token. A transform
/// extension can truncate an unconsumed suffix; globally validating its unseen
/// suffix would change Go's successful/refused boundary. No error recovery is
/// needed here: the product rejects ANY scanner/ValueError/duplicate-key error.
struct Tokens<'a> {
    values: Vec<&'a str>,
    position: usize,
}
impl<'a> Tokens<'a> {
    fn new(value: &'a str) -> Self {
        Self {
            values: value.split('-').collect(),
            position: 0,
        }
    }
    fn peek(&self) -> Result<Option<&'a str>> {
        let Some(&value) = self.values.get(self.position) else {
            return Ok(None);
        };
        if value.is_empty() || value.len() > 8 || !value.bytes().all(|c| c.is_ascii_alphanumeric())
        {
            return Err(LanguageError::Invalid);
        }
        Ok(Some(value))
    }
    fn take(&mut self) -> Result<&'a str> {
        let value = self.peek()?.ok_or(LanguageError::Invalid)?;
        self.position += 1;
        self.peek()?;
        Ok(value)
    }
}

#[derive(Debug)]
struct ParsedCore {
    core: Core,
    variants: Vec<&'static str>,
    raw_extlangs: Vec<String>,
}
impl ParsedCore {
    fn render(&self) -> Result<String> {
        let mut output = language_name(self.core.language)?;
        for extlang in &self.raw_extlangs {
            output.push('-');
            output.push_str(extlang);
        }
        if self.core.script != 0 {
            output.push('-');
            output.push_str(slot(data::SCRIPTS, self.core.script)?);
        }
        if self.core.region != 0 {
            output.push('-');
            output.push_str(&region_name(self.core.region)?);
        }
        for variant in &self.variants {
            output.push('-');
            output.push_str(variant);
        }
        Ok(output)
    }
}

fn parse_core(tokens: &mut Tokens<'_>, normalize_extlangs: bool) -> Result<ParsedCore> {
    let base = tokens.peek()?.ok_or(LanguageError::Invalid)?;
    if !(2..=3).contains(&base.len()) || !base.bytes().all(|c| c.is_ascii_alphabetic()) {
        return Err(LanguageError::Invalid);
    }
    let mut core = Core {
        language: language_id(base)?,
        ..Core::default()
    };
    tokens.take()?;
    let mut extlangs = Vec::new();
    while let Some(value) = tokens.peek()? {
        if value.len() != 3 || !value.as_bytes()[0].is_ascii_alphabetic() {
            break;
        }
        if normalize_extlangs {
            let id = language_id(value)?;
            if id != 0 {
                core.language = id;
            }
        } else {
            extlangs.push(value.to_owned());
        }
        tokens.take()?;
    }
    if let Some(value) = tokens.peek()?
        && value.len() == 4
        && value.as_bytes()[0].is_ascii_alphabetic()
    {
        core.script = script_id(value)?;
        tokens.take()?;
    }
    if let Some(value) = tokens.peek()?
        && (2..=3).contains(&value.len())
    {
        core.region = region_id(value)?;
        tokens.take()?;
    }
    let mut variants = Vec::new();
    while let Some(value) = tokens.peek()? {
        if value.len() < 4 {
            break;
        }
        let i = data::VARIANTS
            .binary_search_by_key(&value, |&(key, _)| key)
            .map_err(|_| LanguageError::Invalid)?;
        variants.push(data::VARIANTS[i]);
        tokens.take()?;
    }
    variants.sort_by_key(|&(_, id)| id);
    variants.dedup_by_key(|entry| entry.1);
    Ok(ParsedCore {
        core,
        variants: variants.into_iter().map(|(key, _)| key).collect(),
        raw_extlangs: extlangs,
    })
}

fn parse_extensions(tokens: &mut Tokens<'_>) -> Result<Vec<String>> {
    let mut extensions = Vec::new();
    while let Some(value) = tokens.peek()? {
        if value.len() != 1 {
            break;
        }
        let kind = tokens.take()?;
        let mut output = vec![kind.to_owned()];
        match kind {
            "x" => {
                while tokens.peek()?.is_some() {
                    output.push(tokens.take()?.to_owned());
                }
            }
            "u" => {
                let mut attributes = Vec::new();
                while let Some(value) = tokens.peek()? {
                    if value.len() <= 2 {
                        break;
                    }
                    attributes.push(tokens.take()?);
                }
                // Within the product35-byte bound Go sort uses stable
                // insertion sort; the frozen comparator examines3 bytes only.
                attributes.sort_by_key(|value| &value.as_bytes()[..3]);
                output.extend(attributes.into_iter().map(str::to_owned));
                let mut keys = Vec::<Vec<&str>>::new();
                while let Some(value) = tokens.peek()? {
                    if value.len() != 2 {
                        break;
                    }
                    let mut part = vec![tokens.take()?];
                    while let Some(value) = tokens.peek()? {
                        if value.len() <= 2 {
                            break;
                        }
                        part.push(tokens.take()?);
                    }
                    keys.push(part);
                }
                keys.sort_by_key(|part| part[0]);
                for pair in keys.windows(2) {
                    if pair[0][0] == pair[1][0] && pair[0] != pair[1] {
                        return Err(LanguageError::Invalid);
                    }
                }
                keys.dedup();
                output.extend(keys.into_iter().flatten().map(str::to_owned));
            }
            "t" => {
                if let Some(value) = tokens.peek()?
                    && (2..=3).contains(&value.len())
                    && value.as_bytes()[1].is_ascii_alphabetic()
                {
                    output.push(parse_core(tokens, false)?.render()?.to_ascii_lowercase());
                }
                while let Some(value) = tokens.peek()? {
                    if value.len() != 2 || !value.as_bytes()[1].is_ascii_digit() {
                        break;
                    }
                    output.push(tokens.take()?.to_owned());
                    while let Some(value) = tokens.peek()? {
                        if value.len() < 3 {
                            break;
                        }
                        output.push(tokens.take()?.to_owned());
                    }
                }
            }
            _ => {
                while let Some(value) = tokens.peek()? {
                    if value.len() < 2 {
                        break;
                    }
                    output.push(tokens.take()?.to_owned());
                }
            }
        }
        if output.len() == 1 {
            return Err(LanguageError::Invalid);
        }
        extensions.push(output.join("-"));
        if kind == "x" {
            break;
        }
    }
    extensions.sort_by_key(|value| {
        if value.starts_with("x-") {
            b'{'
        } else {
            value.as_bytes()[0]
        }
    });
    Ok(extensions)
}

fn exact_layout(parsed: &ParsedCore, extensions: &[String]) -> Result<Option<Core>> {
    if parsed.variants.is_empty() && extensions.is_empty() {
        return Ok(parsed.core.exact_compact());
    }
    let mut value = parsed.render()?;
    for extension in extensions {
        value.push('-');
        value.push_str(extension);
    }
    Ok(data::COMPACT_SPECIAL
        .split(' ')
        .any(|special| special == value)
        .then_some(parsed.core))
}

/// Match the observable Make→Tag.String round-trip, including -u-rg elision
/// when both exact compact layouts are the same. Inexact cases retain the
/// complete original tag; no parent/maximize approximation is needed or used.
fn compact_round_trip(parsed: &mut ParsedCore, extensions: Vec<String>) -> Result<Vec<String>> {
    let Some(index) = extensions.iter().position(|value| value.starts_with("u-")) else {
        if let Some(decoded) = exact_layout(parsed, &extensions)? {
            parsed.core = decoded;
        }
        return Ok(extensions);
    };
    let original_extension = extensions[index].clone();
    let parts: Vec<_> = original_extension.split('-').collect();
    let Some(key) = parts.iter().position(|&part| part == "rg") else {
        return Ok(extensions);
    };
    let Some(region) = parts.get(key + 1) else {
        return Ok(extensions);
    };
    if region.len() != 6 || !region.ends_with("zzzz") {
        return Ok(extensions);
    }
    let Ok(region_id) = region_id(&region[..2]) else {
        return Ok(extensions);
    };
    // Only the first type is read by TypeForKey, but SetTypeForKey removes
    // all following type subtags with that key.
    let mut end = key + 1;
    while end < parts.len() && parts[end].len() > 2 {
        end += 1;
    }
    let mut remaining = extensions.clone();
    let new_parts: Vec<_> = parts[..key]
        .iter()
        .chain(parts[end..].iter())
        .copied()
        .collect();
    if new_parts.len() == 1 {
        remaining.remove(index);
    } else {
        remaining[index] = new_parts.join("-");
    }
    let Some(language_core) = exact_layout(parsed, &remaining)? else {
        return Ok(extensions);
    };
    let regional = ParsedCore {
        core: Core {
            region: region_id,
            ..parsed.core
        },
        variants: parsed.variants.clone(),
        raw_extlangs: Vec::new(),
    };
    let Some(locale_core) = exact_layout(&regional, &remaining)? else {
        return Ok(extensions);
    };
    parsed.core = language_core;
    if locale_core == language_core {
        return Ok(remaining);
    }
    // The distinct compact locale re-adds the first region type with the
    // canonical two-letter spelling; keep other keys in their parsed order.
    let mut normalized = extensions;
    let mut replaced: Vec<_> = parts[..key].iter().map(|part| (*part).to_owned()).collect();
    replaced.push("rg".to_owned());
    replaced.push(region_name(locale_core.region)?.to_ascii_lowercase() + "zzzz");
    replaced.extend(parts[end..].iter().map(|part| (*part).to_owned()));
    normalized[index] = replaced.join("-");
    Ok(normalized)
}

fn parse_default(value: &str, grandfathered: bool) -> Result<String> {
    if grandfathered
        && let Ok(i) = data::GRANDFATHERED.binary_search_by_key(&value, |&(key, _)| key)
    {
        return parse_default(&data::GRANDFATHERED[i].1.to_ascii_lowercase(), false);
    }
    let mut tokens = Tokens::new(value);
    if tokens.peek()? == Some("x") {
        return Ok(parse_extensions(&mut tokens)?.join("-"));
    }
    let mut parsed = parse_core(&mut tokens, true)?;
    let extensions = if tokens.peek()?.is_some_and(|value| value.len() == 1) {
        parse_extensions(&mut tokens)?
    } else if tokens.peek()?.is_some() {
        return Err(LanguageError::Invalid);
    } else {
        Vec::new()
    };
    parsed.core.canonicalize_default();
    let extensions = compact_round_trip(&mut parsed, extensions)?;
    let mut output = parsed.render()?;
    for extension in extensions {
        output.push('-');
        output.push_str(&extension);
    }
    Ok(output)
}

pub(super) fn canonical(value: &str) -> Result<String> {
    let value = trim_space(value).replace('_', "-");
    if value.is_empty() || value.len() > 35 {
        return Err(LanguageError::Invalid);
    }
    let output = parse_default(&value.to_ascii_lowercase(), true)?;
    if output == "und" {
        return Err(LanguageError::Invalid);
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    use std::{collections::BTreeMap, error::Error};

    type TestResult<T = ()> = std::result::Result<T, Box<dyn Error>>;

    fn frame(hash: &mut Sha256, id: u32, value: &str) -> TestResult {
        hash.update(id.to_le_bytes());
        hash.update(u32::try_from(value.len())?.to_le_bytes());
        hash.update(value.as_bytes());
        Ok(())
    }
    fn hexadecimal(hash: Sha256) -> String {
        librairii_core::Checksum::from_bytes(hash.finalize().into()).to_string()
    }
    fn product_outcome(input: &str) -> TestResult<String> {
        match canonical(input) {
            Ok(value) => Ok(value),
            Err(LanguageError::Invalid) => Ok(String::new()),
            Err(error) => Err(format!("{input:?}: {error}").into()),
        }
    }
    fn strings(value: &serde_json::Value) -> TestResult<Vec<&str>> {
        value
            .as_array()
            .ok_or("seed list missing")?
            .iter()
            .map(|value| value.as_str().ok_or_else(|| "nonstring seed".into()))
            .collect()
    }

    #[derive(Default)]
    struct Group {
        input: Sha256,
        canonical: Sha256,
        count: u32,
        accepted: u32,
    }
    impl Group {
        fn add(&mut self, input: &str) -> TestResult {
            let canonical = product_outcome(input)?;
            frame(&mut self.input, self.count, input)?;
            frame(&mut self.canonical, self.count, &canonical)?;
            self.accepted += u32::from(!canonical.is_empty());
            self.count += 1;
            Ok(())
        }
    }
    fn letters(size: usize, mut add: impl FnMut(String) -> TestResult) -> TestResult {
        for number in 0..26usize.pow(u32::try_from(size)?) {
            let mut number = number;
            let mut key = vec![b'a'; size];
            for character in key.iter_mut().rev() {
                *character += u8::try_from(number % 26)?;
                number /= 26;
            }
            add(String::from_utf8(key)?)?;
        }
        Ok(())
    }

    #[test]
    fn selected_default_cases_match_actual_go_and_dns_omission_reference() -> TestResult {
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("language-reference.json"))?;
        assert_eq!(reference["go"], "go1.26.5");
        assert_eq!(reference["cldr"], "32");
        assert_eq!(reference["dnsOmission"]["equalRawAndProductResults"], true);
        assert_eq!(reference["dnsOmission"]["unchangedMembers"], 484);
        let mut masked = Sha256::new();
        masked.update(data::REGION_TYPES_WITHOUT_TLD);
        assert_eq!(
            reference["dnsOmission"]["regionFlags"]["maskedByteSha256"],
            hexadecimal(masked)
        );
        for case in reference["selected"]
            .as_array()
            .ok_or("selected cases missing")?
        {
            let input = case["input"].as_str().ok_or("selected input missing")?;
            assert_eq!(
                case["canonical"],
                product_outcome(input)?,
                "{input:?}, actualGo={case}"
            );
        }
        Ok(())
    }

    #[test]
    fn bounded_default_grammar_matches_actual_go_without_dns() -> TestResult {
        let seed: serde_json::Value = serde_json::from_str(include_str!("language-cases.json"))?;
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("language-reference.json"))?;
        let mut groups = BTreeMap::from(
            [
                "bases",
                "scripts",
                "regions",
                "compact-rg",
                "variants",
                "random",
                "selected",
            ]
            .map(|name| (name, Group::default())),
        );
        let group = groups.get_mut("bases").ok_or("group missing")?;
        let suffixes = strings(&seed["suffixes"])?;
        for size in [2, 3] {
            letters(size, |base| {
                for suffix in &suffixes {
                    group.add(&(base.to_owned() + suffix))?;
                }
                Ok(())
            })?;
        }
        let group = groups.get_mut("scripts").ok_or("group missing")?;
        letters(4, |script| group.add(&("en-".to_owned() + &script)))?;
        let group = groups.get_mut("regions").ok_or("group missing")?;
        for size in [2, 3] {
            letters(size, |region| group.add(&("en-Latn-".to_owned() + &region)))?;
        }
        for number in 0..1000 {
            group.add(&format!("en-Latn-{number:03}"))?;
        }
        let group = groups.get_mut("compact-rg").ok_or("group missing")?;
        for core in strings(&seed["compactCores"])? {
            letters(2, |region| group.add(&format!("{core}-u-rg-{region}zzzz")))?;
        }
        let group = groups.get_mut("variants").ok_or("group missing")?;
        let variants = strings(&seed["variants"])?;
        for a in &variants {
            for b in &variants {
                group.add(&format!("sl-{a}-{b}"))?;
            }
        }
        let group = groups.get_mut("random").ok_or("group missing")?;
        let tokens = strings(&seed["randomTokens"])?;
        let mut state = 0x7155ac31u32;
        let mut next = || {
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            state
        };
        for _ in 0..10000 {
            let n = next() % 8 + 1;
            let mut parts = Vec::new();
            for _ in 0..n {
                parts.push(tokens[usize::try_from(next())? % tokens.len()]);
            }
            group.add(&parts.join("-"))?;
        }
        let group = groups.get_mut("selected").ok_or("group missing")?;
        for input in strings(&seed["selected"])? {
            group.add(input)?;
        }
        for (name, group) in groups {
            let expected = &reference["groups"][name];
            assert_eq!(expected["count"], group.count, "{name} count");
            assert_eq!(expected["accepted"], group.accepted, "{name} accepted");
            assert_eq!(expected["inputs"], hexadecimal(group.input), "{name} input");
            assert_eq!(
                expected["canonical"],
                hexadecimal(group.canonical),
                "{name} canonical"
            );
        }
        Ok(())
    }

    #[test]
    fn frozen_default_aliases_are_distinct_from_suppress_macro_and_cldr() -> Result<()> {
        for (input, expected) in [
            ("sh", "sr-Latn"),
            ("sh-Cyrl", "sr-Cyrl"),
            ("mo", "ro-MD"),
            ("mo-RO", "ro-RO"),
            ("iw", "he"),
            ("cmn", "cmn"),
            ("nb", "nb"),
            ("en-Latn", "en-Latn"),
            ("en-Qaai", "en-Zinh"),
            (" en_US_POSIX ", "en-US-u-va-posix"),
            ("i-klingon", "tlh"),
            ("und-x-user", "und-x-user"),
            ("x-private", "x-private"),
            ("en-US-u-rg-uszzzz", "en-US"),
            ("de-u-rg-dezzzz", "de-u-rg-dezzzz"),
        ] {
            assert_eq!(canonical(input)?, expected, "{input}");
        }
        for input in [
            "",
            "und",
            "root",
            "notalanguage",
            "en-9999",
            "en-u-ca-gregory-ca-buddhist",
        ] {
            assert_eq!(canonical(input), Err(LanguageError::Invalid), "{input}");
        }
        Ok(())
    }

    #[test]
    fn frozen_language_lookup_retains_non_dns_region_bits_only() {
        assert_eq!(data::REGION_TYPES_WITHOUT_TLD.len(), 359);
        assert!(
            data::REGION_TYPES_WITHOUT_TLD
                .iter()
                .all(|value| value & !5 == 0)
        );
        assert!(
            data::REGION_TYPES_WITHOUT_TLD
                .iter()
                .any(|value| value & 1 != 0)
        );
        assert!(
            data::REGION_TYPES_WITHOUT_TLD
                .iter()
                .any(|value| value & 4 != 0)
        );
        assert_eq!(Core::default().render(), Ok("und".to_owned()));
    }
}

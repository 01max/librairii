// Copyright 2011 The Go Authors. All rights reserved.
// Use of this source code is governed by a BSD-style
// license that can be found in the LICENSE file.
// Copyright 2016 The Go Authors. All rights reserved.
// Copyright 2009 The Go Authors. All rights reserved.
//
// PRIVATE DERIVATIVE of the approved Go1.26.5/xtext0.43.0 Unicode15 sources.
// The exact composition.go/forminfo.go/trie.go/normalize.go and product wrapper
// identities, retained original headers and notice references are in
// source-manifest.json. This is not a current-Unicode library composition.
use super::{normalization_data as data, unicode_properties as unicode};

const MAX_INPUT_BYTES: usize = 262_144; // frozen saved-query wire ceiling
const MAX_FOLDED_BYTES: usize = MAX_INPUT_BYTES * 3;
const MAX_NORMALIZED_BYTES: usize = MAX_FOLDED_BYTES * 33;
const NON_STARTERS: u8 = 30;
const CGJ: char = '\u{034f}';

#[derive(Debug, thiserror::Error)]
enum NormalizeError {
    #[error("saved query input exceeds the frozen wire limit")]
    InputLimit,
    #[error("frozen normalization data/algorithm invariant failed")]
    FrozenData,
}
type Result<T> = std::result::Result<T, NormalizeError>;

fn in_ranges(character: char, ranges: &[(u32, u32, u32)]) -> bool {
    let rune = u32::from(character);
    let i = ranges.partition_point(|&(_, hi, _)| hi < rune);
    ranges
        .get(i)
        .is_some_and(|&(lo, _, stride)| rune >= lo && (rune - lo) % stride == 0)
}
fn is_space(character: char) -> bool {
    in_ranges(character, unicode::WHITE_SPACE)
}

pub(super) fn trim_space(value: &str) -> &str {
    value.trim_matches(is_space)
}

fn folded(value: &str) -> Result<String> {
    let mut output = String::new();
    for character in value.chars() {
        match unicode::FULL_FOLD.binary_search_by_key(&u32::from(character), |&(rune, _)| rune) {
            Ok(i) => output.push_str(unicode::FULL_FOLD[i].1),
            Err(_) => output.push(character),
        }
        if output.len() > MAX_FOLDED_BYTES {
            return Err(NormalizeError::FrozenData);
        }
    }
    Ok(output)
}

#[derive(Clone, Copy, Debug, Default)]
struct Properties {
    ccc: u8,
    leading: u8,
    trailing: u8,
    backward: bool,
    decomposition: Option<&'static str>,
}

/// The selected generated UTF-8 trie, including sparse ranges. Rust char
/// guarantees the full valid UTF-8 precondition of Go lookupStringUnsafe; no
/// unsafe Rust or unchecked indexing is required to translate that algorithm.
fn lookup(character: char) -> Result<u16> {
    let mut encoded = [0; 4];
    let bytes = character.encode_utf8(&mut encoded).as_bytes();
    let get_index = |i: usize| {
        data::INDEX
            .get(i)
            .copied()
            .map(usize::from)
            .ok_or(NormalizeError::FrozenData)
    };
    if bytes.len() == 1 {
        return data::VALUES
            .get(usize::from(bytes[0]))
            .copied()
            .ok_or(NormalizeError::FrozenData);
    }
    let mut index = get_index(usize::from(bytes[0]))?;
    for byte in bytes.iter().take(bytes.len() - 1).skip(1) {
        index = get_index((index << 6) + usize::from(*byte))?;
    }
    let terminal = bytes[bytes.len() - 1];
    if index < 95 {
        return data::VALUES
            .get((index << 6) + usize::from(terminal))
            .copied()
            .ok_or(NormalizeError::FrozenData);
    }
    let offset = usize::from(
        *data::SPARSE_OFFSET
            .get(index - 95)
            .ok_or(NormalizeError::FrozenData)?,
    );
    let &(stride, count, _) = data::SPARSE.get(offset).ok_or(NormalizeError::FrozenData)?;
    let mut lo = offset + 1;
    let mut hi = lo + usize::from(count);
    while lo < hi {
        let middle = lo + (hi - lo) / 2;
        let &(value, start, end) = data::SPARSE.get(middle).ok_or(NormalizeError::FrozenData)?;
        if start <= terminal && terminal <= end {
            return u16::from(terminal - start)
                .checked_mul(stride)
                .and_then(|step| value.checked_add(step))
                .ok_or(NormalizeError::FrozenData);
        }
        if terminal < start {
            hi = middle;
        } else {
            lo = middle + 1;
        }
    }
    Ok(0)
}

fn properties(character: char) -> Result<Properties> {
    let value = lookup(character)?;
    let mut p = Properties::default();
    if value == 0 {
        return Ok(p);
    }
    let mut compressed_ccc = 0;
    if value >= 0x8000 {
        compressed_ccc = (value & 0xff) as u8;
        let flags = ((value >> 8) & 0x3f) as u8;
        p.backward = flags & 0x8 != 0;
        p.trailing = flags & 3;
        if compressed_ccc != 0 || p.backward {
            p.leading = p.trailing;
        }
    } else {
        let index = usize::from(value);
        let header = *data::DECOMPS.get(index).ok_or(NormalizeError::FrozenData)?;
        let flags = ((header & 0xe0) >> 2) | 4;
        p.backward = flags & 8 != 0;
        p.trailing = flags & 3;
        let size = if header & 31 == 31 {
            33
        } else {
            usize::from(header & 31)
        };
        let start = index + 1;
        p.decomposition = Some(
            std::str::from_utf8(
                data::DECOMPS
                    .get(start..start + size)
                    .ok_or(NormalizeError::FrozenData)?,
            )
            .map_err(|_| NormalizeError::FrozenData)?,
        );
        if index >= data::FIRSTCCC {
            // Go compInfo compares the UPDATED offset of the trailing-CCC
            // byte with these boundaries, not the original decomposition ID.
            let offset = start + size;
            let tail = *data::DECOMPS
                .get(offset)
                .ok_or(NormalizeError::FrozenData)?;
            p.trailing |= tail & 3;
            if offset >= data::FIRSTLEADINGCCC {
                p.leading = tail & 3;
                if offset >= data::FIRSTSTARTERWITHNLEAD {
                    // The generated fake decomposition encodes Jamo V/T's
                    // nonstarter accounting despite their zero CCC.
                    p.decomposition = None;
                    p.backward = false;
                    return Ok(p);
                }
                compressed_ccc = *data::DECOMPS
                    .get(offset + 1)
                    .ok_or(NormalizeError::FrozenData)?;
            }
        }
    }
    p.ccc = *data::CCC
        .get(usize::from(compressed_ccc))
        .ok_or(NormalizeError::FrozenData)?;
    Ok(p)
}

#[derive(Debug)]
struct Segment {
    runes: [(char, u8); 32],
    used: usize,
}
impl Segment {
    fn new() -> Self {
        Self {
            runes: [('\0', 0); 32],
            used: 0,
        }
    }
    fn insert(&mut self, character: char, ccc: u8) -> Result<()> {
        if self.used == self.runes.len() {
            return Err(NormalizeError::FrozenData);
        }
        let mut position = self.used;
        if ccc != 0 {
            while position > 0 && self.runes[position - 1].1 > ccc {
                position -= 1;
            }
        }
        self.runes.copy_within(position..self.used, position + 1);
        self.runes[position] = (character, ccc);
        self.used += 1;
        Ok(())
    }
    fn flush(&mut self, output: &mut String) -> Result<()> {
        for &(character, _) in &self.runes[..self.used] {
            output.push(character);
        }
        self.used = 0;
        if output.len() > MAX_NORMALIZED_BYTES {
            return Err(NormalizeError::FrozenData);
        }
        Ok(())
    }
}

/// Exact stream-safe partitioning is applied to ORIGINAL folded characters'
/// generated leading/trailing counts BEFORE decomposition and CCC ordering.
/// Inserting CGJ after globally ordered NFKD is observably different for Mc.
fn nfkd(value: &str) -> Result<String> {
    if value.len() > MAX_FOLDED_BYTES {
        return Err(NormalizeError::InputLimit);
    }
    let mut output = String::new();
    let mut segment = Segment::new();
    let mut nonstarters = 0u8;
    for character in value.chars() {
        let p = properties(character)?;
        if nonstarters + p.leading > NON_STARTERS {
            segment.flush(&mut output)?;
            output.push(CGJ);
            nonstarters = 0;
        }
        nonstarters += p.leading;
        if p.leading == 0 {
            nonstarters = p.trailing;
            segment.flush(&mut output)?;
        }
        let rune = u32::from(character);
        if (0xac00..0xd7a4).contains(&rune) {
            let syllable = rune - 0xac00;
            for code in [0x1100 + syllable / 588, 0x1161 + syllable % 588 / 28] {
                segment.insert(char::from_u32(code).ok_or(NormalizeError::FrozenData)?, 0)?;
            }
            if syllable % 28 != 0 {
                segment.insert(
                    char::from_u32(0x11a7 + syllable % 28).ok_or(NormalizeError::FrozenData)?,
                    0,
                )?;
            }
        } else if let Some(decomposition) = p.decomposition {
            for decomposed in decomposition.chars() {
                let next = properties(decomposed)?;
                if next.ccc == 0 && !next.backward {
                    segment.flush(&mut output)?;
                }
                segment.insert(decomposed, next.ccc)?;
            }
        } else {
            segment.insert(character, p.ccc)?;
        }
    }
    segment.flush(&mut output)?;
    Ok(output)
}

fn normalize(value: &str) -> Result<String> {
    if value.len() > MAX_INPUT_BYTES {
        return Err(NormalizeError::InputLimit);
    }
    let folded = folded(trim_space(value))?;
    let decomposed = nfkd(&folded)?;
    let mut output = String::new();
    let mut pending_space = false;
    for character in decomposed.chars() {
        if in_ranges(character, unicode::MN) {
            continue;
        }
        if is_space(character) {
            pending_space = !output.is_empty();
        } else {
            if pending_space {
                output.push(' ');
                pending_space = false;
            }
            output.push(character);
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    use std::collections::BTreeMap;
    use std::error::Error;

    fn original_ccc(character: char) -> Result<u8> {
        let value = lookup(character)?;
        if value < 0x8000 && usize::from(value) >= data::FIRSTCCCZEROEXCEPT {
            return Ok(0);
        }
        Ok(properties(character)?.ccc)
    }

    fn frame(hash: &mut Sha256, id: u32, value: &[u8]) -> std::result::Result<(), Box<dyn Error>> {
        hash.update(id.to_le_bytes());
        hash.update(u32::try_from(value.len())?.to_le_bytes());
        hash.update(value);
        Ok(())
    }

    #[test]
    fn every_scalar_and_frozen_ccc_boundaries_match_actual_go_reference()
    -> std::result::Result<(), Box<dyn Error>> {
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("normalization-reference.json"))?;
        assert_eq!(reference["go"], "go1.26.5");
        assert_eq!(reference["norm"], "15.0.0");
        assert_eq!(reference["cases"], "15.0.0");
        assert_eq!(reference["unicode"], "15.0.0");
        assert_eq!(reference["provenance"]["originalFiles"], 436);
        assert_eq!(reference["provenance"]["unchangedOriginalFiles"], 436);
        assert_eq!(reference["provenance"]["unchangedZipMembers"], 487);
        let mut scalar = BTreeMap::<&str, Sha256>::from([
            ("fold", Sha256::new()),
            ("nfkd", Sha256::new()),
            ("normalize", Sha256::new()),
            ("ccc", Sha256::new()),
            ("mn", Sha256::new()),
            ("space", Sha256::new()),
        ]);
        let mut representatives = BTreeMap::new();
        let mut contextual_runes = Vec::new();
        let mut count = 0u32;
        for code in 0..=0x10ffff {
            let Some(character) = char::from_u32(code) else {
                continue;
            };
            let value = character.to_string();
            let ccc = original_ccc(character)?;
            let mn = in_ranges(character, unicode::MN);
            let decomposed = nfkd(&value)?;
            let p = properties(character)?;
            if decomposed != value || p.ccc != 0 || p.backward {
                contextual_runes.push(character);
            }
            for (key, data) in [
                ("fold", folded(&value)?.into_bytes()),
                ("nfkd", decomposed.into_bytes()),
                ("normalize", normalize(&value)?.into_bytes()),
                ("ccc", vec![ccc]),
                ("mn", vec![u8::from(mn)]),
                ("space", vec![u8::from(is_space(character))]),
            ] {
                frame(
                    scalar.get_mut(key).ok_or("missing hash metric")?,
                    code,
                    &data,
                )?;
            }
            if ccc != 0 {
                representatives.entry((ccc, mn)).or_insert(character);
            }
            count += 1;
        }
        assert_eq!(reference["scalarCount"], count);
        assert_eq!(reference["representativeCount"], representatives.len());
        assert_eq!(reference["contextualRuneCount"], contextual_runes.len());
        for (key, hash) in scalar {
            assert_eq!(
                reference["scalarDigests"][key],
                librairii_core::Checksum::from_bytes(hash.finalize().into()).to_string(),
                "all scalar {key}"
            );
        }

        let mut sequence = BTreeMap::<&str, Sha256>::from([
            ("input", Sha256::new()),
            ("nfkd", Sha256::new()),
            ("normalize", Sha256::new()),
        ]);
        let mut count = 0u32;
        let mut add = |value: String| -> std::result::Result<(), Box<dyn Error>> {
            for (key, data) in [
                ("nfkd", nfkd(&value)?),
                ("normalize", normalize(&value)?),
                ("input", value),
            ] {
                frame(
                    sequence.get_mut(key).ok_or("missing sequence metric")?,
                    count,
                    data.as_bytes(),
                )?;
            }
            count += 1;
            Ok(())
        };
        for a in representatives.values() {
            for b in representatives.values() {
                for n in [0, 1, 2, 28, 29, 30, 31, 32, 59, 60, 61, 300] {
                    add(format!("a{}{b}{a}Z", a.to_string().repeat(n)))?;
                }
            }
        }
        for r in [
            0x1161, 0x11a8, 0xac00, 0xac01, 0x034f, 0x0344, 0x0f73, 0x16ff0, 0x1715,
        ] {
            let r = char::from_u32(r).ok_or("invalid sequence seed")?;
            for n in [29, 30, 31, 32, 60, 61, 300] {
                add(format!(
                    "a{}\u{0300}\u{0315}\u{0327}Z",
                    r.to_string().repeat(n)
                ))?;
                add(format!("a{}{r}\u{0327}\u{0315}Z", "\u{0300}".repeat(n)))?;
            }
        }
        for r in contextual_runes {
            for n in [29, 30, 31] {
                add(format!("a{}{r}\u{0315}\u{0327}Z", "\u{0300}".repeat(n)))?;
                add(format!("a{r}{}\u{0315}\u{0327}Z", "\u{0300}".repeat(n)))?;
            }
        }
        assert_eq!(reference["sequenceCount"], count);
        for (key, hash) in sequence {
            assert_eq!(
                reference["sequenceDigests"][key],
                librairii_core::Checksum::from_bytes(hash.finalize().into()).to_string(),
                "original-character partition {key}"
            );
        }
        Ok(())
    }

    #[test]
    fn frozen_go_streamsafe_partition_preserves_surviving_mc_order()
    -> std::result::Result<(), Box<dyn Error>> {
        // Actual Go id0, report7c41d90d...e375: CCC6/CCC9 Mc marks survive Mn.
        let input = format!("a{}\u{1715}\u{16ff0}Z", "\u{16ff0}".repeat(29));
        let expected = format!("a{}\u{1715}\u{16ff0}z", "\u{16ff0}".repeat(29));
        assert_eq!(normalize(&input)?, expected);
        assert_eq!(
            nfkd(&input)?,
            format!("a{}\u{1715}\u{034f}\u{16ff0}Z", "\u{16ff0}".repeat(29))
        );
        Ok(())
    }

    #[test]
    fn frozen_unicode_folding_mn_space_and_hangul_keep_separate_semantics()
    -> std::result::Result<(), Box<dyn Error>> {
        assert_eq!(
            normalize(" \u{85}ÉTÉ\u{a0}Straße\u{2003}ﬃ  ")?,
            "ete strasse ffi"
        );
        assert_eq!(
            normalize("Ａngström İ \u{1d165}x")?,
            "angstrom i \u{1d165}x"
        ); // Mc retained
        assert_eq!(nfkd("각")?, "\u{1100}\u{1161}\u{11a8}");
        assert_eq!(
            nfkd(&format!("a{}x", "\u{300}".repeat(31)))?,
            format!("a{}\u{034f}\u{300}x", "\u{300}".repeat(30))
        );
        assert_eq!(normalize(&format!("a{}x", "\u{300}".repeat(31)))?, "ax");
        assert!(matches!(
            normalize(&"x".repeat(MAX_INPUT_BYTES + 1)),
            Err(NormalizeError::InputLimit)
        ));
        Ok(())
    }
}

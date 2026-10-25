//! Qualification comparison only. No production codec or dependency selection.
use std::collections::BTreeMap;
use unicode_general_category::{GeneralCategory, get_general_category};
use unicode_normalization::UnicodeNormalization;

pub type FoldMap = BTreeMap<char, String>;

pub fn fold_map(bytes: &[u8]) -> Result<FoldMap, String> {
    let raw: BTreeMap<String, String> = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    if raw.len() != 1616 {
        return Err("expected the frozen 1616-entry fold map".into());
    }
    raw.into_iter()
        .map(|(key, value)| {
            let mut chars = key.chars();
            let c = chars.next().ok_or("empty fold-map key")?;
            if chars.next().is_some() {
                return Err("fold-map key is not one scalar".into());
            }
            Ok((c, value))
        })
        .collect()
}

// Exact complete White_Space Range16 triplets from Go1.26.5 unicode/tables.go.
// Source SHA256 6a88d48746c4b2c471cc04ae40559694b734770212e6f5ea5f1962adb0c0bdb1.
// Go source terms: BSD-3-Clause; full upstream notices ship beside this probe.
const GO15_SPACE: &[(u32, u32, u32)] = &[
    (0x0009, 0x000d, 1),
    (0x0020, 0x0085, 101),
    (0x00a0, 0x1680, 5600),
    (0x2000, 0x200a, 1),
    (0x2028, 0x2029, 1),
    (0x202f, 0x205f, 48),
    (0x3000, 0x3000, 1),
];

pub fn go15_space(c: char) -> bool {
    let n = u32::from(c);
    GO15_SPACE
        .iter()
        .any(|&(lo, hi, stride)| n >= lo && n <= hi && (n - lo).is_multiple_of(stride))
}

pub fn is_mn(c: char) -> bool {
    get_general_category(c) == GeneralCategory::NonspacingMark
}

pub fn fold(input: &str, map: &FoldMap) -> String {
    let mut result = String::new();
    for c in input.chars() {
        if let Some(mapped) = map.get(&c) {
            result.push_str(mapped);
        } else {
            result.push(c);
        }
    }
    result
}

fn collapse(chars: impl Iterator<Item = char>) -> String {
    let mut result = String::new();
    let mut space = false;
    for c in chars {
        if is_mn(c) {
            continue;
        }
        if go15_space(c) {
            space = !result.is_empty();
        } else {
            if space {
                result.push(' ');
                space = false;
            }
            result.push(c);
        }
    }
    result
}

pub fn normalize(input: &str, map: &FoldMap, stream_safe: bool) -> String {
    let folded = fold(input.trim_matches(go15_space), map);
    if stream_safe {
        collapse(folded.nfkd().stream_safe())
    } else {
        collapse(folded.nfkd())
    }
}

pub fn hex(input: &str) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(input.len() * 2);
    for &b in input.as_bytes() {
        out.push(char::from(DIGITS[usize::from(b >> 4)]));
        out.push(char::from(DIGITS[usize::from(b & 15)]));
    }
    out
}

pub fn unhex(input: &str) -> Result<String, String> {
    if !input.len().is_multiple_of(2) {
        return Err("odd UTF8 hex length".into());
    }
    let mut bytes = Vec::with_capacity(input.len() / 2);
    for pair in input.as_bytes().as_chunks::<2>().0 {
        let nibble = |b: u8| match b {
            b'0'..=b'9' => Ok(b - b'0'),
            b'a'..=b'f' => Ok(b - b'a' + 10),
            _ => Err("invalid lower-case UTF8 hex"),
        };
        bytes.push(nibble(pair[0])? * 16 + nibble(pair[1])?);
    }
    String::from_utf8(bytes).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whitespace_range_strides_do_not_fill_gaps_or_accept_zero_width_space() {
        assert!(go15_space('\u{85}'));
        assert!(go15_space('\u{1680}'));
        assert!(go15_space('\u{205f}'));
        assert!(!go15_space('!'));
        assert!(!go15_space('\u{200b}'));
        assert!(!go15_space('\u{feff}'));
    }

    #[test]
    fn strips_only_mn_and_keeps_spacing_marks_and_pending_space_across_mn() {
        assert_eq!(collapse(" A\u{301}\t\u{301}B\u{301} ".chars()), "A B");
        assert_eq!(collapse("x\u{903}y".chars()), "x\u{903}y");
    }

    #[test]
    fn hex_decoder_refuses_malformed_or_non_utf8_protocol_data() {
        for bad in ["0", "gg", "ff", "C3A9"] {
            assert!(unhex(bad).is_err(), "{bad}");
        }
        assert_eq!(unhex(&hex("É\0日本語")).unwrap(), "É\0日本語");
    }
}

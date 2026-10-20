use qualification_unicode_compat::{
    FoldMap, fold, fold_map, go15_space, hex, is_mn, normalize, unhex,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::{self, BufRead};
use unicode_normalization::{UnicodeNormalization, char::canonical_combining_class};

const SCALARS: usize = 0x110000 - 0x800;
const MAX_RECORDED_DIFFS: usize = 4096;

fn input_at(index: usize, sequences: Option<&[Value]>) -> Result<String, String> {
    if let Some(cases) = sequences {
        return cases
            .get(index)
            .and_then(|v| v["input"].as_str())
            .map(str::to_owned)
            .ok_or_else(|| "sequence identity/count mismatch".into());
    }
    let n = index + if index >= 0xd800 { 0x800 } else { 0 };
    let scalar = u32::try_from(n).ok().and_then(char::from_u32);
    scalar
        .map(String::from)
        .ok_or_else(|| "scalar identity/count mismatch".into())
}

fn compare(
    reader: impl BufRead,
    map: &FoldMap,
    sequences: Option<&[Value]>,
) -> Result<Value, String> {
    let expected = sequences.map_or(SCALARS, <[Value]>::len);
    let mut count = 0;
    let mut ended = false;
    let mut totals = BTreeMap::<String, usize>::new();
    let mut comparisons = BTreeMap::<String, usize>::new();
    let mut differences = Vec::new();
    for line in reader.lines() {
        let line = line.map_err(|e| e.to_string())?;
        if ended {
            return Err("data after END marker".into());
        }
        if let Some(n) = line.strip_prefix("END\t") {
            if n.parse::<usize>().ok() != Some(count) || count != expected {
                return Err(format!("incomplete oracle stream: {count}/{expected}"));
            }
            ended = true;
            continue;
        }
        let fields: Vec<_> = line.split('\t').collect();
        if fields.len() != 8 || fields[0].parse::<usize>().ok() != Some(count) {
            return Err("oracle record format/order mismatch".into());
        }
        let input = input_at(count, sequences)?;
        if unhex(fields[1])? != input {
            return Err("oracle input bytes/identity mismatch".into());
        }
        let mut check = |metric: &str, actual: String, expected: &str| {
            *comparisons.entry(metric.into()).or_default() += 1;
            let total = totals.entry(metric.into()).or_default();
            if actual != expected {
                *total += 1;
                if differences.len() < MAX_RECORDED_DIFFS {
                    differences.push(
                        json!({"id":count,"name":sequences.map(|v|&v[count]["name"]),
                        "inputHex":hex(&input),"metric":metric,"go":expected,"candidate":actual}),
                    );
                }
            }
        };
        if sequences.is_none() {
            let c = input.chars().next().ok_or("missing scalar")?;
            check("Mn", is_mn(c).to_string(), fields[2]);
            check("Go15_IsSpace", go15_space(c).to_string(), fields[3]);
            check(
                "rust_std_IsSpace_diagnostic",
                c.is_whitespace().to_string(),
                fields[3],
            );
            check("CCC", canonical_combining_class(c).to_string(), fields[4]);
        } else if fields[2..5] != ["-", "-", "-"] {
            return Err("sequence incorrectly carries scalar properties".into());
        }
        check(
            "raw_NFKD",
            hex(&input.nfkd().collect::<String>()),
            fields[5],
        );
        check(
            "stream_safe_NFKD",
            hex(&input.nfkd().stream_safe().collect::<String>()),
            fields[5],
        );
        check("fullFold", hex(&fold(&input, map)), fields[6]);
        check(
            "pipeline_raw",
            hex(&normalize(&input, map, false)),
            fields[7],
        );
        check(
            "pipeline_stream_safe",
            hex(&normalize(&input, map, true)),
            fields[7],
        );
        if let Some(want) = sequences.and_then(|s| s[count]["expectedNormalized"].as_str()) {
            check("admitted_expected_vs_Go", fields[7].into(), &hex(want));
        }
        count += 1;
    }
    if !ended {
        return Err("missing END marker".into());
    }
    let total: usize = totals.values().sum();
    Ok(
        json!({"status":if total == 0 {"PASS"} else {"DIFF"},"compared":count,
        "metricDifferences":totals,"metricComparisons":comparisons,"differenceCount":total,"differences":differences,
        "allDifferencesRecorded":total <= MAX_RECORDED_DIFFS,
        "scope":"qualification-only, actual observed host; no production selection"}),
    )
}

fn run() -> Result<(), String> {
    if unicode_normalization::UNICODE_VERSION != (15, 0, 0)
        || unicode_general_category::UNICODE_VERSION != (15, 0, 0)
    {
        return Err("wrong candidate Unicode data version".into());
    }
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 3 {
        return Err("usage: qualification-unicode-compat FOLD_JSON scalars|CASES_JSON".into());
    }
    let map = fold_map(&std::fs::read(&args[1]).map_err(|e| e.to_string())?)?;
    let sequences: Option<Vec<Value>> = if args[2] == "scalars" {
        None
    } else {
        Some(
            serde_json::from_slice(&std::fs::read(&args[2]).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?,
        )
    };
    let report = compare(io::stdin().lock(), &map, sequences.as_deref())?;
    println!(
        "{}",
        serde_json::to_string(&report).map_err(|e| e.to_string())?
    );
    if report["status"] == "DIFF" {
        std::process::exit(2);
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("qualification/tooling failure (not parity evidence): {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_reordered_or_incomplete_oracle_output_never_passes() {
        let cases = vec![json!({"name":"one","input":"A"})];
        let map = BTreeMap::from([('A', "a".into())]);
        for bad in [
            "",
            "END\t0\n",
            "1\t41\t-\t-\t-\t41\t61\t61\n",
            "0\t42\t-\t-\t-\t42\t62\t62\n",
        ] {
            assert!(compare(bad.as_bytes(), &map, Some(&cases)).is_err());
        }
    }

    #[test]
    fn comparison_retains_differences_and_recovers_with_complete_correct_stream() {
        let cases = vec![json!({"name":"one","input":"A"})];
        let map = BTreeMap::from([('A', "a".into())]);
        let good = "0\t41\t-\t-\t-\t41\t61\t61\nEND\t1\n";
        assert_eq!(
            compare(good.as_bytes(), &map, Some(&cases)).unwrap()["status"],
            "PASS"
        );
        let bad = good.replace("\t61\t61", "\t41\t41");
        let observed = compare(bad.as_bytes(), &map, Some(&cases)).unwrap();
        assert_eq!(observed["status"], "DIFF");
        assert_eq!(observed["metricDifferences"]["fullFold"], 1);
        assert!(compare(format!("{good}END\t1\n").as_bytes(), &map, Some(&cases)).is_err());
    }
}

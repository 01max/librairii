//! Qualification-only child. Host supervisor must prove resource controls first.
//! Neither this policy sketch nor its synthetic outcomes are product classification.
use serde_json::{Value, json};
use std::cell::Cell;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::rc::Rc;
use std::time::{Duration, Instant};

const FROZEN_ENTRIES: usize = 10_000;
const FROZEN_EXPANDED: u64 = 2 << 30;
const FROZEN_RATIO: u64 = 200;
const EXPERIMENT_DICTIONARY: u64 = 8 << 20;

struct Source<R> {
    file: R,
    count: Rc<Cell<u64>>,
    ceiling: u64,
}
impl<R: Read> Read for Source<R> {
    fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        let remaining = self.ceiling.saturating_sub(self.count.get());
        if remaining == 0 {
            return Err(std::io::Error::other(
                "qualification compressed-read ceiling",
            ));
        }
        let limit = output.len().min(remaining as usize);
        let read = self.file.read(&mut output[..limit])?;
        self.count.set(self.count.get() + read as u64);
        Ok(read)
    }
}
impl<R: Seek> Seek for Source<R> {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.file.seek(position)
    }
}

fn proc_value(path: &str) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| format!("unavailable:{error}"))
}
fn emit(value: Value) {
    println!("{value}");
    std::io::stdout().flush().unwrap();
}
fn observation(outcome: &str, details: Value, start: Instant, reads: u64) {
    emit(
        json!({"outcome":outcome,"details":details,"elapsedMicros":start.elapsed().as_micros(),
        "sourceBytesRead":reads,"processStatus":proc_value("/proc/self/status"),
        "cgroupMemoryPeak":proc_value("/sys/fs/cgroup/memory.peak")}),
    );
}

fn dictionary(id: &[u8], properties: &[u8]) -> Option<u64> {
    match id {
        [0x21] => {
            let property = *properties.first()?;
            if property > 40 {
                return None;
            }
            Some(if property == 40 {
                u32::MAX as u64
            } else {
                (2 | u64::from(property & 1)) << (property / 2 + 11)
            })
        }
        [0x03, 0x01, 0x01] if properties.len() >= 5 => {
            Some(u32::from_le_bytes(properties[1..5].try_into().unwrap()) as u64)
        }
        _ => None,
    }
}

fn metadata_refusal(
    entries: usize,
    expanded: Option<u64>,
    input: u64,
    dictionaries: &[u64],
) -> Option<&'static str> {
    if entries > FROZEN_ENTRIES {
        return Some("frozen_entry_limit");
    }
    let expanded = match expanded {
        Some(value) => value,
        None => return Some("expanded_overflow"),
    };
    if expanded > FROZEN_EXPANDED {
        return Some("frozen_expanded_limit");
    }
    if u128::from(expanded) > u128::from(input.max(1)) * u128::from(FROZEN_RATIO) {
        return Some("frozen_ratio_limit");
    }
    if dictionaries
        .iter()
        .any(|&size| size > EXPERIMENT_DICTIONARY)
    {
        return Some("experimental_dictionary_limit_not_frozen_policy");
    }
    None
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let start = Instant::now();
    emit(
        json!({"stage":"boot","memoryMax":proc_value("/sys/fs/cgroup/memory.max"),
        "swapMax":proc_value("/sys/fs/cgroup/memory.swap.max"),"limits":proc_value("/proc/self/limits"),
        "memoryEventsBefore":proc_value("/sys/fs/cgroup/memory.events")}),
    );
    let args: Vec<_> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("smoke") => {
            emit(json!({"outcome":"smoke_ok"}));
            return Ok(());
        }
        Some("control-address") => {
            let data = Vec::<u8>::with_capacity(512 << 20);
            std::hint::black_box(data);
            emit(json!({"outcome":"CONTROL_FAILED_address_allocation_allowed"}));
            return Ok(());
        }
        Some("control-memory") => {
            let data = vec![0x55_u8; 192 << 20];
            std::hint::black_box(data.iter().map(|&n| u64::from(n)).sum::<u64>());
            emit(json!({"outcome":"CONTROL_FAILED_memory_allocation_allowed"}));
            return Ok(());
        }
        Some("control-wall") => {
            std::thread::sleep(Duration::from_secs(60));
            emit(json!({"outcome":"CONTROL_FAILED_wall_allowed"}));
            return Ok(());
        }
        Some("control-cpu") => {
            let mut count = 0_u64;
            loop {
                count = std::hint::black_box(count.wrapping_add(1));
            }
        }
        _ => {}
    }
    let mode = args.get(1).ok_or("missing mode")?;
    let file = File::open(args.get(2).ok_or("missing synthetic fixture")?)?;
    let input_size = file.metadata()?.len();
    let reads = Rc::new(Cell::new(0_u64));
    if mode == "cancel-before" {
        observation(
            "qualification_cancelled",
            json!({"stage":"before_header"}),
            start,
            reads.get(),
        );
        return Ok(());
    }
    let source_ceiling = args
        .get(3)
        .map(|s| s.parse())
        .transpose()?
        .unwrap_or(64 << 20);
    let delivered_ceiling = args
        .get(4)
        .map(|s| s.parse())
        .transpose()?
        .unwrap_or(FROZEN_EXPANDED);
    let source = Source {
        file,
        count: reads.clone(),
        ceiling: source_ceiling,
    };
    let mut reader = match sevenz_rust2::ArchiveReader::new(source, sevenz_rust2::Password::empty())
    {
        Ok(reader) => reader,
        Err(error) => {
            observation(
                "library_error",
                json!({"stage":"header","error":error.to_string()}),
                start,
                reads.get(),
            );
            return Ok(());
        }
    };
    reader.set_thread_count(1);
    let archive = reader.archive();
    let entries = archive.files.len();
    let expanded = archive
        .files
        .iter()
        .try_fold(0_u64, |sum, entry| sum.checked_add(entry.size));
    let dictionaries: Vec<_> = archive
        .blocks
        .iter()
        .flat_map(|block| &block.coders)
        .filter_map(|coder| dictionary(coder.encoder_method_id(), coder.properties()))
        .collect();
    let metadata = json!({"entries":entries,"expanded":expanded,"inputBytes":input_size,
        "dictionaries":dictionaries,"solid":archive.is_solid,"headerSourceBytesRead":reads.get()});
    emit(json!({"stage":"metadata","metadata":metadata}));
    if mode != "raw"
        && mode != "raw-cancel"
        && let Some(reason) = metadata_refusal(entries, expanded, input_size, &dictionaries)
    {
        observation(
            "qualification_metadata_refused",
            json!({"reason":reason,"metadata":metadata}),
            start,
            reads.get(),
        );
        return Ok(());
    }
    if mode == "metadata-only" {
        observation(
            "qualification_metadata_allowed_only",
            metadata,
            start,
            reads.get(),
        );
        return Ok(());
    }
    let mut delivered = 0_u64;
    let mut decoded = Vec::new();
    let mut cancelled = false;
    let mut callbacks = 0_u64;
    emit(json!({"stage":"before_callbacks","callbackCount":callbacks}));
    let result = reader.for_each_entries(|entry, input| {
        callbacks += 1;
        emit(json!({"stage":"callback","ordinal":callbacks,"name":entry.name}));
        let mut bytes = 0_u64;
        let mut crc = !0_u32;
        let mut buffer = [0_u8; 8192];
        loop {
            let allowed = buffer.len().min(delivered_ceiling.saturating_sub(delivered).saturating_add(1) as usize);
            let count = input.read(&mut buffer[..allowed])?;
            if count == 0 { break; }
            bytes += count as u64;
            delivered += count as u64;
            if delivered > delivered_ceiling {
                return Err(std::io::Error::other("qualification delivered-byte ceiling").into());
            }
            for &byte in &buffer[..count] {
                crc ^= u32::from(byte);
                for _ in 0..8 { crc = (crc >> 1) ^ (0xEDB88320_u32 & (0_u32.wrapping_sub(crc & 1))); }
            }
        }
        decoded.push(json!({"name":entry.name,"bytes":bytes,"declaredBytes":entry.size,"headerCrc":entry.crc,"hasCrc":entry.has_crc,"observedCrc32":!crc}));
        if mode == "raw-cancel" {
            cancelled = true;
            return Ok(false);
        }
        Ok(true)
    });
    let size_mismatch = decoded
        .iter()
        .any(|entry| entry["bytes"] != entry["declaredBytes"]);
    let details = json!({"metadata":metadata,"deliveredBytes":delivered,"deliveredCeiling":delivered_ceiling,"callbackCount":callbacks,
        "decoded":decoded,"error":result.as_ref().err().map(ToString::to_string)});
    observation(
        if cancelled {
            "qualification_cancelled"
        } else if result.is_err() {
            "library_or_probe_read_error"
        } else if size_mismatch {
            "decoder_eof_with_size_mismatch"
        } else {
            "decoded"
        },
        details,
        start,
        reads.get(),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frozen_metadata_boundaries_and_overflow_remain_explicit() {
        assert_eq!(metadata_refusal(10_000, Some(2 << 30), 11 << 20, &[]), None);
        assert_eq!(
            metadata_refusal(10_001, Some(1), 100, &[]),
            Some("frozen_entry_limit")
        );
        assert_eq!(
            metadata_refusal(1, Some((2 << 30) + 1), 11 << 20, &[]),
            Some("frozen_expanded_limit")
        );
        assert_eq!(
            metadata_refusal(1, None, 100, &[]),
            Some("expanded_overflow")
        );
        assert_eq!(metadata_refusal(1, Some(200), 1, &[]), None);
        assert_eq!(
            metadata_refusal(1, Some(201), 1, &[]),
            Some("frozen_ratio_limit")
        );
    }

    #[test]
    fn seeking_back_does_not_reset_the_actual_source_read_budget() {
        let count = Rc::new(Cell::new(0));
        let mut source = Source {
            file: std::io::Cursor::new([1, 2, 3, 4]),
            count: count.clone(),
            ceiling: 4,
        };
        let mut output = [0; 2];
        source.read_exact(&mut output).unwrap();
        source.seek(SeekFrom::Start(0)).unwrap();
        source.read_exact(&mut output).unwrap();
        assert_eq!(output, [1, 2]);
        assert_eq!(count.get(), 4);
        assert!(
            source
                .read(&mut output)
                .unwrap_err()
                .to_string()
                .contains("compressed-read ceiling")
        );
        assert_eq!(count.get(), 4);
    }
}

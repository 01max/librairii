#![no_main]
use libfuzzer_sys::fuzz_target;
use librairii_adapters::archives::{Archive, DecoderLimits};
use librairii_app::native_jobs::Cancellation;
use librairii_core::import::{
    CompressionAccounting, EntryInfo, ImportIdentity, InventoryBuilder, Limits,
    project_optional_metadata, project_studio_json, validate_entry_name,
};
use std::io::Cursor;
fuzz_target!(|data: &[u8]| {
    if data.is_empty() || data.len() > 16 * 1024 {
        return;
    }
    let payload = &data[1..];
    let limits = Limits {
        max_entries: 64,
        max_expanded_bytes: 128 * 1024,
        max_metadata_bytes: 16 * 1024,
        max_artwork_bytes: 16 * 1024,
        ..Limits::default()
    };
    match data[0] % 5 {
        kind @ (0 | 1) => {
            let name = if kind == 0 {
                b"fuzz.zip".as_slice()
            } else {
                b"fuzz.7z".as_slice()
            };
            if let Ok(mut archive) = Archive::open_bounded(
                Cursor::new(payload),
                name,
                limits,
                DecoderLimits {
                    header_bytes: 16 * 1024,
                    memory_bytes: 64 * 1024 * 1024,
                },
                &Cancellation::default(),
            ) {
                let _ = archive.inspect_metadata();
                let _ = archive.verify_payloads();
            }
        }
        2 => {
            let _ = validate_entry_name(payload, limits);
        }
        _ => {
            let Ok(mut builder) = InventoryBuilder::new(limits, CompressionAccounting::ZipEntries)
            else {
                return;
            };
            for name in [
                "story.json",
                "thumbnail.png",
                "assets/track",
                "_metadata.json",
            ] {
                if builder
                    .push(EntryInfo {
                        name: name.as_bytes(),
                        compressed_bytes: 1,
                        uncompressed_bytes: 1,
                        is_directory: false,
                        is_link: false,
                    })
                    .is_err()
                {
                    return;
                }
            }
            let Ok(inventory) = builder.finish() else {
                return;
            };
            let _ = project_studio_json(&inventory, payload, limits);
            if let Ok(identity) =
                ImportIdentity::parse_text(b"00112233-4455-4677-8899-aabbccddeeff")
            {
                let _ = project_optional_metadata(&inventory, Some(payload), &identity, limits);
            }
        }
    }
});

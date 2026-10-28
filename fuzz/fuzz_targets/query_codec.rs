#![no_main]
use libfuzzer_sys::fuzz_target;
use librairii_core::collection_codec::{self as codec, url};
fuzz_target!(|data: &[u8]| {
    if data.is_empty() || data.len() > 16 * 1024 {
        return;
    }
    let payload = &data[1..];
    match data[0] % 3 {
        0 => {
            let version = i64::from(data[0] % 7);
            let _references = codec::decode_references(version, payload, 1);
            if let Ok((encoded, query)) = codec::migrate(version, payload, 1) {
                let current =
                    codec::decode(5, encoded.as_bytes(), 1).expect("migrated current query");
                assert_eq!(
                    codec::encode(current).expect("canonical current encoding"),
                    encoded
                );
                assert_eq!(
                    codec::encode(query).expect("migrated query encoding"),
                    encoded
                );
            }
        }
        1 => {
            if let Ok(request) = url::restore_request(payload) {
                let encoded = url::request_bytes(&request).expect("restored request serializes");
                assert_eq!(
                    url::restore_request(&encoded).expect("current request roundtrip"),
                    request
                );
            }
        }
        _ => {
            // Independent URL and JSON frames let mutations reach successful
            // history restoration as well as malformed framing and payloads.
            if payload.len() < 2 {
                return;
            }
            let hash_len = usize::from(u16::from_le_bytes([payload[0], payload[1]]));
            let Some(hash) = payload.get(2..2 + hash_len) else {
                return;
            };
            if let Ok(text) = std::str::from_utf8(hash) {
                let _decoded = url::decode_history(text, &payload[2 + hash_len..], 1);
                let _query = url::decode(text, 1);
            }
        }
    }
});

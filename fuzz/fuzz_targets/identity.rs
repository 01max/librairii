#![no_main]
use libfuzzer_sys::fuzz_target;
use librairii_core::{Checksum, StoryUuid};
fuzz_target!(|data: &[u8]| {
    if data.len() > 128 {
        return;
    }
    if let Ok(text) = std::str::from_utf8(data) {
        if let Ok(checksum) = text.parse::<Checksum>() {
            assert_eq!(checksum.to_string().parse::<Checksum>(), Ok(checksum));
            assert_eq!(text.len(), 64);
        }
        if let Ok(uuid) = text.parse::<StoryUuid>() {
            assert_eq!(uuid.to_string().parse::<StoryUuid>(), Ok(uuid));
        }
    }
});

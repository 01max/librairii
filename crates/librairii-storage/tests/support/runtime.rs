use librairii_app::native_jobs::{Handle, Refusal, Start};
use std::{
    error::Error,
    thread,
    time::{Duration, Instant},
};
/// Try-intake may refuse while the coordinator briefly owns its mutex. Tests
/// retain the original move-only command, just as a native controller must.
pub fn offer(handle: &Handle, mut start: Start) -> Result<(), Box<dyn Error>> {
    let end = Instant::now() + Duration::from_secs(10);
    loop {
        match handle.try_start(start) {
            Ok(()) => return Ok(()),
            Err((Refusal::Busy, original)) if Instant::now() < end => {
                start = original;
                thread::sleep(Duration::from_millis(1));
            }
            Err(_) => return Err("runtime intake unavailable".into()),
        }
    }
}

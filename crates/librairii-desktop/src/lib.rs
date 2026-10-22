//! Native composition without constructing a toolkit/window in headless checks.
#![forbid(unsafe_code)]
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub mod imports;

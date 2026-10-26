//! Native composition without constructing a toolkit/window in headless checks.
#![forbid(unsafe_code)]
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub mod imports;
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub mod library;
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub mod settings;
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub mod startup;

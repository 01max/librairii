//! Native composition without constructing a toolkit/window in headless checks.
#![forbid(unsafe_code)]
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub mod imports;
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub mod library;
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub mod metadata_jobs;
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub mod presentation;
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub mod settings;
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub mod startup;

#[cfg(any(target_os = "macos", target_os = "linux"))]
pub mod browsing;
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub mod maintenance;
pub mod menus;

#[cfg(any(target_os = "macos", target_os = "linux"))]
pub mod workflows;

mod host;

mod export_preparation;

mod startup_flow;

pub mod preferences;

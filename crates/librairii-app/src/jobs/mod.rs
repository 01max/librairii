//! Released NEXT6 headless scheduling slice, not product composition.
//!
//! The original injected engine remains private. The `scoped` child adds a
//! run-lifetime facade over the existing typed terminal-store port. There is no
//! library activation, import/custody implementation or owned static bridge.
//! Controlled tests are separate from actual storage composition evidence.
#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "The released private headless engine is tested here; production composition and shared ports remain outside this handoff."
    )
)]

mod model;
mod progress;
mod runtime;
mod terminal;

/// Scoped terminal reporting over an explicitly borrowed synchronous store.
pub mod scoped;

#[cfg(test)]
mod tests;

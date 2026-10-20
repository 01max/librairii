//! Released NEXT6 headless scheduling slice, not product composition.
//!
//! All values and injected callbacks are private to this module. There is no
//! library activation, storage/import/custody port or durable receipt here.
//! Tests exercise actual owned threads and controlled responses. A successful
//! injected response is not evidence of a real database commit.
#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "The released private headless engine is tested here; production composition and shared ports remain outside this handoff."
    )
)]

mod model;
mod runtime;

#[cfg(test)]
mod tests;

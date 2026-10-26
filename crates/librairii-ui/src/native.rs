//! Compiled native widgets. No persistence, filesystem or browser bridge.
// Slint's generated widget implementation owns toolkit internals and generated
// lint decisions. Handwritten presentation code retains workspace lint policy.
#[allow(
    unsafe_code,
    clippy::all,
    clippy::unwrap_used,
    clippy::panic,
    clippy::todo,
    missing_debug_implementations
)]
mod widgets {
    slint::include_modules!();
}
pub use widgets::*;

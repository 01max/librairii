//! Scoped, controlled terminal reporting. This is not product composition.
//!
//! The host retains its library and database outside `run`. Callbacks borrow a
//! synchronous store; each calling worker owns and waits its own storage pending
//! result. The host must inspect `RunOutcome` before ending required ownership or
//! closing storage. Joining threads alone never certifies settlement.
//!
//! No source admission, creation, native activation, Files, import or custody is
//! supplied here. Source/work/delivery callbacks are explicitly controlled host
//! inputs. Public DTO construction does not establish database authority.

mod driver;
mod model;
mod runtime;

pub use model::*;
pub use runtime::run;

#[cfg(test)]
mod tests;

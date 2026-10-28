//! Read-only process CPU diagnostic; wall time remains the acceptance gate.
#![forbid(unsafe_code)]
pub fn milliseconds() -> f64 {
    let clock = rustix::time::clock_gettime(rustix::time::ClockId::ProcessCPUTime);
    clock.tv_sec as f64 * 1000. + clock.tv_nsec as f64 / 1_000_000.
}

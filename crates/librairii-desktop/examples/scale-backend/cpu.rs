//! Read-only process CPU diagnostic; wall time remains the acceptance gate.
#[allow(unsafe_code)]
pub fn milliseconds() -> std::io::Result<f64> {
    let mut clock = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: clock is an initialized, aligned writable timespec. The public
    // process clock reads accumulated CPU for this process, with no state write.
    if unsafe { libc::clock_gettime(libc::CLOCK_PROCESS_CPUTIME_ID, &raw mut clock) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(clock.tv_sec as f64 * 1000. + clock.tv_nsec as f64 / 1_000_000.)
}

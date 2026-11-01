//! Native desktop handovers called only after actual path validation on a worker.
use std::{
    io,
    path::Path,
    process::{Command, Stdio},
};
pub fn reveal(path: &Path) -> io::Result<()> {
    #[cfg(target_os = "macos")]
    let mut command = Command::new("/usr/bin/open");
    #[cfg(target_os = "linux")]
    let mut command = {
        // GIO returns the asynchronous launch acknowledgement/error, rather than
        // waiting for an external file manager's window lifetime. No shell argv.
        let mut launcher = Command::new("gio");
        launcher.args(["open", "--"]);
        launcher
    };
    let status = command
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(
            "native file manager rejected the destination",
        ))
    }
}

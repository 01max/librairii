//! Bounded readiness observation for the isolated Go lock-owner rehearsal.
use std::{
    fs, io,
    path::Path,
    thread,
    time::{Duration, Instant},
};

const READY: &[u8] = b"go-flock-owned\n";

pub fn wait_for_ready(
    path: &Path,
    mut owner_alive: impl FnMut() -> io::Result<bool>,
    deadline: Instant,
) -> io::Result<()> {
    loop {
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "Go readiness deadline",
            ));
        }
        if !owner_alive()? {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Go owner exited before readiness",
            ));
        }
        let complete = match fs::read(path) {
            Ok(bytes) if bytes == READY => true,
            Ok(bytes) if READY.starts_with(&bytes) => false,
            Ok(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "invalid Go readiness payload",
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => false,
            Err(error) => return Err(error),
        };
        // A file name or an incomplete write is not ownership evidence. The
        // helper must still be alive and inside the original deadline at return.
        if !owner_alive()? {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Go owner exited during readiness",
            ));
        }
        if Instant::now() >= deadline {
            return Err(io::Error::new(io::ErrorKind::TimedOut, "late Go readiness"));
        }
        if complete {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static SERIAL: AtomicU64 = AtomicU64::new(0);
    struct Marker(std::path::PathBuf);
    impl Marker {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "librairii-readiness-{}-{}",
                std::process::id(),
                SERIAL.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn path(&self) -> std::path::PathBuf {
            self.0.join("ready")
        }
    }
    impl Drop for Marker {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn creation_before_write_waits_for_complete_owned_marker() {
        let fixture = Marker::new();
        let path = fixture.path();
        fs::write(&path, b"").unwrap();
        let mut polls = 0;
        wait_for_ready(
            &path,
            || {
                polls += 1;
                if polls == 2 {
                    fs::write(&path, b"go-flock-")?;
                }
                if polls == 3 {
                    fs::write(&path, READY)?;
                }
                Ok(true)
            },
            Instant::now() + Duration::from_secs(1),
        )
        .unwrap();
        assert!(polls >= 3);
        assert_eq!(fs::read(path).unwrap(), READY);
    }

    #[test]
    fn complete_marker_cannot_hide_owner_exit() {
        let fixture = Marker::new();
        fs::write(fixture.path(), READY).unwrap();
        let mut polls = 0;
        let error = wait_for_ready(
            &fixture.path(),
            || {
                polls += 1;
                Ok(polls == 1)
            },
            Instant::now() + Duration::from_secs(1),
        )
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
    }

    #[test]
    fn final_owner_observation_cannot_accept_after_deadline() {
        let fixture = Marker::new();
        fs::write(fixture.path(), READY).unwrap();
        let deadline = Instant::now() + Duration::from_millis(20);
        let mut polls = 0;
        let error = wait_for_ready(
            &fixture.path(),
            || {
                polls += 1;
                if polls == 2 {
                    thread::sleep(Duration::from_millis(30));
                }
                Ok(true)
            },
            deadline,
        )
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    }

    #[test]
    fn deadline_and_invalid_payload_are_not_retried_as_readiness() {
        let fixture = Marker::new();
        fs::write(fixture.path(), b"wrong-owner\n").unwrap();
        let error = wait_for_ready(
            &fixture.path(),
            || Ok(true),
            Instant::now() + Duration::from_secs(1),
        )
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        fs::write(fixture.path(), READY).unwrap();
        let error = wait_for_ready(
            &fixture.path(),
            || panic!("expired deadline"),
            Instant::now(),
        )
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    }

    #[test]
    fn absent_marker_waits_for_creation_but_missing_owner_refuses() {
        let fixture = Marker::new();
        let error = wait_for_ready(
            &fixture.path(),
            || Ok(false),
            Instant::now() + Duration::from_secs(1),
        )
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
        let mut polls = 0;
        wait_for_ready(
            &fixture.path(),
            || {
                polls += 1;
                if polls == 3 {
                    fs::write(fixture.path(), READY)?;
                }
                Ok(true)
            },
            Instant::now() + Duration::from_secs(1),
        )
        .unwrap();
        assert!(polls >= 3);
    }
}

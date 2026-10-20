//! Fresh owned synthetic roots and loopback-only networking. No profile discovery.
use std::{
    fs, io,
    net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    time::Duration,
};

/// Only this owner can authorize its exact fresh library and profile directories.
#[derive(Debug)]
pub struct TestRoots {
    owner: tempfile::TempDir,
    library: PathBuf,
    profile: PathBuf,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorizedRoots {
    library: PathBuf,
    profile: PathBuf,
}
impl AuthorizedRoots {
    pub fn library(&self) -> &Path {
        &self.library
    }
    pub fn profile(&self) -> &Path {
        &self.profile
    }
}
impl TestRoots {
    pub fn new() -> io::Result<Self> {
        let owner = tempfile::Builder::new()
            .prefix("librairii-synthetic-")
            .tempdir()?;
        let root = fs::canonicalize(owner.path())?;
        let library = root.join("library");
        let profile = root.join("profile");
        fs::create_dir(&library)?;
        fs::create_dir(&profile)?;
        Ok(Self {
            owner,
            library,
            profile,
        })
    }
    pub fn library(&self) -> &Path {
        &self.library
    }
    pub fn profile(&self) -> &Path {
        &self.profile
    }
    /// Missing, external, swapped, aliased and symlinked paths fail before any use.
    /// This helper does not open a product library or load settings.
    pub fn authorize(
        &self,
        library: Option<&Path>,
        profile: Option<&Path>,
    ) -> io::Result<AuthorizedRoots> {
        let valid = |actual: Option<&Path>, expected: &Path| -> io::Result<()> {
            let actual = actual.ok_or_else(|| io::Error::other("explicit test root required"))?;
            if actual != expected
                || fs::symlink_metadata(actual)?.file_type().is_symlink()
                || fs::canonicalize(actual)? != expected
                || !actual.is_dir()
                || actual.parent() != Some(fs::canonicalize(self.owner.path())?.as_path())
            {
                return Err(io::Error::other(
                    "root is not an owned isolated test directory",
                ));
            }
            Ok(())
        };
        valid(library, &self.library)?;
        valid(profile, &self.profile)?;
        Ok(AuthorizedRoots {
            library: self.library.clone(),
            profile: self.profile.clone(),
        })
    }
}

/// A numeric loopback address avoids DNS and cannot represent a public endpoint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LocalEndpoint(SocketAddr);
impl LocalEndpoint {
    pub fn new(address: SocketAddr) -> io::Result<Self> {
        if !address.ip().is_loopback() || address.port() == 0 {
            return Err(io::Error::other(
                "non-loopback or unspecified test endpoint",
            ));
        }
        Ok(Self(address))
    }
    pub const fn address(self) -> SocketAddr {
        self.0
    }
    pub fn connect(self) -> io::Result<TcpStream> {
        let stream = TcpStream::connect_timeout(&self.0, Duration::from_secs(2))?;
        stream.set_read_timeout(Some(Duration::from_secs(2)))?;
        stream.set_write_timeout(Some(Duration::from_secs(2)))?;
        Ok(stream)
    }
}
/// No background runtime or public bind. Tests own and join any server thread.
pub fn local_listener() -> io::Result<(TcpListener, LocalEndpoint)> {
    let listener = TcpListener::bind((IpAddr::V4(Ipv4Addr::LOCALHOST), 0))?;
    let endpoint = LocalEndpoint::new(listener.local_addr()?)?;
    Ok((listener, endpoint))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    #[test]
    fn only_exact_owned_roots_are_authorized_and_cleaned() -> io::Result<()> {
        let roots = TestRoots::new()?;
        let external = tempfile::tempdir()?;
        fs::write(external.path().join("keep"), b"synthetic sentinel")?;
        assert!(roots.authorize(None, None).is_err());
        assert!(
            roots
                .authorize(Some(external.path()), Some(roots.profile()))
                .is_err()
        );
        assert!(
            roots
                .authorize(Some(roots.profile()), Some(roots.library()))
                .is_err()
        );
        assert!(roots.authorize(Some(roots.library()), None).is_err());
        let authorized = roots.authorize(Some(roots.library()), Some(roots.profile()))?;
        assert_ne!(authorized.library(), authorized.profile());
        let library = authorized.library().to_owned();
        fs::write(library.join("fixture"), b"isolated")?;
        drop(roots);
        assert!(!library.exists());
        assert_eq!(
            fs::read(external.path().join("keep"))?,
            b"synthetic sentinel"
        );
        Ok(())
    }
    #[cfg(unix)]
    #[test]
    fn a_symlink_replacing_an_owned_directory_is_rejected() -> io::Result<()> {
        let roots = TestRoots::new()?;
        let external = tempfile::tempdir()?;
        fs::remove_dir(roots.library())?;
        std::os::unix::fs::symlink(external.path(), roots.library())?;
        assert!(
            roots
                .authorize(Some(roots.library()), Some(roots.profile()))
                .is_err()
        );
        Ok(())
    }
    #[test]
    fn network_boundary_rejects_public_and_wildcard_addresses() -> io::Result<()> {
        for ip in [
            "192.0.2.1",
            "0.0.0.0",
            "::",
            "2001:db8::1",
            "::ffff:192.0.2.1",
        ] {
            let address = SocketAddr::new(ip.parse().map_err(io::Error::other)?, 443);
            assert!(LocalEndpoint::new(address).is_err());
        }
        assert!(LocalEndpoint::new(SocketAddr::from(([127, 0, 0, 1], 0))).is_err());
        Ok(())
    }
    #[test]
    fn synthetic_http_bytes_are_served_over_loopback() -> io::Result<()> {
        let (listener, endpoint) = local_listener()?;
        let server = std::thread::spawn(move || -> io::Result<()> {
            let (mut socket, peer) = listener.accept()?;
            assert!(peer.ip().is_loopback());
            socket.set_read_timeout(Some(Duration::from_secs(2)))?;
            let mut request = [0; 512];
            let count = socket.read(&mut request)?;
            assert!(request[..count].starts_with(b"GET /fixture HTTP/1.0"));
            socket.write_all(b"HTTP/1.0 200 OK\r\nContent-Length: 2\r\n\r\n{}")
        });
        let mut client = endpoint.connect()?;
        client.write_all(b"GET /fixture HTTP/1.0\r\n\r\n")?;
        let mut response = String::new();
        client.read_to_string(&mut response)?;
        server
            .join()
            .map_err(|_| io::Error::other("fixture server failed"))??;
        assert_eq!(response, "HTTP/1.0 200 OK\r\nContent-Length: 2\r\n\r\n{}");
        Ok(())
    }
}

/// Inject partial writes and a deterministic failure after observable bytes.
/// Used around real temporary files by later custody/publication tests.
#[derive(Debug)]
pub struct FailAfter<W> {
    inner: W,
    remaining: usize,
    chunk: usize,
}
impl<W> FailAfter<W> {
    pub fn new(inner: W, remaining: usize, chunk: usize) -> io::Result<Self> {
        if chunk == 0 {
            return Err(io::Error::other("write chunk must be positive"));
        }
        Ok(Self {
            inner,
            remaining,
            chunk,
        })
    }
}
impl<W: io::Write> io::Write for FailAfter<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        if self.remaining == 0 {
            return Err(io::Error::other("synthetic write failure"));
        }
        let count = self
            .inner
            .write(&bytes[..bytes.len().min(self.remaining).min(self.chunk)])?;
        self.remaining -= count;
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
#[cfg(test)]
mod failure_tests {
    use super::*;
    use std::io::Write;
    #[test]
    fn failed_write_preserves_source_and_exact_partial_destination_bytes() -> io::Result<()> {
        let roots = TestRoots::new()?;
        let source = roots.library().join("synthetic-source");
        let destination = roots.library().join("synthetic-partial");
        fs::write(&source, b"synthetic fixture bytes")?;
        let file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&destination)?;
        let mut writer = FailAfter::new(file, 9, 2)?;
        assert!(writer.write_all(&fs::read(&source)?).is_err());
        writer.flush()?;
        drop(writer);
        assert_eq!(fs::read(&source)?, b"synthetic fixture bytes");
        assert_eq!(fs::read(&destination)?, b"synthetic");
        assert!(FailAfter::new(Vec::<u8>::new(), 5, 0).is_err());
        Ok(())
    }
}

use super::*;
use std::{
    error::Error,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::atomic::{AtomicBool, Ordering},
    thread,
};
type TestResult<T = ()> = std::result::Result<T, Box<dyn Error>>;
#[derive(Debug)]
struct Reply {
    status: u16,
    media: &'static str,
    body: Vec<u8>,
    delay: Duration,
    chunked: bool,
}
fn reply(body: &[u8]) -> Reply {
    Reply {
        status: 200,
        media: "application/json",
        body: body.to_vec(),
        delay: Duration::ZERO,
        chunked: false,
    }
}
fn token() -> Reply {
    reply(br#"{"response":{"token":{"server":" public-guest "}}}"#)
}
fn server(
    replies: Vec<Reply>,
) -> TestResult<(String, thread::JoinHandle<std::io::Result<Vec<String>>>)> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let base = format!("http://{}/", listener.local_addr()?);
    listener.set_nonblocking(true)?;
    let worker = thread::spawn(move || {
        let mut requests = Vec::new();
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        for reply in replies {
            let mut stream: TcpStream = loop {
                match listener.accept() {
                    Ok((s, _)) => break s,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        if std::time::Instant::now() > deadline {
                            return Err(std::io::Error::other("fixture request deadline"));
                        }
                        thread::sleep(Duration::from_millis(2));
                    }
                    Err(e) => return Err(e),
                }
            };
            stream.set_nonblocking(false)?;
            stream.set_read_timeout(Some(Duration::from_secs(2)))?;
            let mut request = Vec::new();
            let mut buf = [0; 1024];
            while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                let n = stream.read(&mut buf)?;
                if n == 0 || request.len() + n > 16384 {
                    return Err(std::io::Error::other("fixture request bounds"));
                }
                request.extend_from_slice(&buf[..n]);
            }
            requests.push(String::from_utf8_lossy(&request).into_owned());
            thread::sleep(reply.delay);
            let length = if reply.chunked {
                "Transfer-Encoding: chunked".to_string()
            } else {
                format!("Content-Length: {}", reply.body.len())
            };
            let head = format!(
                "HTTP/1.1 {} Fixture\r\nContent-Type: {}\r\n{}\r\nLocation: /redirect-target\r\nConnection: close\r\n\r\n",
                reply.status, reply.media, length
            );
            if stream.write_all(head.as_bytes()).is_ok() {
                if reply.chunked {
                    let _ = write!(stream, "{:x}\r\n", reply.body.len());
                    let _ = stream.write_all(&reply.body);
                    let _ = stream.write_all(b"\r\n0\r\n\r\n");
                } else {
                    let _ = stream.write_all(&reply.body);
                }
            }
        }
        Ok(requests)
    });
    Ok((base, worker))
}
fn joined(worker: thread::JoinHandle<std::io::Result<Vec<String>>>) -> TestResult<Vec<String>> {
    worker
        .join()
        .map_err(|_| "fixture thread panicked")?
        .map_err(Into::into)
}
// A response stays pending until the operation has stopped. Receipt is an
// explicit signal, so cancellation does not depend on a fast CI scheduler.
struct WaitingServer {
    base: String,
    arrived: tokio::sync::oneshot::Receiver<()>,
    stop: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<std::io::Result<bool>>>,
}
impl WaitingServer {
    fn new() -> TestResult<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        listener.set_nonblocking(true)?;
        let base = format!("http://{}/", listener.local_addr()?);
        let stop = Arc::new(AtomicBool::new(false));
        let shutdown = stop.clone();
        let (sent, arrived) = tokio::sync::oneshot::channel();
        let worker = thread::spawn(move || {
            let deadline = std::time::Instant::now() + Duration::from_secs(10);
            let mut stream = loop {
                if shutdown.load(Ordering::Acquire) {
                    return Ok(false);
                }
                if std::time::Instant::now() >= deadline {
                    return Err(std::io::Error::other("fixture connection deadline"));
                }
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2))
                    }
                    Err(error) => return Err(error),
                }
            };
            stream.set_nonblocking(true)?;
            let mut request = Vec::new();
            let mut buffer = [0; 1024];
            while !request.windows(4).any(|part| part == b"\r\n\r\n") {
                if shutdown.load(Ordering::Acquire) {
                    return Ok(false);
                }
                if std::time::Instant::now() >= deadline {
                    return Err(std::io::Error::other("fixture header deadline"));
                }
                match stream.read(&mut buffer) {
                    Ok(0) => return Ok(false), // Early timeout/cancel may send no headers.
                    Ok(count) => {
                        if request.len() + count > 16384 {
                            return Err(std::io::Error::other("fixture request bounds"));
                        }
                        request.extend_from_slice(&buffer[..count]);
                    }
                    Err(error)
                        if matches!(
                            error.kind(),
                            std::io::ErrorKind::ConnectionReset
                                | std::io::ErrorKind::ConnectionAborted
                        ) =>
                    {
                        return Ok(false);
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2))
                    }
                    Err(error) => return Err(error),
                }
            }
            let _ = sent.send(());
            while !shutdown.load(Ordering::Acquire) {
                if std::time::Instant::now() >= deadline {
                    return Err(std::io::Error::other("fixture response release deadline"));
                }
                thread::sleep(Duration::from_millis(2));
            }
            Ok(true)
        });
        Ok(Self {
            base,
            arrived,
            stop,
            worker: Some(worker),
        })
    }
    fn finish(&mut self) -> TestResult<bool> {
        self.stop.store(true, Ordering::Release);
        self.worker
            .take()
            .ok_or("fixture already joined")?
            .join()
            .map_err(|_| "fixture thread panicked")?
            .map_err(Into::into)
    }
}
impl Drop for WaitingServer {
    fn drop(&mut self) {
        if self.worker.is_some() {
            let _ = self.finish();
        }
    }
}
#[tokio::test]
async fn guest_envelope_headers_and_exact_catalog_bytes_match_protocol() -> TestResult {
    let payload = b"  {\"response\":{\"synthetic\":true}}\n";
    let (base, worker) = server(vec![token(), reply(payload)])?;
    let client = CatalogClient::loopback(&base, DEADLINE)?;
    assert_eq!(
        client.fetch_catalog(&Cancellation::default()).await?,
        payload
    );
    let requests = joined(worker)?;
    assert!(requests[0].starts_with("GET /guest/create HTTP/1.1"));
    assert!(requests[1].starts_with("GET /v2/packs HTTP/1.1"));
    for request in &requests {
        let request = request.to_ascii_lowercase();
        assert!(request.contains("accept: application/json\r\n"));
        assert!(request.contains("user-agent: librairii/metadata-sync\r\n"));
    }
    let headers = requests[1].to_ascii_lowercase();
    assert!(headers.contains("application-sender: luniistore_desktop\r\n"));
    assert!(headers.contains("x-auth-token: public-guest\r\n"));
    assert!(!requests[0].to_ascii_lowercase().contains("x-auth-token"));
    Ok(())
}
#[tokio::test]
async fn redirects_status_content_type_malformed_json_and_token_are_refused() -> TestResult {
    for (response, want) in [
        (
            {
                let mut r = token();
                r.status = 302;
                r
            },
            MetadataError::Redirect,
        ),
        (
            {
                let mut r = token();
                r.status = 500;
                r
            },
            MetadataError::Status,
        ),
        (
            {
                let mut r = token();
                r.media = "text/html";
                r
            },
            MetadataError::InvalidJson,
        ),
        (reply(b"{"), MetadataError::InvalidJson),
        (
            reply(br#"{"response":{"token":{"server":"bad\nheader"}}}"#),
            MetadataError::InvalidToken,
        ),
    ] {
        let (base, worker) = server(vec![response])?;
        let client = CatalogClient::loopback(&base, DEADLINE)?;
        assert_eq!(
            client.fetch_catalog(&Cancellation::default()).await,
            Err(want)
        );
        assert_eq!(joined(worker)?.len(), 1);
    }
    for base in [
        "http://example.com/",
        "http://localhost/",
        "https://127.0.0.1/",
        "http://user@127.0.0.1/",
        "http://127.0.0.1/path",
        "http://127.0.0.1/?query",
    ] {
        assert!(CatalogClient::loopback(base, DEADLINE).is_err());
    }
    Ok(())
}
#[tokio::test]
async fn actual_streamed_limits_apply_to_token_and_catalog_without_content_length() -> TestResult {
    let mut too_large = reply(&vec![b' '; TOKEN_LIMIT + 1]);
    too_large.chunked = true;
    let (base, worker) = server(vec![too_large])?;
    let client = CatalogClient::loopback(&base, DEADLINE)?;
    assert_eq!(
        client.fetch_catalog(&Cancellation::default()).await,
        Err(MetadataError::TooLarge)
    );
    joined(worker)?;
    let mut too_large = reply(&vec![b' '; CATALOG_LIMIT + 1]);
    too_large.chunked = true;
    let (base, worker) = server(vec![token(), too_large])?;
    let client = CatalogClient::loopback(&base, DEADLINE)?;
    assert_eq!(
        client.fetch_catalog(&Cancellation::default()).await,
        Err(MetadataError::TooLarge)
    );
    joined(worker)?;
    Ok(())
}
#[tokio::test]
async fn cancellation_and_total_deadline_interrupt_waiting_responses() -> TestResult {
    // A deadline can expire before a connection is established. The fixture
    // must stop/join without requiring a request that never reached the wire.
    for timeout in [Duration::from_nanos(1), Duration::from_millis(30)] {
        let mut fixture = WaitingServer::new()?;
        let client = CatalogClient::loopback(&fixture.base, timeout)?;
        let start = std::time::Instant::now();
        assert_eq!(
            client.fetch_catalog(&Cancellation::default()).await,
            Err(MetadataError::Network)
        );
        assert!(start.elapsed() < Duration::from_millis(500));
        fixture.finish()?;
    }
    let mut fixture = WaitingServer::new()?;
    let client = CatalogClient::loopback(&fixture.base, DEADLINE)?;
    let cancel = Cancellation::default();
    let signal = cancel.clone();
    let cancellation = async {
        let observed = tokio::time::timeout(Duration::from_secs(5), &mut fixture.arrived).await;
        signal.cancel(); // Also release the client if the fixture failed to observe it.
        observed??;
        Ok::<(), Box<dyn Error>>(())
    };
    let (result, cancellation) = tokio::join!(client.fetch_catalog(&cancel), cancellation);
    cancellation?;
    assert_eq!(result, Err(MetadataError::Cancelled));
    assert!(
        fixture.finish()?,
        "request must arrive before in-flight cancellation"
    );
    Ok(())
}
#[tokio::test]
async fn pre_cancelled_request_retires_fixture_without_requiring_headers() -> TestResult {
    let mut fixture = WaitingServer::new()?;
    let client = CatalogClient::loopback(&fixture.base, DEADLINE)?;
    let cancel = Cancellation::default();
    cancel.cancel();
    assert_eq!(
        client.fetch_catalog(&cancel).await,
        Err(MetadataError::Cancelled)
    );
    // This checks owned teardown, not absence of TCP traffic: the fixture may
    // stop before queued sockets/bytes have been observed.
    fixture.finish()?;
    assert!(fixture.worker.is_none());
    Ok(())
}
#[tokio::test]
async fn tls_validates_roots_and_server_names_with_no_insecure_override() -> TestResult {
    use rustls::pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject};
    let ca = include_bytes!("../../tests/fixtures/tls/ca.cert.pem");
    let cert = include_bytes!("../../tests/fixtures/tls/fixture.cert.pem");
    let key = include_bytes!("../../tests/fixtures/tls/fixture.key.pem");
    for (trust, wrong_host) in [(false, false), (true, true), (true, false)] {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        listener.set_nonblocking(true)?;
        let address = listener.local_addr()?;
        let tls = rustls::ServerConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()?
        .with_no_client_auth()
        .with_single_cert(
            vec![CertificateDer::from_pem_slice(cert)?],
            PrivateKeyDer::from_pem_slice(key)?,
        )?;
        let worker = thread::spawn(move || -> std::io::Result<()> {
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            for response in [
                br#"{"response":{"token":{"server":"guest"}}}"#.as_slice(),
                b"{}".as_slice(),
            ] {
                let socket = loop {
                    match listener.accept() {
                        Ok((socket, _)) => break socket,
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            if std::time::Instant::now() > deadline {
                                return Err(std::io::Error::other("TLS fixture deadline"));
                            }
                            thread::sleep(Duration::from_millis(2));
                        }
                        Err(e) => return Err(e),
                    }
                };
                socket.set_nonblocking(false)?;
                socket.set_read_timeout(Some(Duration::from_secs(2)))?;
                let connection = rustls::ServerConnection::new(Arc::new(tls.clone()))
                    .map_err(std::io::Error::other)?;
                let mut stream = rustls::StreamOwned::new(connection, socket);
                let mut request = [0; 4096];
                if stream.read(&mut request).is_err() {
                    return Ok(());
                }
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    response.len()
                )?;
                stream.write_all(response)?;
                stream.flush()?;
            }
            Ok(())
        });
        let host = if wrong_host {
            "wrong.invalid"
        } else {
            "127.0.0.1"
        };
        let base = parse(&format!("https://{host}:{}/", address.port()))?;
        let endpoints = Endpoints {
            token: base.join("guest/create")?,
            catalog: base.join("v2/packs")?,
            artwork: base.join("lunii-data-prod")?,
        };
        let builder = Client::builder().no_proxy().resolve(host, address);
        let client = if trust {
            let mut roots = rustls::RootCertStore::empty();
            roots.add(CertificateDer::from_pem_slice(ca)?)?;
            let tls = rustls::ClientConfig::builder_with_provider(Arc::new(
                rustls::crypto::ring::default_provider(),
            ))
            .with_safe_default_protocol_versions()?
            .with_root_certificates(roots)
            .with_no_client_auth();
            CatalogClient::build_tls(endpoints, DEADLINE, builder, tls)?
        } else {
            CatalogClient::build(endpoints, DEADLINE, builder)?
        };
        let result = client.fetch_catalog(&Cancellation::default()).await;
        if trust && !wrong_host {
            assert_eq!(result?, b"{}");
        } else {
            assert_eq!(result, Err(MetadataError::Network));
        }
        worker.join().map_err(|_| "TLS fixture thread")??;
    }
    Ok(())
}
#[tokio::test]
async fn request_admission_is_bounded_without_waiting_queue() -> TestResult {
    let client = CatalogClient::loopback("http://127.0.0.1:9/", DEADLINE)?;
    let _first = client.permits.try_acquire()?;
    let _second = client.permits.try_acquire()?;
    assert_eq!(
        client.fetch_catalog(&Cancellation::default()).await,
        Err(MetadataError::Busy)
    );
    Ok(())
}
#[tokio::test]
async fn artwork_allowlist_stream_limit_sniffing_and_concurrent_misses() -> TestResult {
    use librairii_app::metadata::{
        ArtworkAsset, ArtworkRepository, MAX_ARTWORK_BYTES, RegisteredArtwork, artwork_url, digest,
    };
    use librairii_core::Checksum;
    use std::sync::Mutex;
    #[derive(Debug)]
    struct Repo {
        cache: Mutex<Option<ArtworkAsset>>,
        url: String,
        id: Checksum,
    }
    impl ArtworkRepository for Repo {
        fn registered_artwork(&self, id: Checksum) -> Result<RegisteredArtwork> {
            if id != self.id {
                return Err(MetadataError::ArtworkNotFound);
            }
            Ok(RegisteredArtwork {
                id,
                source_url: self.url.clone(),
                cached: self
                    .cache
                    .lock()
                    .map_err(|_| MetadataError::InvalidArtwork)?
                    .clone(),
            })
        }
        fn cache_artwork(
            &self,
            _: Checksum,
            asset: &ArtworkAsset,
            _: &str,
            _: Option<String>,
            _: Option<String>,
        ) -> Result<()> {
            *self
                .cache
                .lock()
                .map_err(|_| MetadataError::InvalidArtwork)? = Some(asset.clone());
            Ok(())
        }
    }
    let image = include_bytes!("../../tests/fixtures/artwork/cover.png");
    let mut response = reply(image);
    response.media = "image/png";
    response.delay = Duration::from_millis(30);
    let (base, worker) = server(vec![response])?;
    let url = artwork_url("/fixture/cover.png")?;
    let id = digest(url.as_bytes());
    let service = ArtworkService::new(
        Repo {
            cache: Mutex::new(None),
            url,
            id,
        },
        CatalogClient::loopback(&base, DEADLINE)?,
    );
    let cancel = Cancellation::default();
    let (a, b) = tokio::join!(
        service.load(id, "2026-10-21T12:00:00Z", &cancel),
        service.load(id, "2026-10-21T12:00:00Z", &cancel)
    );
    assert_eq!(a?.bytes.as_ref(), image);
    assert_eq!(b?.bytes.as_ref(), image);
    let requests = joined(worker)?;
    assert_eq!(requests.len(), 1);
    assert!(requests[0].starts_with("GET /lunii-data-prod/fixture/cover.png "));
    assert!(requests[0].contains("accept: image/png, image/jpeg, image/webp\r\n"));
    assert_eq!(
        service
            .load(Checksum::from_bytes([0; 32]), "invalid time", &cancel)
            .await,
        Err(MetadataError::ArtworkNotFound)
    );
    for (bytes, media, want) in [
        (
            b"not an image".to_vec(),
            "image/png",
            MetadataError::InvalidArtwork,
        ),
        (image.to_vec(), "text/plain", MetadataError::InvalidArtwork),
        (
            vec![0; MAX_ARTWORK_BYTES + 1],
            "image/png",
            MetadataError::TooLarge,
        ),
    ] {
        let mut r = reply(&bytes);
        r.media = media;
        r.chunked = true;
        let (base, worker) = server(vec![r])?;
        let client = CatalogClient::loopback(&base, DEADLINE)?;
        assert_eq!(
            client
                .fetch_artwork(&artwork_url("/fixture/cover.png")?, &cancel)
                .await
                .err(),
            Some(want)
        );
        joined(worker)?;
    }
    let client = CatalogClient::loopback("http://127.0.0.1:1/", DEADLINE)?;
    for url in [
        "https://outside/cover.png",
        "https://storage.googleapis.com/lunii-data-prod/../cover.png",
        "https://storage.googleapis.com/lunii-data-prod/cover.png?x",
        "https://storage.googleapis.com/lunii-data-prod-evil/cover.png",
    ] {
        assert_eq!(
            client.fetch_artwork(url, &cancel).await.err(),
            Some(MetadataError::InvalidArtwork)
        );
    }
    Ok(())
}

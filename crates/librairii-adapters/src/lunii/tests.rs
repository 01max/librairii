use super::*;
use std::{
    error::Error,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
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
    let mut slow = token();
    slow.delay = Duration::from_millis(150);
    let (base, worker) = server(vec![slow])?;
    let client = CatalogClient::loopback(&base, Duration::from_millis(30))?;
    let start = std::time::Instant::now();
    assert_eq!(
        client.fetch_catalog(&Cancellation::default()).await,
        Err(MetadataError::Network)
    );
    assert!(start.elapsed() < Duration::from_millis(140));
    joined(worker)?;
    let mut slow = token();
    slow.delay = Duration::from_millis(150);
    let (base, worker) = server(vec![slow])?;
    let client = CatalogClient::loopback(&base, DEADLINE)?;
    let cancel = Cancellation::default();
    let signal = cancel.clone();
    let cancellation = async move {
        tokio::time::sleep(Duration::from_millis(20)).await;
        signal.cancel();
    };
    let (result, ()) = tokio::join!(client.fetch_catalog(&cancel), cancellation);
    assert_eq!(result, Err(MetadataError::Cancelled));
    joined(worker)?;
    let cancel = Cancellation::default();
    cancel.cancel();
    assert_eq!(
        client.fetch_catalog(&cancel).await,
        Err(MetadataError::Cancelled)
    );
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

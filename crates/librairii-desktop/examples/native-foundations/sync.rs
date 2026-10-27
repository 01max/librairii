//! Real loopback HTTP, metadata files/SQLite and native production callbacks.
use super::{Fixture, Result, check};
use librairii_core::{OperationKind, operations::is_terminal};
use librairii_desktop::presentation::Snapshot;
use librairii_ui::native::DesktopWindow;
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
struct Server {
    address: String,
    count: Arc<AtomicUsize>,
    artwork: Arc<AtomicUsize>,
    mode: Arc<AtomicU8>,
    hold: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}
impl Server {
    fn new() -> Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        listener.set_nonblocking(true)?;
        let address = format!("http://{}/", listener.local_addr()?);
        let count = Arc::new(AtomicUsize::new(0));
        let artwork = Arc::new(AtomicUsize::new(0));
        let mode = Arc::new(AtomicU8::new(0));
        let hold = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));
        let (c, m, h, s) = (count.clone(), mode.clone(), hold.clone(), stop.clone());
        let artwork_count = artwork.clone();
        let mut cover = Vec::new();
        image::ImageEncoder::write_image(
            image::codecs::png::PngEncoder::new(&mut cover),
            &vec![73u8; 64 * 40 * 4],
            64,
            40,
            image::ExtendedColorType::Rgba8,
        )?;
        let raw = std::fs::read(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../librairii-app/tests/fixtures/lunii-catalog.json"),
        )?;
        let raw = String::from_utf8(raw)?
            .replace(
                "123e4567-e89b-42d3-a456-426614174000",
                "00112233-4455-4677-8899-aabbccddeeff",
            )
            .into_bytes();
        let join = thread::spawn(move || {
            while !s.load(Ordering::Acquire) {
                let (mut stream, _) = match listener.accept() {
                    Ok(v) => v,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2));
                        continue;
                    }
                    Err(_) => return,
                };
                let _timeout = stream.set_read_timeout(Some(Duration::from_secs(1)));
                let mut request = Vec::new();
                let mut buffer = [0u8; 1024];
                while request.len() < 8192 && !request.windows(4).any(|w| w == b"\r\n\r\n") {
                    match stream.read(&mut buffer) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => request.extend_from_slice(&buffer[..n]),
                    }
                }
                let is_artwork =
                    request.starts_with(b"GET /lunii-data-prod/fixture/clockwork-mountain.png ");
                if is_artwork {
                    artwork_count.fetch_add(1, Ordering::AcqRel);
                } else {
                    c.fetch_add(1, Ordering::AcqRel);
                }
                let catalog = request.starts_with(b"GET /v2/packs ");
                while catalog && h.load(Ordering::Acquire) && !s.load(Ordering::Acquire) {
                    thread::sleep(Duration::from_millis(2));
                }
                let failure = m.load(Ordering::Acquire) == 1;
                let body = if failure {
                    b"{}".as_slice()
                } else if is_artwork {
                    cover.as_slice()
                } else if catalog {
                    raw.as_slice()
                } else {
                    br#"{"response":{"token":{"server":"owned-fixture-token"}}}"#.as_slice()
                };
                let header = format!(
                    "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    if failure { "503 Unavailable" } else { "200 OK" },
                    if is_artwork {
                        "image/png"
                    } else {
                        "application/json"
                    },
                    body.len()
                );
                let _sent = stream
                    .write_all(header.as_bytes())
                    .and_then(|()| stream.write_all(body));
            }
        });
        Ok(Self {
            address,
            count,
            artwork,
            mode,
            hold,
            stop,
            join: Some(join),
        })
    }
    fn client(&self) -> Result<librairii_adapters::lunii::CatalogClient> {
        Ok(librairii_adapters::lunii::CatalogClient::loopback(
            &self.address,
            Duration::from_secs(5),
        )?)
    }
    fn count(&self) -> usize {
        self.count.load(Ordering::Acquire)
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.hold.store(false, Ordering::Release);
        if let Some(join) = self.join.take() {
            let _joined = join.join();
        }
    }
}
fn complete(f: &Fixture, ui: &DesktopWindow) -> Result<Snapshot> {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let snapshot = f.settle(ui)?;
        let data = &snapshot.workflows.operations;
        if data.preparing.is_none()
            && data
                .status
                .is_some_and(|status| !status.starting && status.running == 0 && status.queued == 0)
            && data.details.as_ref().is_some_and(|d| {
                d.card.operation.kind == OperationKind::MetadataSync
                    && is_terminal(d.card.operation.state)
            })
        {
            return Ok(snapshot);
        }
        if Instant::now() > deadline {
            return Err("native metadata job did not finish".into());
        }
    }
}
pub fn checks(ui: &DesktopWindow) -> Result {
    while ui.get_dialog() != 0 {
        ui.invoke_close_dialog();
    }
    let server = Server::new()?;
    let mut f = Fixture::open_catalog(ui, server.client()?)?;
    check(
        ui.get_sync_data().loaded
            && ui.get_sync_data().can_start
            && ui.get_sync_data().last_success.contains("Never")
            && server.count() == 0,
        "first sync prompt/status performed implicit networking",
    )?;
    ui.invoke_refresh_sync();
    f.settle(ui)?;
    ui.invoke_hide_sync(true);
    f.settle(ui)?;
    ui.invoke_hide_sync(false);
    f.settle(ui)?;
    check(
        server.count() == 0,
        "local status or show/hide fetched the catalog",
    )?;
    server.mode.store(1, Ordering::Release);
    ui.invoke_start_sync();
    complete(&f, ui)?;
    check(
        ui.get_sync_data().status.contains("unavailable")
            && !ui.get_sync_data().status.contains("previous working"),
        "first failed sync claimed a nonexistent catalog",
    )?;
    server.mode.store(0, Ordering::Release);
    server.hold.store(true, Ordering::Release);
    ui.invoke_start_sync();
    f.settle(ui)?;
    f.search(ui, "Clockwork")?;
    f.select(ui, 0)?;
    ui.invoke_focus_sync();
    ui.invoke_open_dialog(1);
    check(ui.get_close_focused(), "About did not own native focus")?;
    server.hold.store(false, Ordering::Release);
    let succeeded = complete(&f, ui)?;
    check(
        ui.get_close_focused(),
        "metadata completion stole About focus",
    )?;
    check(
        succeeded
            .workflows
            .details
            .as_ref()
            .is_some_and(|d| d.story.summary.description == "A sanitized catalog fixture."),
        "metadata activation left owned Details stale",
    )?;
    ui.invoke_close_dialog();
    let cover_deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let snapshot = f.view.borrow().snapshot.clone();
        if snapshot.page.as_ref().is_some_and(|page| {
            page.all_stories().any(|story| {
                story
                    .thumbnail
                    .as_ref()
                    .is_some_and(|t| t.width > 0 && t.width * 40 == t.height * 64)
            })
        }) {
            break;
        }
        if Instant::now() >= cover_deadline {
            return Err("lazy official cover did not replace the embedded fallback".into());
        }
        f.settle(ui)?;
    }
    check(
        server.artwork.load(Ordering::Acquire) == 1,
        "lazy official cover misses were not coalesced",
    )?;
    let after_success = server.count();
    let stamp = succeeded
        .workflows
        .operations
        .catalog
        .as_ref()
        .and_then(|c| c.activated_at.clone())
        .ok_or("successful sync stamp missing")?;
    let revision = succeeded.workflows.operations.catalog_revision;
    check(
        server.count() == after_success
            && succeeded.workflows.operations.last_success_local.is_some()
            && ui.get_sync_data().can_start,
        "explicit sync did not activate metadata and retain a local timestamp",
    )?;
    ui.invoke_change_language("fr".into());
    f.settle(ui)?;
    ui.invoke_refresh_sync();
    f.settle(ui)?;
    check(
        server.count() == after_success
            && ui
                .get_sync_data()
                .last_success
                .starts_with("Dernière synchronisation réussie"),
        "language/status refresh fetched metadata or lost localization",
    )?;
    server.mode.store(1, Ordering::Release);
    ui.invoke_start_sync();
    let failed = complete(&f, ui)?;
    check(
        failed
            .workflows
            .operations
            .catalog
            .as_ref()
            .and_then(|c| c.activated_at.as_ref())
            == Some(&stamp)
            && failed.workflows.operations.catalog_revision == revision
            && ui.get_sync_data().status.contains("échoué")
            && ui.get_sync_data().start_label.contains("Réessayer"),
        "failed sync lost the last-good catalog/stamp or retry status",
    )?;
    server.mode.store(0, Ordering::Release);
    server.hold.store(true, Ordering::Release);
    let before = server.count();
    ui.invoke_start_sync();
    f.settle(ui)?;
    let deadline = Instant::now() + Duration::from_secs(5);
    while server.count() < before + 2 || !ui.get_sync_data().cancellable {
        ui.invoke_refresh_sync();
        f.settle(ui)?;
        if Instant::now() > deadline {
            return Err("metadata request did not reach its cancellable response".into());
        }
    }
    ui.invoke_start_sync();
    let refused = f.settle(ui)?;
    check(
        refused.workflows.operations.error.is_some() && server.count() == before + 2,
        "second active sync was admitted or fetched again",
    )?;
    ui.invoke_cancel_sync();
    let cancelled = complete(&f, ui)?;
    server.hold.store(false, Ordering::Release);
    check(
        cancelled
            .workflows
            .operations
            .catalog
            .as_ref()
            .and_then(|c| c.activated_at.as_ref())
            == Some(&stamp)
            && cancelled.workflows.operations.catalog_revision == revision
            && ui.get_sync_data().status.contains("annulée"),
        "cancelled sync lost the last successful timestamp or status",
    )?;
    let count = server.count();
    ui.invoke_hide_sync(true);
    f.settle(ui)?;
    check(ui.get_sync_data().hidden, "sync panel did not hide")?;
    f.restart(ui)?;
    check(
        ui.get_sync_data().hidden
            && server.count() == count
            && server.artwork.load(Ordering::Acquire) == 1,
        "sync panel dismissal was not durable or restart fetched metadata",
    )?;
    ui.invoke_hide_sync(false);
    f.settle(ui)?;
    check(
        !ui.get_sync_data().hidden && server.count() == count,
        "Show sync implicitly refreshed metadata",
    )?;
    f.worker.borrow_mut().finish()?;
    let preference = f.locations.native.join("interface.json");
    let readable = std::fs::read(&preference)?;
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&preference, std::fs::Permissions::from_mode(0o0))?;
    f.restart(ui)?;
    let unreadable_safe = f.view.borrow().snapshot.phase
        == librairii_desktop::presentation::Phase::Library
        && ui.get_sync_data().notice.contains("session");
    f.worker.borrow_mut().finish()?;
    std::fs::set_permissions(&preference, std::fs::Permissions::from_mode(0o600))?;
    check(
        unreadable_safe && std::fs::read(&preference)? == readable,
        "unreadable optional preferences blocked browsing or changed bytes",
    )?;
    let mut wire: serde_json::Value = serde_json::from_slice(&std::fs::read(&preference)?)?;
    wire["version"] = 99.into();
    let bytes = serde_json::to_vec(&wire)?;
    std::fs::write(&preference, &bytes)?;
    f.restart(ui)?;
    check(
        f.view.borrow().snapshot.phase == librairii_desktop::presentation::Phase::Library
            && ui.get_sync_data().notice.contains("session")
            && std::fs::read(&preference)? == bytes,
        "future interface preferences caused library loss or were overwritten",
    )?;
    ui.invoke_hide_sync(true);
    f.settle(ui)?;
    check(
        std::fs::read(&preference)? == bytes && server.count() == count,
        "session fallback overwrote future preferences or fetched metadata",
    )?;
    f.finish()
}
pub struct Surface {
    fixture: Fixture,
    _server: Server,
}
impl std::fmt::Debug for Surface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SyncSurface").finish_non_exhaustive()
    }
}
impl Surface {
    pub fn open(ui: &DesktopWindow) -> Result<Self> {
        let server = Server::new()?;
        let fixture = Fixture::open_catalog(ui, server.client()?)?;
        Ok(Self {
            fixture,
            _server: server,
        })
    }
    pub fn success(&self, ui: &DesktopWindow) -> Result {
        ui.invoke_start_sync();
        complete(&self.fixture, ui)?;
        Ok(())
    }
    pub fn french_failure(&self, ui: &DesktopWindow) -> Result {
        ui.invoke_change_language("fr".into());
        self.fixture.settle(ui)?;
        self._server.mode.store(1, Ordering::Release);
        ui.invoke_start_sync();
        complete(&self.fixture, ui)?;
        Ok(())
    }
    pub fn poll(&self, ui: &DesktopWindow) {
        if let Some(snapshot) = self.fixture.worker.borrow().take_snapshot() {
            self.fixture.view.borrow_mut().accept(
                ui,
                snapshot,
                self.fixture.worker.borrow().revision(),
            );
        }
        self.fixture.view.borrow_mut().resize(ui);
    }
}

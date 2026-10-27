//! The sole owner of public Lunii endpoints/protocol. No account, device,
//! firmware, archive download or upload port is exposed. Errors redact URLs,
//! response bodies and guest credentials.
use librairii_core::metadata::MetadataError;
use reqwest::{
    Client, Url,
    header::{ACCEPT, CONTENT_TYPE, HeaderMap, HeaderValue, USER_AGENT},
};
use rustls_platform_verifier::BuilderVerifierExt;
use serde::Deserialize;
use std::{sync::Arc, time::Duration};
use tokio::sync::{Semaphore, watch};
type Result<T> = std::result::Result<T, MetadataError>;
pub const TOKEN_LIMIT: usize = 64 * 1024;
pub const CATALOG_LIMIT: usize = 32 * 1024 * 1024;
const DEADLINE: Duration = Duration::from_secs(30);
const USER_AGENT_VALUE: &str = "Librairii/metadata-sync";
#[derive(Clone, Debug)]
pub struct Cancellation {
    signal: watch::Sender<bool>,
}
impl Default for Cancellation {
    fn default() -> Self {
        let (signal, _) = watch::channel(false);
        Self { signal }
    }
}
impl Cancellation {
    pub fn cancel(&self) {
        self.signal.send_replace(true);
    }
    pub fn is_cancelled(&self) -> bool {
        *self.signal.borrow()
    }
    pub async fn cancelled(&self) {
        let mut receiver = self.signal.subscribe();
        let _ = receiver.wait_for(|v| *v).await;
    }
}
#[derive(Clone, Debug)]
struct Endpoints {
    token: Url,
    catalog: Url,
    artwork: Url,
}
#[derive(Clone, Debug)]
pub struct CatalogClient {
    client: Client,
    endpoints: Endpoints,
    permits: Arc<Semaphore>,
}
impl CatalogClient {
    /// Called only by an explicitly requested native metadata job. The current
    /// thread drives HTTP; no background scheduler or detached request escapes.
    pub fn fetch_for_job(&self, job: &librairii_app::native_jobs::Cancellation) -> Result<Vec<u8>> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| MetadataError::InvalidConfiguration)?;
        let cancel = Cancellation::default();
        runtime.block_on(async {
            let request=self.fetch_catalog(&cancel);tokio::pin!(request);
            loop {tokio::select! {biased; result=&mut request=>return result,_=tokio::time::sleep(Duration::from_millis(10))=>{if job.is_cancelled(){cancel.cancel();}}}}
        })
    }
    pub fn production() -> Result<Self> {
        Self::build(
            Endpoints {
                token: parse("https://server-auth-prod.lunii.com/guest/create")?,
                catalog: parse("https://server-data-prod.lunii.com/v2/packs")?,
                artwork: parse("https://storage.googleapis.com/lunii-data-prod")?,
            },
            DEADLINE,
            Client::builder(),
        )
    }
    /// Explicit isolated fixture route. Overrides cannot point at remote hosts.
    /// The server must be a numeric loopback address; no DNS substitution.
    pub fn loopback(base: &str, timeout: Duration) -> Result<Self> {
        let base = parse(base)?;
        if base.scheme() != "http"
            || !numeric_loopback(&base)
            || base.path() != "/"
            || base.query().is_some()
            || base.fragment().is_some()
            || !base.username().is_empty()
            || base.password().is_some()
        {
            return Err(MetadataError::InvalidConfiguration);
        }
        Self::build(
            Endpoints {
                token: base
                    .join("guest/create")
                    .map_err(|_| MetadataError::InvalidConfiguration)?,
                catalog: base
                    .join("v2/packs")
                    .map_err(|_| MetadataError::InvalidConfiguration)?,
                artwork: base
                    .join("lunii-data-prod")
                    .map_err(|_| MetadataError::InvalidConfiguration)?,
            },
            timeout,
            Client::builder().no_proxy(),
        )
    }
    fn build(
        endpoints: Endpoints,
        timeout: Duration,
        builder: reqwest::ClientBuilder,
    ) -> Result<Self> {
        if timeout.is_zero() || timeout > DEADLINE {
            return Err(MetadataError::InvalidConfiguration);
        }
        let tls = rustls::ClientConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_protocol_versions(&[&rustls::version::TLS13, &rustls::version::TLS12])
        .map_err(|_| MetadataError::InvalidConfiguration)?
        .with_platform_verifier()
        .map_err(|_| MetadataError::InvalidConfiguration)?
        .with_no_client_auth();
        Self::build_tls(endpoints, timeout, builder, tls)
    }
    fn build_tls(
        endpoints: Endpoints,
        timeout: Duration,
        builder: reqwest::ClientBuilder,
        tls: rustls::ClientConfig,
    ) -> Result<Self> {
        let client = builder
            .tls_backend_preconfigured(tls)
            .redirect(reqwest::redirect::Policy::none())
            .referer(false)
            .timeout(timeout)
            .connect_timeout(Duration::from_secs(10).min(timeout))
            .read_timeout(Duration::from_secs(15).min(timeout))
            .pool_idle_timeout(Duration::from_secs(30))
            .pool_max_idle_per_host(2)
            .retry(reqwest::retry::never())
            .no_gzip()
            .no_brotli()
            .no_deflate()
            .no_zstd()
            .build()
            .map_err(|_| MetadataError::InvalidConfiguration)?;
        Ok(Self {
            client,
            endpoints,
            permits: Arc::new(Semaphore::new(2)),
        })
    }
    pub async fn fetch_catalog(&self, cancel: &Cancellation) -> Result<Vec<u8>> {
        let _permit = self
            .permits
            .try_acquire()
            .map_err(|_| MetadataError::Busy)?;
        let token_body = self
            .json(
                self.endpoints.token.clone(),
                HeaderMap::new(),
                TOKEN_LIMIT,
                cancel,
            )
            .await?;
        #[derive(Deserialize)]
        struct Envelope {
            response: TokenResponse,
        }
        #[derive(Deserialize)]
        struct TokenResponse {
            token: Token,
        }
        #[derive(Deserialize)]
        struct Token {
            server: String,
        }
        let parsed: Envelope =
            serde_json::from_slice(&token_body).map_err(|_| MetadataError::InvalidToken)?;
        let token = parsed.response.token.server.trim();
        if token.is_empty() || token.contains(['\r', '\n']) {
            return Err(MetadataError::InvalidToken);
        }
        let mut headers = HeaderMap::new();
        headers.insert(
            "application-sender",
            HeaderValue::from_static("luniistore_desktop"),
        );
        let mut value = HeaderValue::from_str(token).map_err(|_| MetadataError::InvalidToken)?;
        value.set_sensitive(true);
        headers.insert("x-auth-token", value);
        self.json(
            self.endpoints.catalog.clone(),
            headers,
            CATALOG_LIMIT,
            cancel,
        )
        .await
    }
    async fn json(
        &self,
        url: Url,
        headers: HeaderMap,
        limit: usize,
        cancel: &Cancellation,
    ) -> Result<Vec<u8>> {
        let mut response = self.send(url, headers, "application/json", cancel).await?;
        if let Some(value) = response.headers().get(CONTENT_TYPE) {
            let media = value
                .to_str()
                .map_err(|_| MetadataError::InvalidJson)?
                .parse::<mime::Mime>()
                .map_err(|_| MetadataError::InvalidJson)?;
            if media.essence_str() != "application/json" && media.suffix() != Some(mime::JSON) {
                return Err(MetadataError::InvalidJson);
            }
        }
        let body = read_body(&mut response, limit, cancel).await?;
        let mut decoder = serde_json::Deserializer::from_slice(&body);
        serde::de::IgnoredAny::deserialize(&mut decoder).map_err(|_| MetadataError::InvalidJson)?;
        decoder.end().map_err(|_| MetadataError::InvalidJson)?;
        Ok(body)
    }
    async fn send(
        &self,
        url: Url,
        headers: HeaderMap,
        accept: &'static str,
        cancel: &Cancellation,
    ) -> Result<reqwest::Response> {
        if cancel.is_cancelled() {
            return Err(MetadataError::Cancelled);
        }
        let request = self
            .client
            .get(url)
            .headers(headers)
            .header(ACCEPT, accept)
            .header(USER_AGENT, USER_AGENT_VALUE)
            .send();
        let response = tokio::select! {biased;_=cancel.cancelled()=>return Err(MetadataError::Cancelled),result=request=>result.map_err(|_|MetadataError::Network)?};
        if response.status().is_redirection() {
            return Err(MetadataError::Redirect);
        }
        if !response.status().is_success() {
            return Err(MetadataError::Status);
        }
        Ok(response)
    }
}
async fn read_body(
    response: &mut reqwest::Response,
    limit: usize,
    cancel: &Cancellation,
) -> Result<Vec<u8>> {
    let mut body = Vec::new();
    loop {
        let chunk = tokio::select! {biased;_=cancel.cancelled()=>return Err(MetadataError::Cancelled),chunk=response.chunk()=>chunk.map_err(|_|MetadataError::Network)?};
        let Some(chunk) = chunk else {
            break;
        };
        if chunk.len() > limit.saturating_sub(body.len()) {
            return Err(MetadataError::TooLarge);
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}
fn parse(value: &str) -> Result<Url> {
    Url::parse(value).map_err(|_| MetadataError::InvalidConfiguration)
}
fn numeric_loopback(url: &Url) -> bool {
    url.host_str()
        .and_then(|host| {
            host.trim_matches(['[', ']'])
                .parse::<std::net::IpAddr>()
                .ok()
        })
        .is_some_and(|ip| ip.is_loopback())
}
impl CatalogClient {
    pub fn artwork_base(&self) -> &str {
        self.endpoints.artwork.as_str()
    }
}
#[cfg(test)]
mod tests;

#[derive(Clone, Debug)]
pub struct ArtworkDownload {
    pub asset: librairii_app::metadata::ArtworkAsset,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}
impl CatalogClient {
    pub async fn fetch_artwork(
        &self,
        source: &str,
        cancel: &Cancellation,
    ) -> Result<ArtworkDownload> {
        use librairii_app::metadata::{
            ARTWORK_BASE, ArtworkAsset, MAX_ARTWORK_BYTES, MediaType, artwork_url,
        };
        // Validate the original bytes before URL parsing can remove traversal.
        let path = source
            .strip_prefix(ARTWORK_BASE)
            .ok_or(MetadataError::InvalidArtwork)?;
        if artwork_url(path)? != source {
            return Err(MetadataError::InvalidArtwork);
        }
        let url = parse(&format!(
            "{}{path}",
            self.endpoints.artwork.as_str().trim_end_matches('/')
        ))?;
        let _permit = self
            .permits
            .try_acquire()
            .map_err(|_| MetadataError::Busy)?;
        let mut response = self
            .send(
                url,
                HeaderMap::new(),
                "image/png, image/jpeg, image/webp",
                cancel,
            )
            .await?;
        let media = response
            .headers()
            .get(CONTENT_TYPE)
            .ok_or(MetadataError::InvalidArtwork)?
            .to_str()
            .map_err(|_| MetadataError::InvalidArtwork)?
            .parse::<mime::Mime>()
            .map_err(|_| MetadataError::InvalidArtwork)?;
        let media = MediaType::parse(media.essence_str())?;
        let etag = optional_header(response.headers(), "etag")?;
        let last_modified = optional_header(response.headers(), "last-modified")?;
        let body = read_body(&mut response, MAX_ARTWORK_BYTES, cancel).await?;
        Ok(ArtworkDownload {
            asset: ArtworkAsset::checked(body, media)?,
            etag,
            last_modified,
        })
    }
}
fn optional_header(headers: &HeaderMap, name: &str) -> Result<Option<String>> {
    headers
        .get(name)
        .map(|v| {
            v.to_str()
                .map(str::to_owned)
                .map_err(|_| MetadataError::InvalidArtwork)
        })
        .transpose()
}
/// One service is retained per open library; its single miss lock coalesces
/// downloads, with a 64-caller admission ceiling. Do not drive synchronous
/// repository operations on a UI executor. No request starts on construction.
#[derive(Debug)]
pub struct ArtworkService<R> {
    repository: R,
    client: CatalogClient,
    miss: tokio::sync::Mutex<()>,
    callers: Semaphore,
}
impl<R: librairii_app::metadata::ArtworkRepository> ArtworkService<R> {
    pub fn new(repository: R, client: CatalogClient) -> Self {
        Self {
            repository,
            client,
            miss: tokio::sync::Mutex::new(()),
            callers: Semaphore::new(64),
        }
    }
    /// Drive one lazy request on its owned blocking worker. Cancellation remains
    /// connected while HTTP is pending; no task outlives this call.
    pub fn load_blocking(
        &self,
        id: librairii_core::Checksum,
        at: &str,
        stopping: &std::sync::atomic::AtomicBool,
    ) -> Result<librairii_app::metadata::ArtworkAsset> {
        if stopping.load(std::sync::atomic::Ordering::Acquire) {
            return Err(MetadataError::Cancelled);
        }
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| MetadataError::InvalidConfiguration)?;
        let cancel = Cancellation::default();
        runtime.block_on(async {
            let request = self.load(id, at, &cancel);
            tokio::pin!(request);
            loop {
                tokio::select! {
                    biased;
                    _ = tokio::time::sleep(Duration::from_millis(10)) => {
                        if stopping.load(std::sync::atomic::Ordering::Acquire) {
                            cancel.cancel();
                        }
                    }
                    result = &mut request => return result,
                }
            }
        })
    }
    pub async fn load(
        &self,
        id: librairii_core::Checksum,
        at: &str,
        cancel: &Cancellation,
    ) -> Result<librairii_app::metadata::ArtworkAsset> {
        let _slot = self
            .callers
            .try_acquire()
            .map_err(|_| MetadataError::Busy)?;
        if cancel.is_cancelled() {
            return Err(MetadataError::Cancelled);
        }
        if let Some(asset) = self.repository.registered_artwork(id)?.cached {
            return Ok(asset);
        }
        let _miss = tokio::select! {biased;_=cancel.cancelled()=>return Err(MetadataError::Cancelled),guard=self.miss.lock()=>guard};
        let registered = self.repository.registered_artwork(id)?;
        if let Some(asset) = registered.cached {
            return Ok(asset);
        }
        let download = self
            .client
            .fetch_artwork(&registered.source_url, cancel)
            .await?;
        if cancel.is_cancelled() {
            return Err(MetadataError::Cancelled);
        }
        self.repository.cache_artwork(
            id,
            &download.asset,
            at,
            download.etag,
            download.last_modified,
        )?;
        Ok(download.asset)
    }
}

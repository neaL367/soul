//! High-level asynchronous HTTP client with connection pooling, TLS, HTTP/2, and redirect resolution.

pub mod http2;
pub mod transport;

use crate::cache_validator::CacheValidator;
use crate::cors::CorsEvaluator;
use crate::dns::DnsResolver;
use crate::error::NetworkError;
use crate::mixed_content::is_insecure_mixed_content;
use crate::types::{HttpMethod, HttpRequest, HttpResponse};
use std::sync::Arc;
use std::time::Duration;
pub(crate) use transport::RawResponse;
use url::Url;

/// Maximum number of redirects followed before returning `NetworkError::TooManyRedirects`.
pub const MAX_REDIRECTS: usize = 20;

/// Default connect/read timeout for the HTTP client.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Default DNS cache entry TTL.
pub const DEFAULT_DNS_TTL: Duration = Duration::from_mins(5);

/// Configuration parameters for `HttpClient`.
#[derive(Debug, Clone)]
pub struct HttpClientConfig {
    /// Overall request timeout.
    pub timeout: Duration,
    /// User-Agent string sent with outgoing requests.
    pub user_agent: String,
    /// DNS cache entry time-to-live.
    pub dns_ttl: Duration,
}

impl Default for HttpClientConfig {
    fn default() -> Self {
        Self {
            timeout: DEFAULT_TIMEOUT,
            user_agent: "Soul/0.1.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36".to_string(),
            dns_ttl: DEFAULT_DNS_TTL,
        }
    }
}

/// Asynchronous HTTP client supporting plain TCP, HTTP/1.1, HTTP/2 multiplexing, DNS caching, and TLS 1.2/1.3.
#[derive(Clone)]
pub struct HttpClient {
    config: HttpClientConfig,
    tls_config: Arc<rustls::ClientConfig>,
    dns_resolver: DnsResolver,
    cache_validator: CacheValidator,
    hsts_store: Option<Arc<storage::HstsStore>>,
}

impl Default for HttpClient {
    fn default() -> Self {
        Self::new(HttpClientConfig::default())
    }
}

/// Creates a standard Rustls client configuration using system webpki roots.
#[must_use]
pub fn create_default_tls_config() -> rustls::ClientConfig {
    let mut root_store = rustls::RootCertStore::empty();
    root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    rustls::ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth()
}

impl HttpClient {
    /// Creates a new `HttpClient` with the given configuration.
    #[must_use]
    pub fn new(config: HttpClientConfig) -> Self {
        let mut tls_config = create_default_tls_config();
        tls_config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

        let dns_resolver = DnsResolver::new(config.dns_ttl);

        Self {
            config,
            tls_config: Arc::new(tls_config),
            dns_resolver,
            cache_validator: CacheValidator::default(),
            hsts_store: None,
        }
    }

    /// Attaches an `SQLite` RFC 9111 HTTP cache store to this client.
    #[must_use]
    pub fn with_cache_store(mut self, store: Arc<storage::HttpCacheStore>) -> Self {
        self.cache_validator = CacheValidator::new(Some(store));
        self
    }

    /// Attaches an RFC 6797 HSTS persistent security store to this client.
    #[must_use]
    pub fn with_hsts_store(mut self, store: Arc<storage::HstsStore>) -> Self {
        self.hsts_store = Some(store);
        self
    }

    /// Returns a reference to the DNS resolver used by this client.
    #[must_use]
    pub const fn dns_resolver(&self) -> &DnsResolver {
        &self.dns_resolver
    }

    /// Fetches a URL with a standard HTTP GET request.
    ///
    /// # Errors
    /// Returns `NetworkError` if connection, TLS handshake, or protocol exchange fails.
    pub async fn fetch(&self, url: &Url) -> Result<HttpResponse, NetworkError> {
        self.fetch_request(&HttpRequest::get(url.clone())).await
    }

    /// Streams a URL's response body without buffering it in memory, following
    /// redirects. Intended for large transfers such as file downloads.
    ///
    /// The timeout bounds only the handshake and redirect resolution, not the
    /// (unbounded-duration) body transfer. The wire body is capped at
    /// [`crate::streaming::MAX_DOWNLOAD_BYTES`].
    ///
    /// # Errors
    /// Returns `NetworkError` if connection, TLS handshake, redirect
    /// resolution, or the timeout budget is exceeded.
    pub async fn fetch_streaming(
        &self,
        url: &Url,
    ) -> Result<crate::streaming::StreamingResponse, NetworkError> {
        let (raw, final_url) = tokio::time::timeout(
            self.config.timeout,
            self.send_with_redirects(&HttpRequest::get(url.clone()), None),
        )
        .await
        .map_err(|_| NetworkError::Timeout)??;

        Ok(crate::streaming::build_streaming_response(raw, final_url))
    }

    /// Fetches a subresource request initiated by a document, verifying CORS policy.
    ///
    /// # Errors
    /// Returns `NetworkError::CorsViolation` if the response headers forbid
    /// access from `document_origin`.
    pub async fn fetch_with_security_context(
        &self,
        request: &HttpRequest,
        document_origin: Option<&Url>,
    ) -> Result<HttpResponse, NetworkError> {
        let response = tokio::time::timeout(
            self.config.timeout,
            self.fetch_request_inner(request, document_origin),
        )
        .await
        .map_err(|_| NetworkError::Timeout)??;

        if let Some(doc_origin) = document_origin
            && response.url.origin() != doc_origin.origin()
            && !CorsEvaluator::is_allowed(doc_origin, &response.headers)
        {
            return Err(NetworkError::CorsViolation(doc_origin.to_string()));
        }

        Ok(response)
    }

    /// Executes an arbitrary `HttpRequest` over HTTP/1.1 or HTTP/2, following redirects.
    ///
    /// # Errors
    /// Returns `NetworkError` if connection, TLS handshake, protocol exchange,
    /// redirect resolution, the redirect limit, or the timeout budget is exceeded.
    pub async fn fetch_request(&self, request: &HttpRequest) -> Result<HttpResponse, NetworkError> {
        tokio::time::timeout(self.config.timeout, self.fetch_request_inner(request, None))
            .await
            .map_err(|_| NetworkError::Timeout)?
    }

    /// Executes an arbitrary `HttpRequest` without a client-level timeout,
    /// enforcing mixed-content rules against `document_origin` at every hop.
    async fn fetch_request_inner(
        &self,
        request: &HttpRequest,
        document_origin: Option<&Url>,
    ) -> Result<HttpResponse, NetworkError> {
        let mut req = request.clone();

        // RFC 6797 HSTS auto-upgrade from HTTP to HTTPS
        if req.url.scheme() == "http"
            && let Some(host) = req.url.host_str()
            && let Some(hsts) = &self.hsts_store
            && hsts.is_hsts_enforced(host).unwrap_or(false)
        {
            let _ = req.url.set_scheme("https");
        }

        if let Some(cached_resp) = self.cache_validator.prepare_request(&mut req) {
            return Ok(cached_resp);
        }
        let (response, final_url) = self.send_with_redirects(&req, document_origin).await?;
        let collected = self.collect_response(response, final_url.clone()).await?;

        // Record HSTS header if present
        if final_url.scheme() == "https"
            && let Some(host) = final_url.host_str()
            && let Some(hsts_val) = collected.headers.get("strict-transport-security")
            && let Some(hsts) = &self.hsts_store
            && let Some((max_age, inc_sub)) = storage::HstsStore::parse_hsts_header(hsts_val)
        {
            let _ = hsts.record_hsts(host, max_age, inc_sub);
        }

        Ok(self.cache_validator.handle_response(&req, collected))
    }

    /// Follows redirects (up to [`MAX_REDIRECTS`]) and returns the final raw
    /// response together with the URL it was actually served from.
    async fn send_with_redirects(
        &self,
        request: &HttpRequest,
        document_origin: Option<&Url>,
    ) -> Result<(RawResponse, Url), NetworkError> {
        let mut current = request.clone();

        for _hop in 0..=MAX_REDIRECTS {
            if let Some(doc_origin) = document_origin
                && is_insecure_mixed_content(doc_origin, &current.url)
            {
                return Err(NetworkError::MixedContentBlocked(
                    current.url.to_string(),
                    doc_origin.to_string(),
                ));
            }

            let response = self.execute_request(&current).await?;
            let status = response.status().as_u16();
            let location = response
                .headers()
                .get(http::header::LOCATION)
                .and_then(|val| val.to_str().ok());

            if (300..=399).contains(&status)
                && let Some(location) = location
            {
                let next_url = current
                    .url
                    .join(location)
                    .map_err(|e| NetworkError::InvalidRedirect(current.url.to_string(), e))?;

                let method = if matches!(status, 301..=303) && current.method == HttpMethod::Post {
                    HttpMethod::Get
                } else {
                    current.method
                };
                let body = if method == HttpMethod::Get {
                    None
                } else {
                    current.body.clone()
                };

                tracing::info!(from = %current.url, to = %next_url, status, "Following redirect");
                let cross_origin = next_url.origin() != current.url.origin();
                let headers = if cross_origin {
                    current
                        .headers
                        .iter()
                        .filter(|(name, _)| {
                            !matches!(
                                name.to_ascii_lowercase().as_str(),
                                "authorization" | "cookie" | "proxy-authorization"
                            )
                        })
                        .cloned()
                        .collect()
                } else {
                    current.headers.clone()
                };
                current = HttpRequest {
                    url: next_url,
                    method,
                    headers,
                    body,
                };
                continue;
            }

            return Ok((response, current.url.clone()));
        }

        Err(NetworkError::TooManyRedirects(current.url.to_string()))
    }

    async fn collect_response(
        &self,
        response: RawResponse,
        final_url: Url,
    ) -> Result<HttpResponse, NetworkError> {
        transport::collect_response(response, final_url).await
    }

    async fn execute_request(&self, request: &HttpRequest) -> Result<RawResponse, NetworkError> {
        transport::execute_request(
            request,
            &self.config.user_agent,
            &self.tls_config,
            &self.dns_resolver,
        )
        .await
    }
}

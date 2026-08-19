//! HTTP/1.1 and TLS 1.2/1.3 client implementation using Hyper, Tokio, and Rustls.

use crate::cors::CorsEvaluator;
use crate::decompression::MAX_DECOMPRESSED_BYTES;
use crate::error::NetworkError;
use crate::mixed_content::is_insecure_mixed_content;
use crate::types::{HttpMethod, HttpRequest, HttpResponse};
use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Full};
use hyper::client::conn::http1;
use hyper_util::rt::TokioIo;
use rustls::pki_types::ServerName;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;
use url::Url;

use crate::dns::DnsResolver;

/// Maximum redirect hops followed before failing with `NetworkError::TooManyRedirects`.
pub const MAX_REDIRECTS: usize = 5;

/// A raw HTTP response whose body is still streamable, before any collection.
pub(crate) type RawResponse = http::Response<BoxBody<Bytes, hyper::Error>>;

/// Client configuration for network and TLS settings.
#[derive(Clone)]
pub struct HttpClientConfig {
    /// User-Agent header value.
    pub user_agent: String,
    /// Request timeout duration.
    pub timeout: std::time::Duration,
    /// DNS cache TTL duration.
    pub dns_ttl: std::time::Duration,
}

impl Default for HttpClientConfig {
    fn default() -> Self {
        Self {
            user_agent: "Soul/0.1 (Windows NT 10.0; Win64; x64)".to_string(),
            timeout: std::time::Duration::from_secs(30),
            dns_ttl: std::time::Duration::from_mins(5),
        }
    }
}

/// Asynchronous HTTP/1.1 client supporting plain TCP, asynchronous DNS caching, and TLS 1.2/1.3.
#[derive(Clone)]
pub struct HttpClient {
    config: HttpClientConfig,
    tls_config: Arc<rustls::ClientConfig>,
    dns_resolver: DnsResolver,
}

impl Default for HttpClient {
    fn default() -> Self {
        Self::new(HttpClientConfig::default())
    }
}

impl HttpClient {
    /// Creates a new `HttpClient` with the given configuration.
    #[must_use]
    pub fn new(config: HttpClientConfig) -> Self {
        let mut root_store = rustls::RootCertStore::empty();
        root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());

        let mut tls_config = rustls::ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth();
        tls_config.alpn_protocols = vec![b"http/1.1".to_vec()];

        let dns_resolver = DnsResolver::new(config.dns_ttl);

        Self {
            config,
            tls_config: Arc::new(tls_config),
            dns_resolver,
        }
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
        let request = HttpRequest::get(url.clone());
        let (response, final_url) = tokio::time::timeout(
            self.config.timeout,
            self.send_with_redirects(&request, None),
        )
        .await
        .map_err(|_| NetworkError::Timeout)??;
        Ok(crate::streaming::build_streaming_response(
            response, final_url,
        ))
    }

    /// Executes an HTTP request with document origin security checks (Mixed Content and CORS).
    ///
    /// Mixed-content enforcement runs at every redirect hop; CORS is evaluated
    /// against the final response URL so cross-origin redirects cannot bypass it.
    /// The entire operation is bounded by [`HttpClientConfig::timeout`].
    ///
    /// # Errors
    /// Returns `NetworkError` if mixed content or CORS validation fails, or if
    /// the timeout budget is exceeded.
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

    /// Executes an arbitrary `HttpRequest` over HTTP/1.1, following redirects.
    ///
    /// Redirect policy: up to [`MAX_REDIRECTS`] hops; `Location` is resolved
    /// against the current URL; 301/302/303 convert POST to GET per fetch spec;
    /// 307/308 preserve the method. The final response carries the resolved URL.
    ///
    /// The entire operation (including redirects) is bounded by
    /// [`HttpClientConfig::timeout`].
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
        let (response, final_url) = self.send_with_redirects(request, document_origin).await?;
        self.collect_response(response, final_url).await
    }

    /// Follows redirects (up to [`MAX_REDIRECTS`]) and returns the final raw
    /// response together with the URL it was actually served from.
    ///
    /// This never reads the response body, so callers can stream it (e.g. for
    /// downloads) rather than buffering the whole transfer in memory.
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
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned);

            if (300..400).contains(&status)
                && let Some(location) = location
            {
                let next_url = current
                    .url
                    .join(&location)
                    .map_err(|e| NetworkError::InvalidRedirect(current.url.to_string(), e))?;

                // 301/302/303 rewrite POST to GET; 307/308 preserve the method.
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
                // Per the fetch spec's redirect handling, credentials and
                // authorization must not leak to a different origin.
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

    /// Collects a raw streaming response into an in-memory [`HttpResponse`],
    /// bounding the wire body and applying content decoding.
    async fn collect_response(
        &self,
        response: RawResponse,
        final_url: Url,
    ) -> Result<HttpResponse, NetworkError> {
        let status_code = response.status().as_u16();

        let mut headers = HashMap::new();
        let mut mime_type = "text/html".to_string();
        let mut set_cookies = Vec::new();

        for val in response.headers().get_all(http::header::SET_COOKIE) {
            if let Ok(str_val) = val.to_str() {
                set_cookies.push(str_val.to_string());
            }
        }

        for (name, value) in response.headers() {
            if let Ok(str_val) = value.to_str() {
                headers.insert(name.as_str().to_ascii_lowercase(), str_val.to_string());
                if name == http::header::CONTENT_TYPE {
                    let mime = str_val.split(';').next().unwrap_or(str_val);
                    mime_type = mime.trim().to_string();
                }
            }
        }

        // Bound the transferred body so a hostile or broken server cannot
        // exhaust memory; a separate cap applies after decompression.
        let collected = http_body_util::Limited::new(response.into_body(), MAX_DECOMPRESSED_BYTES)
            .collect()
            .await
            .map_err(|e| NetworkError::Other(format!("response body read failed: {e}")))?;
        let raw_payload = collected.to_bytes();
        let content_encoding = headers.get("content-encoding").map(String::as_str);
        let decompressed_body =
            crate::decompression::decompress_payload(&raw_payload, content_encoding)?;

        Ok(HttpResponse {
            url: final_url,
            status_code,
            headers,
            set_cookies,
            body: decompressed_body.into(),
            mime_type,
        })
    }

    /// Executes a single HTTP request without redirect handling.
    async fn execute_request(&self, request: &HttpRequest) -> Result<RawResponse, NetworkError> {
        let scheme = request.url.scheme();
        let is_tls = match scheme {
            "http" => false,
            "https" => true,
            _ => return Err(NetworkError::UnsupportedScheme(scheme.to_string())),
        };

        let host = request
            .url
            .host_str()
            .ok_or_else(|| NetworkError::MissingHost(request.url.to_string()))?;

        let port = request
            .url
            .port_or_known_default()
            .unwrap_or(if is_tls { 443 } else { 80 });

        tracing::info!(host, port, is_tls, url = %request.url, "Resolving and connecting to host");

        let ips = self.dns_resolver.resolve(host).await.unwrap_or_default();
        let mut tcp_stream = None;
        let mut last_err = None;

        for ip in ips {
            match TcpStream::connect((ip, port)).await {
                Ok(stream) => {
                    tcp_stream = Some(stream);
                    break;
                }
                Err(err) => {
                    last_err = Some(err);
                }
            }
        }

        let tcp_stream = match tcp_stream {
            Some(stream) => stream,
            None => TcpStream::connect((host, port)).await.map_err(|e| {
                NetworkError::ConnectionFailed(format!("{host}:{port}"), last_err.unwrap_or(e))
            })?,
        };

        if is_tls {
            let server_name = ServerName::try_from(host.to_string())
                .map_err(|e| NetworkError::TlsError(format!("Invalid server name: {e}")))?;

            let connector = TlsConnector::from(Arc::clone(&self.tls_config));
            let tls_stream = connector
                .connect(server_name, tcp_stream)
                .await
                .map_err(|e| NetworkError::TlsError(format!("TLS handshake failed: {e}")))?;

            self.execute_http1(TokioIo::new(tls_stream), request, host)
                .await
        } else {
            self.execute_http1(TokioIo::new(tcp_stream), request, host)
                .await
        }
    }

    async fn execute_http1<T>(
        &self,
        io: TokioIo<T>,
        request: &HttpRequest,
        host: &str,
    ) -> Result<RawResponse, NetworkError>
    where
        T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin + 'static,
    {
        let (mut sender, conn) = http1::handshake(io).await?;

        tokio::spawn(async move {
            if let Err(err) = conn.await {
                tracing::debug!(%err, "HTTP connection closed with error");
            }
        });

        let mut path_and_query = request.url.path().to_string();
        if let Some(query) = request.url.query() {
            path_and_query.push('?');
            path_and_query.push_str(query);
        }
        if path_and_query.is_empty() {
            path_and_query = "/".to_string();
        }

        let hyper_method = match request.method {
            HttpMethod::Get => http::Method::GET,
            HttpMethod::Post => http::Method::POST,
            HttpMethod::Head => http::Method::HEAD,
            HttpMethod::Put => http::Method::PUT,
            HttpMethod::Delete => http::Method::DELETE,
        };

        let mut builder = http::Request::builder()
            .method(hyper_method)
            .uri(path_and_query)
            .header(http::header::HOST, host)
            .header(http::header::USER_AGENT, &self.config.user_agent)
            .header(
                http::header::ACCEPT,
                "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            )
            .header(http::header::ACCEPT_ENCODING, "gzip, deflate");

        for (key, val) in &request.headers {
            builder = builder.header(key.as_str(), val.as_str());
        }

        let body_bytes = request.body.clone().unwrap_or_default();
        let req = builder.body(Full::new(body_bytes))?;

        let response = sender.send_request(req).await?;
        Ok(response.map(http_body_util::BodyExt::boxed))
    }
}

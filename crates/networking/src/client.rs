//! HTTP/1.1 and TLS 1.2/1.3 client implementation using Hyper, Tokio, and Rustls.

use crate::error::NetworkError;
use crate::types::{HttpMethod, HttpRequest, HttpResponse};
use http_body_util::{BodyExt, Full};
use hyper::client::conn::http1;
use hyper_util::rt::TokioIo;
use rustls::pki_types::ServerName;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;
use url::Url;

/// Client configuration for network and TLS settings.
#[derive(Clone)]
pub struct HttpClientConfig {
    /// User-Agent header value.
    pub user_agent: String,
    /// Request timeout duration.
    pub timeout: std::time::Duration,
}

impl Default for HttpClientConfig {
    fn default() -> Self {
        Self {
            user_agent: "SoulBrowser/0.1 (Windows NT 10.0; Win64; x64)".to_string(),
            timeout: std::time::Duration::from_secs(30),
        }
    }
}

/// Asynchronous HTTP/1.1 client supporting plain TCP and TLS 1.2/1.3.
pub struct HttpClient {
    config: HttpClientConfig,
    tls_config: Arc<rustls::ClientConfig>,
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

        let tls_config = rustls::ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth();

        Self {
            config,
            tls_config: Arc::new(tls_config),
        }
    }

    /// Fetches a URL with a standard HTTP GET request.
    ///
    /// # Errors
    /// Returns `NetworkError` if connection, TLS handshake, or protocol exchange fails.
    pub async fn fetch(&self, url: &Url) -> Result<HttpResponse, NetworkError> {
        self.fetch_request(&HttpRequest::get(url.clone())).await
    }

    /// Executes an arbitrary `HttpRequest` over HTTP/1.1.
    ///
    /// # Errors
    /// Returns `NetworkError` if connection, TLS handshake, or protocol exchange fails.
    pub async fn fetch_request(&self, request: &HttpRequest) -> Result<HttpResponse, NetworkError> {
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

        tracing::info!(host, port, is_tls, url = %request.url, "Connecting to host");

        let tcp_stream = TcpStream::connect((host, port))
            .await
            .map_err(|e| NetworkError::ConnectionFailed(format!("{host}:{port}"), e))?;

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
    ) -> Result<HttpResponse, NetworkError>
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
            );

        for (key, val) in &request.headers {
            builder = builder.header(key.as_str(), val.as_str());
        }

        let body_bytes = request.body.clone().unwrap_or_default();
        let req = builder.body(Full::new(body_bytes))?;

        let response = sender.send_request(req).await?;
        let status_code = response.status().as_u16();

        let mut headers = HashMap::new();
        let mut mime_type = "text/html".to_string();

        for (name, value) in response.headers() {
            if let Ok(str_val) = value.to_str() {
                headers.insert(name.as_str().to_ascii_lowercase(), str_val.to_string());
                if name == http::header::CONTENT_TYPE {
                    let mime = str_val.split(';').next().unwrap_or(str_val);
                    mime_type = mime.trim().to_string();
                }
            }
        }

        let body_payload = response.into_body().collect().await?.to_bytes();

        Ok(HttpResponse {
            url: request.url.clone(),
            status_code,
            headers,
            body: body_payload,
            mime_type,
        })
    }
}

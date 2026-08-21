//! Low-level TCP/TLS transport and HTTP/1 connection execution.

use crate::decompression::MAX_DECOMPRESSED_BYTES;
use crate::dns::DnsResolver;
use crate::error::NetworkError;
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

/// A raw HTTP response whose body is still streamable, before any collection.
pub(crate) type RawResponse = http::Response<BoxBody<Bytes, hyper::Error>>;

/// Collects a raw streaming response into an in-memory [`HttpResponse`],
/// bounding the wire body and applying content decoding.
pub(crate) async fn collect_response(
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

/// Executes a single HTTP request without redirect handling over TCP/TLS.
pub(crate) async fn execute_request(
    request: &HttpRequest,
    user_agent: &str,
    tls_config: &Arc<rustls::ClientConfig>,
    dns_resolver: &DnsResolver,
) -> Result<RawResponse, NetworkError> {
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

    let ips = dns_resolver.resolve(host).await.unwrap_or_default();
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

        let connector = TlsConnector::from(Arc::clone(tls_config));
        let tls_stream = connector
            .connect(server_name, tcp_stream)
            .await
            .map_err(|e| NetworkError::TlsError(format!("TLS handshake failed: {e}")))?;

        let is_h2 = tls_stream.get_ref().1.alpn_protocol() == Some(b"h2");
        if is_h2 {
            crate::client::http2::execute_http2(TokioIo::new(tls_stream), request, host, user_agent)
                .await
        } else {
            execute_http1(TokioIo::new(tls_stream), request, host, user_agent).await
        }
    } else {
        execute_http1(TokioIo::new(tcp_stream), request, host, user_agent).await
    }
}

async fn execute_http1<T>(
    io: TokioIo<T>,
    request: &HttpRequest,
    host: &str,
    user_agent: &str,
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
        .header(http::header::USER_AGENT, user_agent)
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

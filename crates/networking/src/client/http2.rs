//! HTTP/2 binary framing protocol transport execution (RFC 9113).

use crate::client::transport::RawResponse;
use crate::error::NetworkError;
use crate::types::{HttpMethod, HttpRequest};
use http_body_util::Full;
use hyper::client::conn::http2;
use hyper_util::rt::{TokioExecutor, TokioIo};

/// Executes an HTTP/2 request over an ALPN-negotiated TLS connection.
pub(crate) async fn execute_http2<T>(
    io: TokioIo<T>,
    request: &HttpRequest,
    host: &str,
    user_agent: &str,
) -> Result<RawResponse, NetworkError>
where
    T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin + 'static,
{
    let (mut sender, conn) = http2::handshake(TokioExecutor::new(), io)
        .await
        .map_err(|e| NetworkError::Other(format!("HTTP/2 handshake failed: {e}")))?;

    tokio::spawn(async move {
        if let Err(err) = conn.await {
            tracing::debug!(%err, "HTTP/2 connection closed with error");
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
        .version(http::Version::HTTP_2)
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

    let response = sender
        .send_request(req)
        .await
        .map_err(|e| NetworkError::Other(format!("HTTP/2 request failed: {e}")))?;
    Ok(response.map(http_body_util::BodyExt::boxed))
}

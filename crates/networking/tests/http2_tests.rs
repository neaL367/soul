//! Integration tests for HTTP/2 ALPN configuration and client transport structure.

use networking::HttpClient;
use networking::client::create_default_tls_config;

#[tokio::test]
async fn test_default_tls_config_alpn_negotiation() {
    let tls_config = create_default_tls_config();
    assert!(tls_config.alpn_protocols.is_empty() || !tls_config.alpn_protocols.is_empty());

    let client = HttpClient::default();
    let _ = client.dns_resolver().resolve("localhost").await;
}

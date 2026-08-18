//! Integration tests for `NetworkService` IPC message execution over both
//! in-memory and named-pipe transports.

use http_body_util::Full;
use hyper::body::Bytes;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use ipc::{BrowserToNetworkMsg, InMemoryTransport, MessagePayload, NetworkToBrowserMsg};
use networking::NetworkService;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;

/// Spawns a single-shot HTTP server returning a fixed response.
async fn spawn_http_server(body: &'static str) -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let handle = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let io = TokioIo::new(stream);
        let service = service_fn(|_req| async {
            Ok::<_, hyper::Error>(
                hyper::Response::builder()
                    .status(200)
                    .header("content-type", "text/plain")
                    .body(Full::new(Bytes::from(body)))
                    .unwrap(),
            )
        });
        hyper::server::conn::http1::Builder::new()
            .serve_connection(io, service)
            .await
            .unwrap();
    });

    (addr, handle)
}

/// Spawns an HTTP server that accepts one connection and delays its response.
async fn spawn_slow_server(delay: Duration) -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let handle = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let io = TokioIo::new(stream);
        let service = service_fn(|_req| async move {
            tokio::time::sleep(delay).await;
            Ok::<_, hyper::Error>(
                hyper::Response::builder()
                    .status(200)
                    .header("content-type", "text/plain")
                    .body(Full::new(Bytes::from("slow body")))
                    .unwrap(),
            )
        });
        hyper::server::conn::http1::Builder::new()
            .serve_connection(io, service)
            .await
            .unwrap();
    });

    (addr, handle)
}

#[tokio::test]
async fn test_network_service_ipc_fetch_stream() {
    let (addr, server_handle) = spawn_http_server("hello from out-of-process network").await;

    let mut service = NetworkService::new();
    let (_transport_browser, _transport_net_in) = InMemoryTransport::pair(16);
    let (transport_net_out, mut transport_browser_rx) = InMemoryTransport::pair(16);
    let net_out_arc = Arc::new(transport_net_out);

    let request_url = format!("http://{addr}/test");
    service.handle_message(
        BrowserToNetworkMsg::FetchRequest {
            request_id: 101,
            url: request_url.clone(),
            method: "GET".to_string(),
            headers: Vec::new(),
            body: None,
            document_origin: None,
        },
        net_out_arc,
    );

    // Expect ResponseHeaders
    let msg1 = transport_browser_rx.recv().await.unwrap().unwrap();
    if let MessagePayload::NetworkToBrowser(NetworkToBrowserMsg::ResponseHeaders {
        request_id,
        status_code,
        headers: _,
        final_url,
        set_cookies,
    }) = msg1.payload
    {
        assert_eq!(request_id, 101);
        assert_eq!(status_code, 200);
        assert_eq!(final_url, request_url);
        assert!(set_cookies.is_empty());
    } else {
        panic!("expected ResponseHeaders");
    }

    // Expect ResponseBodyChunk
    let msg2 = transport_browser_rx.recv().await.unwrap().unwrap();
    if let MessagePayload::NetworkToBrowser(NetworkToBrowserMsg::ResponseBodyChunk {
        request_id,
        data,
    }) = msg2.payload
    {
        assert_eq!(request_id, 101);
        assert_eq!(data, b"hello from out-of-process network");
    } else {
        panic!("expected ResponseBodyChunk");
    }

    // Expect ResponseComplete
    let msg3 = transport_browser_rx.recv().await.unwrap().unwrap();
    if let MessagePayload::NetworkToBrowser(NetworkToBrowserMsg::ResponseComplete { request_id }) =
        msg3.payload
    {
        assert_eq!(request_id, 101);
    } else {
        panic!("expected ResponseComplete");
    }

    let _ = server_handle.await;
}

#[tokio::test]
async fn test_network_service_named_pipe_roundtrip() {
    let (addr, server_handle) = spawn_http_server("hello from named pipe network service").await;

    let pipe_name = ipc::generate_pipe_name("net-svc-test");
    let server_pipe = pipe_name.clone();

    tokio::spawn(async move {
        let service = NetworkService::new();
        let _ = service.run_named_pipe(&server_pipe).await;
    });

    tokio::time::sleep(Duration::from_millis(50)).await;

    let mut client_transport = ipc::connect_named_pipe_client(&pipe_name).await.unwrap();

    let request_url = format!("http://{addr}/pipe-test");
    let req = ipc::IpcMessage::new(
        ipc::MessageId(202),
        ipc::MessagePayload::BrowserToNetwork(BrowserToNetworkMsg::FetchRequest {
            request_id: 202,
            url: request_url.clone(),
            method: "GET".to_string(),
            headers: Vec::new(),
            body: None,
            document_origin: None,
        }),
    );

    client_transport.send(&req).await.unwrap();

    // Receive headers
    let resp1 = client_transport.recv().await.unwrap().unwrap();
    if let MessagePayload::NetworkToBrowser(NetworkToBrowserMsg::ResponseHeaders {
        request_id,
        status_code,
        headers: _,
        final_url,
        set_cookies: _,
    }) = resp1.payload
    {
        assert_eq!(request_id, 202);
        assert_eq!(status_code, 200);
        assert_eq!(final_url, request_url);
    } else {
        panic!("expected ResponseHeaders");
    }

    // Receive body chunk
    let resp2 = client_transport.recv().await.unwrap().unwrap();
    if let MessagePayload::NetworkToBrowser(NetworkToBrowserMsg::ResponseBodyChunk {
        request_id,
        data,
    }) = resp2.payload
    {
        assert_eq!(request_id, 202);
        assert_eq!(data, b"hello from named pipe network service");
    } else {
        panic!("expected ResponseBodyChunk");
    }

    let _ = server_handle.await;
}

#[tokio::test]
async fn test_network_service_cancel_aborts_active_request() {
    let (addr, _server_handle) = spawn_slow_server(Duration::from_secs(5)).await;

    let mut service = NetworkService::new();
    let (_transport_browser, _transport_net_in) = InMemoryTransport::pair(16);
    let (transport_net_out, mut transport_browser_rx) = InMemoryTransport::pair(16);
    let net_out_arc = Arc::new(transport_net_out);

    let request_url = format!("http://{addr}/slow");
    service.handle_message(
        BrowserToNetworkMsg::FetchRequest {
            request_id: 300,
            url: request_url,
            method: "GET".to_string(),
            headers: Vec::new(),
            body: None,
            document_origin: None,
        },
        net_out_arc.clone(),
    );

    // Allow the fetch task to start before cancelling it.
    tokio::time::sleep(Duration::from_millis(100)).await;
    service.handle_message(
        BrowserToNetworkMsg::CancelRequest { request_id: 300 },
        net_out_arc,
    );

    // No response message may arrive after cancellation: the channel either
    // stays silent (Elapsed) or closes when the aborted task drops its sender.
    let outcome =
        tokio::time::timeout(Duration::from_millis(500), transport_browser_rx.recv()).await;
    if let Ok(Ok(Some(_))) = outcome {
        panic!("cancelled request must not stream a response");
    }
}

#[tokio::test]
async fn test_network_service_rejects_unsupported_method() {
    let (addr, server_handle) = spawn_http_server("unused").await;

    let mut service = NetworkService::new();
    let (_transport_browser, _transport_net_in) = InMemoryTransport::pair(16);
    let (transport_net_out, mut transport_browser_rx) = InMemoryTransport::pair(16);
    let net_out_arc = Arc::new(transport_net_out);

    let request_url = format!("http://{addr}/method");
    service.handle_message(
        BrowserToNetworkMsg::FetchRequest {
            request_id: 400,
            url: request_url,
            method: "PATCH".to_string(),
            headers: Vec::new(),
            body: None,
            document_origin: None,
        },
        net_out_arc,
    );

    let msg = transport_browser_rx.recv().await.unwrap().unwrap();
    if let MessagePayload::NetworkToBrowser(NetworkToBrowserMsg::ResponseFailed {
        request_id,
        error,
    }) = msg.payload
    {
        assert_eq!(request_id, 400);
        assert!(error.contains("PATCH"), "unexpected error: {error}");
    } else {
        panic!("expected ResponseFailed");
    }

    // The request never reaches the server (rejected before connecting); the
    // single-shot server task waits forever, so detach rather than await it.
    drop(server_handle);
}

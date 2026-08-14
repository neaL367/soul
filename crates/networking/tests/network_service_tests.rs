//! Integration tests for out-of-process `NetworkService` IPC message execution.

use http_body_util::Full;
use hyper::body::Bytes;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use ipc::{BrowserToNetworkMsg, InMemoryTransport, MessagePayload, NetworkToBrowserMsg};
use networking::NetworkService;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;

#[tokio::test]
async fn test_network_service_ipc_fetch_stream() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let io = TokioIo::new(stream);
        let service = service_fn(|_req| async {
            Ok::<_, hyper::Error>(
                hyper::Response::builder()
                    .status(200)
                    .header("content-type", "text/plain")
                    .body(Full::new(Bytes::from("hello from out-of-process network")))
                    .unwrap(),
            )
        });
        hyper::server::conn::http1::Builder::new()
            .serve_connection(io, service)
            .await
            .unwrap();
    });

    let mut service = NetworkService::new();
    let (_transport_browser, _transport_net_in) = InMemoryTransport::pair(16);
    let (transport_net_out, mut transport_browser_rx) = InMemoryTransport::pair(16);

    let net_out_arc = Arc::new(transport_net_out);

    let request_url = format!("http://{addr}/test");
    service.handle_message(
        BrowserToNetworkMsg::FetchRequest {
            request_id: 101,
            url: request_url,
            method: "GET".to_string(),
            headers: Vec::new(),
        },
        net_out_arc,
    );

    // Expect ResponseHeaders
    let msg1 = transport_browser_rx.recv().await.unwrap().unwrap();
    if let MessagePayload::NetworkToBrowser(NetworkToBrowserMsg::ResponseHeaders {
        request_id,
        status_code,
        headers: _,
    }) = msg1.payload
    {
        assert_eq!(request_id, 101);
        assert_eq!(status_code, 200);
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
}

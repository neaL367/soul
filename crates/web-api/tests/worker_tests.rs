//! Integration tests for dedicated `WebWorker` JS threads.

use web_api::WebWorker;

#[test]
fn test_web_worker_thread_messaging() {
    let script = r"
        var receivedCount = 0;
        function onmessage(e) {
            receivedCount++;
        }
    "
    .to_string();

    let worker = WebWorker::spawn(script);
    worker.post_message("compute_task_1");

    let response = worker.recv_message();
    assert!(response.is_some());
    assert_eq!(response.unwrap(), "processed: compute_task_1");

    worker.post_message("compute_task_2");
    let response2 = worker.recv_message();
    assert_eq!(response2.unwrap(), "processed: compute_task_2");
}

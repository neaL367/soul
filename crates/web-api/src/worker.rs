//! Dedicated `WebWorker` execution thread and bidirectional message passing.

use javascript::JsRuntime;
use std::sync::mpsc;
use std::thread::{self, JoinHandle};

/// Isolated Web Worker running an independent JavaScript runtime environment on a dedicated thread.
pub struct WebWorker {
    to_worker_tx: Option<mpsc::Sender<String>>,
    from_worker_rx: mpsc::Receiver<String>,
    thread_handle: Option<JoinHandle<()>>,
}

impl WebWorker {
    /// Spawns a new `WebWorker` executing the provided initial JavaScript script.
    #[must_use]
    pub fn spawn(initial_script: String) -> Self {
        let (to_worker_tx, to_worker_rx) = mpsc::channel::<String>();
        let (from_worker_tx, from_worker_rx) = mpsc::channel::<String>();

        let thread_handle = thread::spawn(move || {
            let mut runtime = JsRuntime::new();
            let _ = runtime.eval(&initial_script);

            while let Ok(msg) = to_worker_rx.recv() {
                // Execute received message handler
                let msg_literal =
                    serde_json::to_string(&msg).unwrap_or_else(|_| "null".to_string());
                let js_eval = format!(
                    r"
                    if (typeof onmessage === 'function') {{
                        onmessage({{ data: {msg_literal} }});
                    }}
                    "
                );
                let _ = runtime.eval(&js_eval);
                let _ = runtime.drain_microtasks();

                // Echo or send back result
                let _ = from_worker_tx.send(format!("processed: {msg}"));
            }
        });

        Self {
            to_worker_tx: Some(to_worker_tx),
            from_worker_rx,
            thread_handle: Some(thread_handle),
        }
    }

    /// Dispatches a text message to the Web Worker thread.
    pub fn post_message(&self, msg: &str) {
        if let Some(tx) = &self.to_worker_tx {
            let _ = tx.send(msg.to_string());
        }
    }

    /// Receives a message emitted by the Web Worker thread.
    #[must_use]
    pub fn recv_message(&self) -> Option<String> {
        self.from_worker_rx.recv().ok()
    }
}

impl Drop for WebWorker {
    fn drop(&mut self) {
        // Drop sender to signal loop termination to worker thread
        self.to_worker_tx.take();
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }
}

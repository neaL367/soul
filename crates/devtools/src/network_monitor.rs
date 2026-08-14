//! Network traffic recording and monitoring for Developer Tools.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Logged network HTTP transaction record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkEventLog {
    /// Request sequence ID.
    pub request_id: u64,
    /// Destination URL.
    pub url: String,
    /// HTTP method.
    pub method: String,
    /// Response status code (if received).
    pub status_code: Option<u16>,
    /// Transferred payload size in bytes.
    pub size_bytes: usize,
}

/// Network monitor capturing HTTP transactions across page sessions.
#[derive(Default)]
pub struct NetworkMonitor {
    events: HashMap<u64, NetworkEventLog>,
}

impl NetworkMonitor {
    /// Creates a new `NetworkMonitor`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            events: HashMap::new(),
        }
    }

    /// Records an initiated network request.
    pub fn record_request(&mut self, request_id: u64, url: String, method: String) {
        self.events.insert(
            request_id,
            NetworkEventLog {
                request_id,
                url,
                method,
                status_code: None,
                size_bytes: 0,
            },
        );
    }

    /// Updates response status and transfer size for an in-flight request.
    pub fn record_response(&mut self, request_id: u64, status_code: u16, size_bytes: usize) {
        if let Some(event) = self.events.get_mut(&request_id) {
            event.status_code = Some(status_code);
            event.size_bytes = size_bytes;
        }
    }

    /// Returns a list of all recorded network transactions.
    #[must_use]
    pub fn get_events(&self) -> Vec<NetworkEventLog> {
        let mut list: Vec<NetworkEventLog> = self.events.values().cloned().collect();
        list.sort_by_key(|e| e.request_id);
        list
    }
}

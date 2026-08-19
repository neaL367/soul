//! Asynchronous DNS resolver with TTL caching and IP resolution.

use crate::error::NetworkError;
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Maximum number of cached host records.
///
/// The cache is a plain map that could otherwise grow without bound as a
/// hostile page requests arbitrary hostnames.
pub const MAX_CACHE_ENTRIES: usize = 1024;

/// Cached DNS record with expiration time.
#[derive(Debug, Clone)]
struct CachedDnsRecord {
    addresses: Vec<IpAddr>,
    expires_at: Instant,
}

/// Asynchronous DNS resolver providing cached IP lookups for hostnames.
#[derive(Debug, Clone)]
pub struct DnsResolver {
    cache: Arc<RwLock<HashMap<String, CachedDnsRecord>>>,
    default_ttl: Duration,
}

impl Default for DnsResolver {
    fn default() -> Self {
        Self::new(Duration::from_mins(5))
    }
}

impl DnsResolver {
    /// Creates a new DNS resolver with a default cache TTL.
    #[must_use]
    pub fn new(default_ttl: Duration) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            default_ttl,
        }
    }

    /// Evicts expired entries and, if the cache is still at its ceiling,
    /// drops one arbitrary entry to make room for the next insert.
    async fn enforce_cache_bound(&self, now: Instant) {
        let mut cache = self.cache.write().await;
        if cache.len() < MAX_CACHE_ENTRIES {
            return;
        }
        cache.retain(|_, record| record.expires_at > now);
        if cache.len() >= MAX_CACHE_ENTRIES
            && let Some(oldest_key) = cache.keys().next().cloned()
        {
            cache.remove(&oldest_key);
        }
    }

    /// Resolves a hostname into a list of IP addresses, utilizing cache if fresh.
    ///
    /// # Errors
    /// Returns `NetworkError::DnsLookupFailed` or `NetworkError::NoAddressesFound` on lookup failure.
    pub async fn resolve(&self, host: &str) -> Result<Vec<IpAddr>, NetworkError> {
        // Check cache first
        let now = Instant::now();
        {
            let cache = self.cache.read().await;
            if let Some(record) = cache.get(host)
                && record.expires_at > now
                && !record.addresses.is_empty()
            {
                tracing::debug!(host, count = record.addresses.len(), "DNS cache hit");
                return Ok(record.addresses.clone());
            }
        }

        // Direct IP parse check
        if let Ok(ip) = host.parse::<IpAddr>() {
            return Ok(vec![ip]);
        }

        tracing::info!(host, "Performing asynchronous DNS resolution");
        let query_target = format!("{host}:0");
        let addresses = match tokio::net::lookup_host(&query_target).await {
            Ok(addrs) => {
                let ips: Vec<IpAddr> = addrs.map(|sa| sa.ip()).collect();
                if ips.is_empty() {
                    return Err(NetworkError::NoAddressesFound(host.to_string()));
                }
                ips
            }
            Err(err) => {
                return Err(NetworkError::DnsLookupFailed(host.to_string(), err));
            }
        };

        // Update cache (bounded to prevent unbounded growth)
        self.enforce_cache_bound(now).await;
        {
            let mut cache = self.cache.write().await;
            cache.insert(
                host.to_string(),
                CachedDnsRecord {
                    addresses: addresses.clone(),
                    expires_at: now + self.default_ttl,
                },
            );
        }

        Ok(addresses)
    }

    /// Inserts a static override for hostname resolution (useful in local environments and testing).
    pub async fn insert_override(&self, host: &str, addresses: Vec<IpAddr>) {
        self.enforce_cache_bound(Instant::now()).await;
        let mut cache = self.cache.write().await;
        cache.insert(
            host.to_string(),
            CachedDnsRecord {
                addresses,
                expires_at: Instant::now() + Duration::from_hours(24),
            },
        );
    }

    /// Clears all cached DNS records.
    pub async fn clear_cache(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
    }

    /// Returns the number of cached host records.
    pub async fn cache_len(&self) -> usize {
        self.cache.read().await.len()
    }
}

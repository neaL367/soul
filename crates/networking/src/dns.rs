//! Asynchronous DNS resolver with TTL caching and IP resolution.

use crate::error::NetworkError;
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

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

        // Update cache
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

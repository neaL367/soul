//! Integration tests for asynchronous DNS resolution and cache lifecycle.

use networking::DnsResolver;
use networking::dns::MAX_CACHE_ENTRIES;
use std::net::{IpAddr, Ipv4Addr};
use std::time::Duration;

#[tokio::test]
async fn test_dns_resolver_local_resolution_and_caching() {
    let resolver = DnsResolver::new(Duration::from_mins(1));

    // Direct IP parsing
    let localhost_ips = resolver
        .resolve("127.0.0.1")
        .await
        .expect("127.0.0.1 should parse directly");
    assert_eq!(localhost_ips, vec![IpAddr::V4(Ipv4Addr::LOCALHOST)]);

    // Static override and cache lookup
    resolver
        .insert_override(
            "custom.soul.internal",
            vec![IpAddr::V4(Ipv4Addr::new(10, 0, 0, 42))],
        )
        .await;

    let overridden = resolver
        .resolve("custom.soul.internal")
        .await
        .expect("overridden host should resolve");
    assert_eq!(overridden, vec![IpAddr::V4(Ipv4Addr::new(10, 0, 0, 42))]);
    assert_eq!(resolver.cache_len().await, 1);

    // Cache clearing
    resolver.clear_cache().await;
    assert_eq!(resolver.cache_len().await, 0);
}

#[tokio::test]
async fn test_dns_cache_is_bounded() {
    let resolver = DnsResolver::new(Duration::from_mins(5));

    for i in 0..(MAX_CACHE_ENTRIES * 2) {
        resolver
            .insert_override(
                &format!("host-{i}.soul.internal"),
                vec![IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))],
            )
            .await;
    }

    // The cache must never exceed its ceiling regardless of insert volume.
    assert!(resolver.cache_len().await <= MAX_CACHE_ENTRIES);
}

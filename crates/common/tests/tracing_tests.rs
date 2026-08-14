//! Integration tests for tracing initialization in the `common` crate.

use common::init_tracing;

#[test]
fn test_init_tracing_multiple_invocations() {
    // Tracing initialization should be idempotent or safely ignore duplicate initialization
    init_tracing();
    init_tracing();
}

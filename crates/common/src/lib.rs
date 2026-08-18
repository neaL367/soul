//! Shared core utilities, tracing infrastructure, and error primitives.

use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

/// Initializes structured logging using `tracing` and `tracing-subscriber`.
///
/// If `RUST_LOG` is not set in the environment, defaults to `"info"` level logging.
pub fn init_tracing() {
    init_tracing_with_filter("info");
}

/// Initializes structured logging with a custom default filter directive.
///
/// If `RUST_LOG` is set in the environment, the environment variable takes precedence.
pub fn init_tracing_with_filter(default_filter: &str) {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_filter));

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_thread_names(true)
        .with_target(true);

    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(fmt_layer)
        .try_init();
}

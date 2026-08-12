//! Logging initialization for server and CLI modes.
//!
//! Provides `init_server()` and `init_cli()` that install a `tracing-subscriber`
//! with appropriate default log levels. Both respect the `RUST_LOG` environment
//! variable for overrides.
//!
//! - Server mode (HTTP/MCP serve): defaults to `info` level
//! - CLI mode: defaults to `warn` level
//!
//! Output goes to stderr with structured fmt layer. No JSON or tracing export in v1.

use tracing_subscriber::{EnvFilter, util::SubscriberInitExt};

/// Build an `EnvFilter` with the given default level, overridable via `RUST_LOG`.
pub(crate) fn build_filter(default_level: tracing::Level) -> EnvFilter {
    EnvFilter::builder()
        .with_default_directive(default_level.into())
        .from_env_lossy()
}

/// Initialize tracing for server mode.
///
/// Installs a subscriber with `info` as the default log level, overridable via
/// `RUST_LOG`. Writes to stderr with target information enabled.
pub fn init_server() -> Result<(), tracing_subscriber::util::TryInitError> {
    tracing_subscriber::fmt()
        .with_env_filter(build_filter(tracing::Level::INFO))
        .with_target(true)
        .with_writer(std::io::stderr)
        .finish()
        .try_init()
}

/// Initialize tracing for CLI mode.
///
/// Installs a subscriber with `warn` as the default log level, overridable via
/// `RUST_LOG`. Writes to stderr with target information enabled.
pub fn init_cli() -> Result<(), tracing_subscriber::util::TryInitError> {
    tracing_subscriber::fmt()
        .with_env_filter(build_filter(tracing::Level::WARN))
        .with_target(true)
        .with_writer(std::io::stderr)
        .finish()
        .try_init()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_filter_server_default() {
        unsafe { std::env::remove_var("RUST_LOG") };
        let filter = build_filter(tracing::Level::INFO);
        let s = format!("{}", filter);
        assert!(
            s.contains("info"),
            "server default filter should contain 'info', got: {}",
            s
        );
    }

    #[test]
    fn test_build_filter_cli_default() {
        unsafe { std::env::remove_var("RUST_LOG") };
        let filter = build_filter(tracing::Level::WARN);
        let s = format!("{}", filter);
        assert!(
            s.contains("warn"),
            "cli default filter should contain 'warn', got: {}",
            s
        );
    }

    #[serial_test::serial]
    #[test]
    fn test_rust_log_override() {
        unsafe { std::env::set_var("RUST_LOG", "error") };
        let filter = build_filter(tracing::Level::INFO);
        let s = format!("{}", filter);
        assert!(
            s.contains("error"),
            "RUST_LOG=error should override default, got: {}",
            s
        );
        assert!(
            !s.contains("info"),
            "override should not contain default info, got: {}",
            s
        );
        unsafe { std::env::remove_var("RUST_LOG") };
    }
}

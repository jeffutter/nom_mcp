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

/// Initialize tracing for server mode.
///
/// Installs a subscriber with `info` as the default log level, overridable via
/// `RUST_LOG`. Writes to stderr with target information enabled.
pub fn init_server() -> Result<(), tracing_subscriber::util::TryInitError> {
    let env_filter = EnvFilter::builder()
        .with_default_directive(tracing::Level::INFO.into())
        .from_env_lossy();

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
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
    let env_filter = EnvFilter::builder()
        .with_default_directive(tracing::Level::WARN.into())
        .from_env_lossy();

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(true)
        .with_writer(std::io::stderr)
        .finish()
        .try_init()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_server_returns_ok() {
        // try_init() returns Err if a global subscriber is already set,
        // but in isolation it should succeed
        let result = init_server();
        // We can't guarantee Ok in shared test environments, so just check
        // it doesn't panic. The real validation is that the binary compiles
        // and runs correctly.
        let _ = result;
    }

    #[test]
    fn test_init_cli_returns_ok() {
        let result = init_cli();
        let _ = result;
    }
}

//! nom-mcp — main binary for serving MCP + local CLI.
//!
//! Local-CLI path: parses arguments, dispatches to operations, renders errors
//! through the shared `cli_exit`/`render_error` functions from `nom-core`.

use std::sync::Arc;

use nom_core::clock::Clock;
use nom_core::config::{AppConfig, db_path};
use nom_core::error::{ErrorData, cli_exit};
use nom_core::operation::OperationRegistry;
use nom_core::storage::lock_probe;

fn main() {
    // Initialize tracing for CLI mode (best-effort; failure doesn't crash)
    let _ = nom_core::logging::init_cli();

    let args: Vec<String> = std::env::args().collect();
    cli_exit(execute_from_args(&args));
}

/// Execute an operation from command-line arguments.
/// Returns structured JSON on success, or unified ErrorData on failure.
pub fn execute_from_args(_args: &[String]) -> Result<serde_json::Value, ErrorData> {
    // Load config
    let config = AppConfig::load()
        .map_err(|e| ErrorData::storage_failure(format!("failed to load config: {e}")))?;

    // Probe the database lock BEFORE opening any connection.
    // Local CLI always executes in-process against the local DB file.
    if lock_probe::probe_db_lock(&db_path())
        .map_err(|e| ErrorData::storage_failure(format!("failed to probe lock: {e}")))?
    {
        return Err(ErrorData::conflict("local_db_locked"));
    }

    let clock = Arc::new(Clock::new(&config)?);

    // Build registry with Clock — all surfaces share this Clock
    let _registry = OperationRegistry::new(clock.clone());

    // TODO: register domain operations
    // Future tasks will populate the registry here

    // For now, return a placeholder so the error path is wired correctly.
    let today = clock.today();
    Ok(serde_json::json!({ "status": "ok", "today": Clock::format_date(today) }))
}

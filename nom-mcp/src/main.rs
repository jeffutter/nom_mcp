//! nom-mcp — main binary for serving MCP + local CLI.
//!
//! Local-CLI path: parses arguments, dispatches to operations, renders errors
//! through the shared `cli_exit`/`render_error` functions from `nom-core`.

use nom_core::error::{ErrorData, cli_exit};

fn main() {
    // Initialize tracing for CLI mode (best-effort; failure doesn't crash)
    let _ = nom_core::logging::init_cli();

    let args: Vec<String> = std::env::args().collect();
    cli_exit(execute_from_args(&args));
}

/// Execute an operation from command-line arguments.
/// Returns structured JSON on success, or unified ErrorData on failure.
pub fn execute_from_args(_args: &[String]) -> Result<serde_json::Value, ErrorData> {
    // TODO: parse arguments, probe lock, dispatch to Operation registry.
    // For now, return a placeholder so the error path is wired correctly.
    Ok(serde_json::json!({ "status": "placeholder" }))
}

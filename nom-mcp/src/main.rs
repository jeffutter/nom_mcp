//! nom-mcp — main binary for serving MCP + local CLI.
//!
//! Local-CLI path: parses arguments, dispatches to operations, renders errors
//! through the shared `render_error` function from `nom-core`.

use nom_core::error::{ErrorData, render_error};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    
    match execute_from_args(&args) {
        Ok(value) => {
            println!("{}", serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string()));
        }
        Err(error) => {
            let (message, exit_code) = render_error(&error);
            eprintln!("{message}");
            std::process::exit(exit_code);
        }
    }
}

/// Execute an operation from command-line arguments.
/// Returns structured JSON on success, or unified ErrorData on failure.
pub fn execute_from_args(_args: &[String]) -> Result<serde_json::Value, ErrorData> {
    // TODO: parse arguments, probe lock, dispatch to Operation registry.
    // For now, return a placeholder so the error path is wired correctly.
    Ok(serde_json::json!({ "status": "placeholder" }))
}

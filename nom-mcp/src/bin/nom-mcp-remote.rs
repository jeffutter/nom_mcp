//! nom-mcp-remote — thin HTTP client for remote MCP access.
//!
//! `fetch_from_server` is currently a placeholder stub (see its TODO); once
//! it makes real HTTP requests it will deserialize `ErrorData` from error
//! responses and share the same `cli_exit`/`render_error` path as local-CLI
//! so their output is identical.

use nom_core::error::{ErrorData, cli_exit};

fn main() {
    // TODO: parse server URL from config/env, make HTTP request to remote server.
    // For now, demonstrate the error path with a placeholder.
    cli_exit(fetch_from_server());
}

/// Fetch data from the remote server.
/// Returns structured JSON on success, or deserialized ErrorData on failure.
fn fetch_from_server() -> Result<serde_json::Value, ErrorData> {
    // TODO: actually make HTTP request, check status code, deserialize body.
    // On error response, deserialize the body as ErrorData.
    //
    // Example flow:
    // ```
    // let resp = reqwest::get(server_url).send()?;
    // if resp.status().is_success() {
    //     Ok(resp.json::<serde_json::Value>()?)
    // } else {
    //     Err(resp.json::<ErrorData>()?)
    // }
    // ```
    Ok(serde_json::json!({ "status": "placeholder" }))
}

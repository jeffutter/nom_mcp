//! nom-mcp-remote — thin HTTP client for remote MCP access.
//!
//! Deserializes `ErrorData` from HTTP responses and uses the same shared
//! `render_error` function as the local-CLI so output is identical.

use nom_core::error::{ErrorData, render_error};

fn main() {
    // TODO: parse server URL from config/env, make HTTP request to remote server.
    // For now, demonstrate the error path with a placeholder.

    // Simulate fetching result from remote server
    let response = fetch_from_server();

    match response {
        Ok(value) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string())
            );
        }
        Err(error) => {
            let (message, exit_code) = render_error(&error);
            eprintln!("{message}");
            std::process::exit(exit_code);
        }
    }
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

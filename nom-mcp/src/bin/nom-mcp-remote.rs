//! nom-mcp-remote — thin HTTP client for remote MCP access.
//!
//! Parses CLI arguments, loads the server URL from config, makes HTTP requests
//! to the nom\_mcp server's `/api/{operation}` endpoints, and renders results
//! or errors through the shared `cli_exit`/`render_error` functions from
//! `nom-core`, producing output identical to local-CLI.

use std::collections::HashMap;

use nom_core::config::AppConfig;
use nom_core::error::{cli_exit, ErrorData};
use url::Url;

fn main() {
    // Initialize tracing for CLI mode (best-effort; failure doesn't crash)
    let _ = nom_core::logging::init_cli();

    let args: Vec<String> = std::env::args().collect();
    cli_exit(execute_from_args(&args));
}

/// Execute an operation against the remote server from command-line arguments.
pub fn execute_from_args(args: &[String]) -> Result<serde_json::Value, ErrorData> {
    if args.len() < 2 {
        return Err(ErrorData::validation("command", "no operation specified"));
    }

    let op_name = &args[1];
    let params = parse_params(&args[2..])?;

    // Load config to get server_url
    let config = AppConfig::load()
        .map_err(|e| ErrorData::storage_failure(format!("failed to load config: {e}")))?;

    let server_url = config
        .remote
        .server_url
        .ok_or_else(|| ErrorData::validation("server_url", "not configured"))?;

    let base_url = Url::parse(&server_url).map_err(|_| {
        ErrorData::validation("server_url", format!("invalid URL: {server_url}"))
    })?;

    fetch_from_server(base_url, op_name, params)
}

/// Parse key=value argument pairs into a JSON object.
/// Supports auto-typing: bare numbers become JSON numbers, "true"/"false" become booleans,
/// everything else is a string.
fn parse_params(args: &[String]) -> Result<serde_json::Value, ErrorData> {
    let mut map = HashMap::new();
    for arg in args {
        let pos = arg
            .find('=')
            .ok_or_else(|| ErrorData::validation("argument", format!("expected key=value, got: {arg}")))?;
        let key = &arg[..pos];
        let value = &arg[pos + 1..];
        map.insert(
            key.to_string(),
            parse_value(value),
        );
    }
    serde_json::to_value(map)
        .map_err(|e| ErrorData::storage_failure(format!("failed to serialize params: {e}")))
}

/// Auto-type a string value: numbers → JSON number, true/false → boolean, else string.
fn parse_value(s: &str) -> serde_json::Value {
    if s == "true" {
        serde_json::Value::Bool(true)
    } else if s == "false" {
        serde_json::Value::Bool(false)
    } else if let Ok(n) = s.parse::<i64>() {
        serde_json::json!(n)
    } else if let Ok(f) = s.parse::<f64>() {
        serde_json::json!(f)
    } else {
        serde_json::json!(s)
    }
}

/// Fetch data from the remote server via HTTP POST.
/// Returns structured JSON on success, or deserialized ErrorData on failure.
fn fetch_from_server(
    base_url: Url,
    operation: &str,
    params: serde_json::Value,
) -> Result<serde_json::Value, ErrorData> {
    // Build URL using path_segments_mut() to prevent injection
    let mut url = base_url;
    url.path_segments_mut()
        .map_err(|_| ErrorData::validation("server_url", "cannot be used as a base URL"))?
        .extend(["api", operation]);

    // Create HTTP client with user-agent and timeouts
    let version = env!("CARGO_PKG_VERSION");
    let client = reqwest::blocking::Client::builder()
        .user_agent(format!("nom-mcp-remote/{version}"))
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| ErrorData::external_api_failure(format!("failed to build HTTP client: {e}")))?;

    // Make the request
    let resp = client
        .post(url.as_str())
        .json(&params)
        .send()
        .map_err(|e| ErrorData::external_api_failure(format!("request failed: {e}")))?;

    let status = resp.status();

    if status.is_success() {
        resp.json::<serde_json::Value>()
            .map_err(|e| ErrorData::storage_failure(format!("failed to deserialize response: {e}")))
    } else {
        // Read body once, then try to deserialize as ErrorData for identical rendering
        let body = resp.text()
            .map_err(|e| ErrorData::external_api_failure(format!("failed to read error body: {e}")))?;
        match serde_json::from_str::<ErrorData>(&body) {
            Ok(error_data) => Err(error_data),
            Err(_) => Err(ErrorData::external_api_failure(format!(
                "server returned {}: {}",
                status, body
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- parse_value tests --

    #[test]
    fn test_parse_value_numbers() {
        assert_eq!(parse_value("42"), serde_json::json!(42));
        assert_eq!(parse_value("-7"), serde_json::json!(-7));
    }

    #[test]
    fn test_parse_value_floats() {
        assert_eq!(parse_value("2.71"), serde_json::Value::from(2.71_f64));
        assert_eq!(parse_value("-0.5"), serde_json::json!(-0.5));
    }

    #[test]
    fn test_parse_value_booleans() {
        assert_eq!(parse_value("true"), serde_json::json!(true));
        assert_eq!(parse_value("false"), serde_json::json!(false));
    }

    #[test]
    fn test_parse_value_strings() {
        assert_eq!(parse_value("hello"), serde_json::json!("hello"));
        assert_eq!(parse_value("123abc"), serde_json::json!("123abc"));
    }

    // -- parse_params tests --

    #[test]
    fn test_parse_params_empty() {
        let result = parse_params(&[]).unwrap();
        assert!(result.is_object());
        assert!(result.as_object().unwrap().is_empty());
    }

    #[test]
    fn test_parse_params_mixed_types() {
        let result = parse_params(&["name=widget".into(), "count=5".into(), "active=true".into()]).unwrap();
        assert_eq!(result["name"], "widget");
        assert_eq!(result["count"], 5);
        assert_eq!(result["active"], true);
    }

    #[test]
    fn test_parse_params_missing_equals() {
        let result = parse_params(&["bare-flag".into()]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.category, nom_core::error::ErrorCategory::Validation);
    }

    // -- execute_from_args validation tests --

    #[test]
    fn test_execute_no_args() {
        let result = execute_from_args(&["nom-mcp-remote".into()]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.category, nom_core::error::ErrorCategory::Validation);
    }

    // Integration tests — run blocking HTTP client on a std::thread to avoid
    // dropping the tokio runtime from within an async context.
    #[test]
    fn test_fetch_from_server_success() {
        std::thread::spawn(|| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let server = rt.block_on(wiremock::MockServer::start());
            let base_url = server.uri().parse().unwrap();

            rt.block_on(
                wiremock::Mock::given(wiremock::matchers::method("POST"))
                    .respond_with(
                        wiremock::ResponseTemplate::new(200)
                            .set_body_json(serde_json::json!({ "result": "ok" }))
                    )
                    .expect(1)
                    .mount(&server),
            );

            let result = fetch_from_server(
                base_url,
                "search_food",
                serde_json::json!({}),
            );
            assert!(result.is_ok());
            assert_eq!(result.unwrap()["result"], "ok");
        })
        .join()
        .unwrap();
    }

    #[test]
    fn test_fetch_from_server_error_response() {
        std::thread::spawn(|| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let server = rt.block_on(wiremock::MockServer::start());
            let base_url = server.uri().parse().unwrap();

            rt.block_on(
                wiremock::Mock::given(wiremock::matchers::method("POST"))
                    .respond_with(
                        wiremock::ResponseTemplate::new(400)
                            .set_body_json(serde_json::json!({
                                "category": "Validation",
                                "field": "query",
                                "reason": "empty query"
                            }))
                    )
                    .expect(1)
                    .mount(&server),
            );

            let result = fetch_from_server(
                base_url,
                "search_food",
                serde_json::json!({}),
            );
            assert!(result.is_err());
            let err = result.unwrap_err();
            assert_eq!(err.category, nom_core::error::ErrorCategory::Validation);
            assert_eq!(err.field, Some("query".to_string()));
            assert_eq!(err.reason, Some("empty query".to_string()));
        })
        .join()
        .unwrap();
    }

    #[test]
    fn test_fetch_from_server_network_error() {
        std::thread::spawn(|| {
            let base_url = Url::parse("http://127.0.0.1:54321").unwrap();
            let result = fetch_from_server(
                base_url,
                "search_food",
                serde_json::json!({}),
            );
            assert!(result.is_err());
            let err = result.unwrap_err();
            assert_eq!(err.category, nom_core::error::ErrorCategory::ExternalApiFailure);
        })
        .join()
        .unwrap();
    }

    #[test]
    fn test_fetch_from_server_injection_prevented() {
        std::thread::spawn(|| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let server = rt.block_on(wiremock::MockServer::start());
            let base_url = server.uri().parse().unwrap();

            rt.block_on(
                wiremock::Mock::given(wiremock::matchers::method("POST"))
                    .and(wiremock::matchers::path("/api/search%2Ffood"))
                    .respond_with(
                        wiremock::ResponseTemplate::new(200)
                            .set_body_json(serde_json::json!({ "safe": true }))
                    )
                    .expect(1)
                    .mount(&server),
            );

            let result = fetch_from_server(
                base_url,
                "search/food",
                serde_json::json!({}),
            );
            assert!(result.is_ok());
        })
        .join()
        .unwrap();
    }
}

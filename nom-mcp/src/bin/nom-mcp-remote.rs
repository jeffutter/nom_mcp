//! nom-mcp-remote — thin HTTP client for remote MCP access.
//!
//! Parses CLI arguments, loads the server URL from config, makes HTTP requests
//! to the nom\_mcp server's `/api/{operation}` endpoints, and renders results
//! or errors through the shared `cli_exit`/`render_error` functions from
//! `nom-core`, producing output identical to local-CLI.

use nom_core::cli::parse_params;
use nom_core::config::AppConfig;
use nom_core::error::{ErrorData, cli_exit};
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

    let base_url = Url::parse(&server_url)
        .map_err(|_| ErrorData::validation("server_url", format!("invalid URL: {server_url}")))?;

    fetch_from_server(base_url, op_name, params)
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
        .map_err(|e| {
            ErrorData::external_api_failure(format!("failed to build HTTP client: {e}"))
        })?;

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
        let body = resp.text().map_err(|e| {
            ErrorData::external_api_failure(format!("failed to read error body: {e}"))
        })?;
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

    // -- TestGuard for config isolation.
    // Keep in sync with the identical TestGuard in nom-core/src/config.rs --
    struct TestGuard {
        temp_dir: Option<std::path::PathBuf>,
        saved_xdg: Option<String>,
        cleared_vars: Vec<String>,
    }

    impl TestGuard {
        fn new() -> Self {
            Self {
                temp_dir: None,
                saved_xdg: std::env::var_os("XDG_CONFIG_HOME")
                    .map(|v| v.to_string_lossy().to_string()),
                cleared_vars: Vec::new(),
            }
        }

        fn set(&mut self, key: &str, value: &str) {
            unsafe { std::env::set_var(key, value) };
            self.cleared_vars.push(key.to_string());
        }

        fn set_temp_dir(&mut self, path: std::path::PathBuf) {
            self.temp_dir = Some(path);
        }
    }

    impl Drop for TestGuard {
        fn drop(&mut self) {
            if let Some(saved) = &self.saved_xdg {
                if saved.is_empty() {
                    unsafe { std::env::remove_var("XDG_CONFIG_HOME") };
                } else {
                    unsafe { std::env::set_var("XDG_CONFIG_HOME", saved) };
                }
            }
            // Remove any test-specific env vars.
            // Skip XDG_CONFIG_HOME — the block above already restored it.
            for var in &self.cleared_vars {
                if var == "XDG_CONFIG_HOME" {
                    continue;
                }
                unsafe { std::env::remove_var(var) };
            }
            if let Some(ref dir) = self.temp_dir {
                let _ = std::fs::remove_dir_all(dir);
            }
        }
    }

    // -- execute_from_args validation tests --

    #[test]
    fn test_execute_no_args() {
        let result = execute_from_args(&["nom-mcp-remote".into()]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.category, nom_core::error::ErrorCategory::Validation);
    }

    #[serial_test::serial]
    #[test]
    fn test_execute_from_args_missing_server_url() {
        // Point XDG_CONFIG_HOME at a nonexistent dir so no config.toml is loaded.
        // AppConfig will load with defaults, and remote.server_url will be None.
        let mut guard = TestGuard::new();
        guard.set("XDG_CONFIG_HOME", "/tmp/nom_mcp_test_missing_url_12345");

        let result = execute_from_args(&[
            "nom-mcp-remote".into(),
            "search_food".into(),
            "query=almonds".into(),
        ]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.category, nom_core::error::ErrorCategory::Validation);
        assert_eq!(err.field, Some("server_url".to_string()));
    }

    #[serial_test::serial]
    #[test]
    fn test_execute_from_args_invalid_server_url() {
        // Create a temp config with a malformed server_url.
        let mut guard = TestGuard::new();
        let temp_dir = std::env::temp_dir().join("nom_mcp_test_invalid_url");
        let config_dir = temp_dir.join("config");
        let file_path = config_dir.join("nom_mcp").join("config.toml");

        std::fs::create_dir_all(config_dir.join("nom_mcp")).ok();
        std::fs::write(
            &file_path,
            r#"[remote]
server_url = "not a url"
"#,
        )
        .expect("failed to write test config");

        guard.set_temp_dir(temp_dir.clone());
        guard.set("XDG_CONFIG_HOME", &config_dir.to_string_lossy());

        let result = execute_from_args(&[
            "nom-mcp-remote".into(),
            "search_food".into(),
            "query=almonds".into(),
        ]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.category, nom_core::error::ErrorCategory::Validation);
        assert_eq!(err.field, Some("server_url".to_string()));
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
                            .set_body_json(serde_json::json!({ "result": "ok" })),
                    )
                    .expect(1)
                    .mount(&server),
            );

            let result = fetch_from_server(base_url, "search_food", serde_json::json!({}));
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
                    .respond_with(wiremock::ResponseTemplate::new(400).set_body_json(
                        serde_json::json!({
                            "category": "Validation",
                            "field": "query",
                            "reason": "empty query"
                        }),
                    ))
                    .expect(1)
                    .mount(&server),
            );

            let result = fetch_from_server(base_url, "search_food", serde_json::json!({}));
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
            let result = fetch_from_server(base_url, "search_food", serde_json::json!({}));
            assert!(result.is_err());
            let err = result.unwrap_err();
            assert_eq!(
                err.category,
                nom_core::error::ErrorCategory::ExternalApiFailure
            );
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
                            .set_body_json(serde_json::json!({ "safe": true })),
                    )
                    .expect(1)
                    .mount(&server),
            );

            let result = fetch_from_server(base_url, "search/food", serde_json::json!({}));
            assert!(result.is_ok());
        })
        .join()
        .unwrap();
    }
}

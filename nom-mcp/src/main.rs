//! nom-mcp — main binary for serving MCP + local CLI.
//!
//! Local-CLI path: parses arguments, dispatches to operations, renders errors
//! through the shared `cli_exit`/`render_error` functions from `nom-core`.

use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::Arc;

use nom_core::client::{off::OffClient, usda::FdcClient};
use nom_core::clock::Clock;
use nom_core::config::{AppConfig, load_or_create_off_app_uuid};
use nom_core::error::{ErrorData, cli_exit};
use nom_core::food::{CreateCustomFood, SearchFood};
use nom_core::goal::{GetGoalProgress, SetNutritionGoals};
use nom_core::meal::{DeleteMeal, GetMealsByDateRange, LogMeal, SearchMeals, UpdateMeal};
use nom_core::operation::{OperationRegistry, cli_router};
use nom_core::weekly::GetWeeklyProgress;
use nom_core::weight::{
    DeleteWeightEntry, GetWeightByDate, GetWeightByDateRange, GetWeightToday, LogWeight,
    UpdateWeightEntry,
};
use nom_core::widget::{GetWidgetDisplay, SetWidgetDisplay};

fn main() {
    // The 'serve' subcommand runs a long-lived MCP server (stdio or HTTP)
    // instead of the one-shot local-CLI dispatch; it has its own tracing
    // defaults (info vs warn) so we branch before initializing logging.
    if std::env::args().nth(1).as_deref() == Some("serve") {
        let args: Vec<String> = std::env::args().collect();
        let result = match parse_serve_mode(&args) {
            ServeMode::Stdio => run_serve_stdio(),
            ServeMode::Http { port } => run_serve_http(port),
            ServeMode::Unknown(mode) => {
                eprintln!("nom-mcp serve: unknown mode '{mode}' (expected 'stdio' or 'http')");
                std::process::exit(1);
            }
        };
        if let Err(err) = result {
            eprintln!("nom-mcp serve failed: {err}");
            std::process::exit(1);
        }
        return;
    }

    // Initialize tracing for CLI mode (best-effort; failure doesn't crash)
    let _ = nom_core::logging::init_cli();

    let args: Vec<String> = std::env::args().collect();
    cli_exit(execute_from_args(&args));
}

/// Which server transport `nom-mcp serve` should run, as parsed from argv.
#[derive(Debug, PartialEq, Eq)]
enum ServeMode {
    Stdio,
    Http { port: u16 },
    Unknown(String),
}

/// Parse `serve [stdio|http [--port N]]` from raw argv (args[0] is the binary
/// name, args[1] is "serve"). Bare `serve` and `serve stdio` are equivalent
/// (TASK-34 backward compatibility). Default HTTP port is 8000 (matches
/// notectl's `serve http` convention; doc-5 states no specific default).
fn parse_serve_mode(args: &[String]) -> ServeMode {
    match args.get(2).map(String::as_str) {
        None | Some("stdio") => ServeMode::Stdio,
        Some("http") => {
            let port = args
                .iter()
                .position(|a| a == "--port")
                .and_then(|i| args.get(i + 1))
                .and_then(|v| v.parse::<u16>().ok())
                .unwrap_or(8000);
            ServeMode::Http { port }
        }
        Some(other) => ServeMode::Unknown(other.to_string()),
    }
}

/// Build the OperationRegistry shared by both the local-CLI path and the MCP
/// serve path, so both surfaces register the identical set of operations.
fn build_registry(
    clock: Arc<Clock>,
    off_client: Arc<OffClient>,
    fdc_client: Option<Arc<FdcClient>>,
) -> OperationRegistry {
    let mut registry = OperationRegistry::new(clock.clone());

    // Register food operations
    registry.register(Arc::new(SearchFood::new(off_client, fdc_client)));
    registry.register(Arc::new(CreateCustomFood::new()));

    // Register meal operations
    registry.register(Arc::new(LogMeal::new(*clock)));
    registry.register(Arc::new(UpdateMeal::new(*clock)));
    registry.register(Arc::new(DeleteMeal::new()));
    registry.register(Arc::new(SearchMeals::new()));
    registry.register(Arc::new(GetMealsByDateRange::new()));

    // Register goal operations
    registry.register(Arc::new(SetNutritionGoals::new(*clock)));
    registry.register(Arc::new(GetGoalProgress::new(*clock)));

    // Register weight operations
    registry.register(Arc::new(LogWeight::new(*clock)));
    registry.register(Arc::new(UpdateWeightEntry::new(*clock)));
    registry.register(Arc::new(DeleteWeightEntry::new()));
    registry.register(Arc::new(GetWeightToday::new(*clock)));
    registry.register(Arc::new(GetWeightByDate::new()));
    registry.register(Arc::new(GetWeightByDateRange::new()));

    // Register widget display operations (MCP-only)
    registry.register(Arc::new(GetWidgetDisplay::new()));
    registry.register(Arc::new(SetWidgetDisplay::new()));

    // Register weekly progress operation (MCP-only)
    registry.register(Arc::new(GetWeeklyProgress::new(*clock)));

    registry
}

/// Load config and build the clients (OFF, optional FDC) shared by both the
/// local-CLI path and the MCP serve path.
fn build_clients(
    config: &AppConfig,
) -> Result<(Arc<OffClient>, Option<Arc<FdcClient>>), ErrorData> {
    let mut off_client = OffClient::new("https://world.openfoodfacts.org", &config.off_user_agent)
        .map_err(|e| ErrorData::storage_failure(format!("OFF client init failed: {e}")))?;

    // Optional OFF Basic-auth credentials (env vars NOM_MCP_OFF_USERNAME /
    // NOM_MCP_OFF_PASSWORD or the TOML config). Both must be set together;
    // otherwise requests go out unauthenticated and we warn at startup.
    match (config.off_username.as_ref(), config.off_password.as_ref()) {
        (Some(u), Some(p)) if !u.get().is_empty() && !p.get().is_empty() => {
            off_client = off_client.with_basic_auth(u.get(), p.get());
        }
        (None, None) => {
            tracing::warn!(
                "OpenFoodFacts credentials not configured (set NOM_MCP_OFF_USERNAME and \
                 NOM_MCP_OFF_PASSWORD); OFF requests will be unauthenticated"
            );
        }
        _ => {
            tracing::warn!(
                "OpenFoodFacts credentials incomplete (both off_username and off_password \
                 must be set); falling back to unauthenticated OFF requests"
            );
        }
    }

    // OFF app-identity params (`app_name`/`app_version`/`app_uuid`) — required
    // on write queries, sent on every request so OFF can attribute usage to
    // this app and installation. The uuid is a stable random per-installation
    // identifier: taken from config when provided, otherwise generated once
    // and persisted under `$XDG_DATA_HOME/nom_mcp/`.
    let app_uuid = config
        .off_app_uuid
        .clone()
        .or_else(|| match load_or_create_off_app_uuid() {
            Ok(uuid) => Some(uuid),
            Err(e) => {
                tracing::warn!(
                    "could not persist OpenFoodFacts app_uuid ({e}); OFF requests will omit it"
                );
                None
            }
        });
    off_client =
        off_client.with_app_identity("nom_mcp", env!("CARGO_PKG_VERSION"), app_uuid.as_deref());
    let off_client = Arc::new(off_client);

    // USDA FDC client is optional — validated lazily when search_food runs
    let fdc_client: Option<Arc<FdcClient>> = config
        .usda_api_key
        .as_ref()
        .map(|key| {
            FdcClient::new("https://api.nal.usda.gov/fdc", key.get())
                .map(Arc::new)
                .map_err(|e| ErrorData::storage_failure(format!("FDC client init failed: {e}")))
        })
        .transpose()?;

    Ok((off_client, fdc_client))
}

/// Execute an operation from command-line arguments.
/// Returns structured JSON on success, or unified ErrorData on failure.
pub fn execute_from_args(args: &[String]) -> Result<serde_json::Value, ErrorData> {
    // Load config
    let config = AppConfig::load()
        .map_err(|e| ErrorData::storage_failure(format!("failed to load config: {e}")))?;

    let clock = Arc::new(Clock::new(&config)?);
    let (off_client, fdc_client) = build_clients(&config)?;
    let registry = build_registry(clock.clone(), off_client, fdc_client);

    // Dispatch based on CLI subcommand (clap-backed: `--help`/`-h` print usage and exit)
    let (op, op_args) = cli_router::parse_and_dispatch(&registry, args)?;

    // Run the async operation in a tokio runtime
    tokio::runtime::Runtime::new()
        .map_err(|e| ErrorData::storage_failure(format!("failed to create runtime: {e}")))?
        .block_on(op.execute_json(op_args))
}

/// Build the clock and operation registry shared by both serve transports,
/// so stdio and HTTP serve modes can never diverge in how they're
/// constructed (TASK-34/TASK-35 AC #4).
fn build_serve_context(
    config: &AppConfig,
) -> Result<(Arc<Clock>, Arc<OperationRegistry>), ErrorData> {
    let clock = Arc::new(Clock::new(config)?);
    let (off_client, fdc_client) = build_clients(config)?;
    let registry = Arc::new(build_registry(clock.clone(), off_client, fdc_client));
    Ok((clock, registry))
}

/// Run nom-mcp as a real MCP server over stdio, blocking until the client
/// disconnects. Logs go to stderr (via `nom_core::logging::init_server`);
/// stdout is reserved for the MCP JSON-RPC protocol.
fn run_serve_stdio() -> Result<(), Box<dyn std::error::Error>> {
    let _ = nom_core::logging::init_server();

    let config = AppConfig::load()?;
    let (clock, registry) = build_serve_context(&config)?;
    let handler = nom_core::operation::mcp_handler::McpHandler::new(registry, *clock);

    tokio::runtime::Runtime::new()?.block_on(async {
        use rmcp::ServiceExt;
        let service = handler.serve(rmcp::transport::stdio()).await?;
        service.waiting().await?;
        Ok::<_, Box<dyn std::error::Error>>(())
    })
}

/// Resolve a configured bind address (IPv4/IPv6 literal, or hostname) plus a
/// port into a concrete `SocketAddr`.
///
/// `bind_address` is tried as a bare IP literal first — this is the common
/// case (`127.0.0.1`, `::1`, `::`) and avoids the ambiguity of naively
/// joining an IPv6 literal with `:port` (e.g. `::1:8000`, which
/// `SocketAddr`/`TcpListener` parsing rejects since a bare IPv6 literal must
/// be bracketed before a port is appended). If it isn't a bare IP literal —
/// e.g. a hostname like `localhost` — it falls back to `ToSocketAddrs`
/// resolution of `"{bind_address}:{port}"`, matching what
/// `TcpListener::bind(&str)` did implicitly before this function existed.
fn resolve_bind_addr(bind_address: &str, port: u16) -> std::io::Result<SocketAddr> {
    if let Ok(ip) = bind_address.parse::<std::net::IpAddr>() {
        return Ok(SocketAddr::new(ip, port));
    }

    format!("{bind_address}:{port}")
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("could not resolve bind address: {bind_address}"),
            )
        })
}

/// TEMPORARY debug logging (investigating widget gating since 81d32ff):
/// log every HTTP request hitting the `/mcp` endpoint — including ones rmcp
/// rejects before they reach the `McpHandler` (e.g. Claude iOS's bogus
/// `MCP-Protocol-Version: 2026-01-26` header) — so the full per-session
/// request stream is visible alongside the identity logs from
/// `on_initialized`. Remove once the investigation is done.
async fn debug_log_mcp_request(
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    if req.uri().path().starts_with("/mcp") {
        tracing::debug!(
            method = %req.method(),
            path = %req.uri().path(),
            mcp_session_id = ?req.headers().get(axum::http::header::HeaderName::from_static("mcp-session-id")),
            mcp_protocol_version = ?req.headers().get(axum::http::header::HeaderName::from_static("mcp-protocol-version")),
            "MCP HTTP request"
        );
    }
    next.run(req).await
}

/// Run nom-mcp as an HTTP server exposing both the REST API (`/api/*`) and a
/// streamable-HTTP MCP endpoint (`/mcp`) on a single listener, sharing the
/// same registry/clock construction path as the stdio serve mode. Logs go to
/// stderr (via `nom_core::logging::init_server`), consistent with stdio serve.
fn run_serve_http(port: u16) -> Result<(), Box<dyn std::error::Error>> {
    let _ = nom_core::logging::init_server();

    let config = AppConfig::load()?;
    let (clock, registry) = build_serve_context(&config)?;
    let handler = nom_core::operation::mcp_handler::McpHandler::new(registry.clone(), *clock);
    let bind_address = config.http_bind_address.clone();

    tokio::runtime::Runtime::new()?.block_on(async {
        use rmcp::transport::streamable_http_server::{
            StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
        };
        use tokio_util::sync::CancellationToken;

        let ct = CancellationToken::new();
        let mcp_config = StreamableHttpServerConfig::default().with_cancellation_token(ct.clone());
        let mcp_service = StreamableHttpService::new(
            move || Ok(handler.clone()),
            Arc::new(LocalSessionManager::default()),
            mcp_config,
        );

        let router = nom_core::operation::http_router::build_http_router(registry)
            .nest_service("/mcp", mcp_service)
            .layer(axum::middleware::from_fn(debug_log_mcp_request));

        let addr = resolve_bind_addr(&bind_address, port)?;
        let listener = tokio::net::TcpListener::bind(addr).await?;
        tracing::info!(%addr, "nom-mcp HTTP serve mode listening (REST at /api/*, MCP at /mcp)");

        let mut sigterm =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
        axum::serve(listener, router.into_make_service())
            .with_graceful_shutdown(async move {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {}
                    _ = sigterm.recv() => {}
                }
                ct.cancel();
            })
            .await?;

        Ok::<_, Box<dyn std::error::Error>>(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn test_parse_serve_mode_bare_serve_is_stdio() {
        assert_eq!(
            parse_serve_mode(&args(&["nom-mcp", "serve"])),
            ServeMode::Stdio
        );
    }

    #[test]
    fn test_parse_serve_mode_explicit_stdio() {
        assert_eq!(
            parse_serve_mode(&args(&["nom-mcp", "serve", "stdio"])),
            ServeMode::Stdio
        );
    }

    #[test]
    fn test_parse_serve_mode_http_default_port() {
        assert_eq!(
            parse_serve_mode(&args(&["nom-mcp", "serve", "http"])),
            ServeMode::Http { port: 8000 }
        );
    }

    #[test]
    fn test_parse_serve_mode_http_explicit_port() {
        assert_eq!(
            parse_serve_mode(&args(&["nom-mcp", "serve", "http", "--port", "9999"])),
            ServeMode::Http { port: 9999 }
        );
    }

    #[test]
    fn test_parse_serve_mode_unknown() {
        assert_eq!(
            parse_serve_mode(&args(&["nom-mcp", "serve", "bogus"])),
            ServeMode::Unknown("bogus".to_string())
        );
    }

    #[test]
    fn test_resolve_bind_addr_ipv4_literal() {
        let addr = resolve_bind_addr("127.0.0.1", 8000).unwrap();
        assert_eq!(addr.to_string(), "127.0.0.1:8000");
    }

    #[test]
    fn test_resolve_bind_addr_ipv6_loopback() {
        let addr = resolve_bind_addr("::1", 8000).unwrap();
        assert_eq!(addr.to_string(), "[::1]:8000");
    }

    #[test]
    fn test_resolve_bind_addr_ipv6_unspecified() {
        let addr = resolve_bind_addr("::", 8000).unwrap();
        assert_eq!(addr.to_string(), "[::]:8000");
    }

    #[test]
    fn test_resolve_bind_addr_hostname_resolves_via_fallback() {
        // "localhost" isn't a bare IP literal, so this exercises the
        // ToSocketAddrs fallback path rather than the literal-parse path.
        let addr = resolve_bind_addr("localhost", 8000).unwrap();
        assert_eq!(addr.port(), 8000);
        assert!(addr.ip().is_loopback());
    }
}

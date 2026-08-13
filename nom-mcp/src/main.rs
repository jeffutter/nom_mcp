//! nom-mcp — main binary for serving MCP + local CLI.
//!
//! Local-CLI path: parses arguments, dispatches to operations, renders errors
//! through the shared `cli_exit`/`render_error` functions from `nom-core`.

use std::sync::Arc;

use nom_core::client::{off::OffClient, usda::FdcClient};
use nom_core::clock::Clock;
use nom_core::config::AppConfig;
use nom_core::error::{ErrorData, cli_exit};
use nom_core::food::{CreateCustomFood, SearchFood};
use nom_core::goal::{GetGoalProgress, SetNutritionGoals};
use nom_core::meal::{DeleteMeal, GetMealsByDateRange, LogMeal, SearchMeals, UpdateMeal};
use nom_core::operation::{OperationRegistry, cli_router};
use nom_core::weight::{
    DeleteWeightEntry, GetWeightByDate, GetWeightByDateRange, GetWeightToday, LogWeight,
    UpdateWeightEntry,
};
use nom_core::widget::{GetWidgetDisplay, SetWidgetDisplay};

fn main() {
    // The 'serve' subcommand runs a long-lived MCP stdio server instead of the
    // one-shot local-CLI dispatch; it has its own tracing defaults (info vs
    // warn) so we branch before initializing logging.
    if std::env::args().nth(1).as_deref() == Some("serve") {
        if let Err(err) = run_serve() {
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

    registry
}

/// Load config and build the clients (OFF, optional FDC) shared by both the
/// local-CLI path and the MCP serve path.
fn build_clients(
    config: &AppConfig,
) -> Result<(Arc<OffClient>, Option<Arc<FdcClient>>), ErrorData> {
    let off_client = Arc::new(
        OffClient::new("https://world.openfoodfacts.org", &config.off_user_agent)
            .map_err(|e| ErrorData::storage_failure(format!("OFF client init failed: {e}")))?,
    );

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

/// Run nom-mcp as a real MCP server over stdio, blocking until the client
/// disconnects. Logs go to stderr (via `nom_core::logging::init_server`);
/// stdout is reserved for the MCP JSON-RPC protocol.
fn run_serve() -> Result<(), Box<dyn std::error::Error>> {
    let _ = nom_core::logging::init_server();

    let config = AppConfig::load()?;
    let clock = Arc::new(Clock::new(&config)?);
    let (off_client, fdc_client) = build_clients(&config)?;
    let registry = build_registry(clock.clone(), off_client, fdc_client);
    let handler = nom_core::operation::mcp_handler::McpHandler::new(registry, *clock);

    tokio::runtime::Runtime::new()?.block_on(async {
        use rmcp::ServiceExt;
        let service = handler.serve(rmcp::transport::stdio()).await?;
        service.waiting().await?;
        Ok::<_, Box<dyn std::error::Error>>(())
    })
}

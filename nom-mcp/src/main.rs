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
use nom_core::meal::{DeleteMeal, GetMealsByDateRange, LogMeal, SearchMeals, UpdateMeal};
use nom_core::operation::OperationRegistry;

fn main() {
    // Initialize tracing for CLI mode (best-effort; failure doesn't crash)
    let _ = nom_core::logging::init_cli();

    let args: Vec<String> = std::env::args().collect();
    cli_exit(execute_from_args(&args));
}

/// Execute an operation from command-line arguments.
/// Returns structured JSON on success, or unified ErrorData on failure.
pub fn execute_from_args(args: &[String]) -> Result<serde_json::Value, ErrorData> {
    // Load config
    let config = AppConfig::load()
        .map_err(|e| ErrorData::storage_failure(format!("failed to load config: {e}")))?;

    let clock = Arc::new(Clock::new(&config)?);

    // Build clients
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

    // Build registry with Clock — all surfaces share this Clock
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

    // Dispatch based on CLI subcommand
    if args.len() < 2 {
        return Err(ErrorData::validation("command", "no subcommand provided"));
    }

    let op_name = &args[1];
    let op_args = serde_json::json!({}); // TODO: parse args[2..] into JSON

    let Some(op) = registry.get(op_name) else {
        return Err(ErrorData::not_found());
    };

    // Run the async operation in a tokio runtime
    tokio::runtime::Runtime::new()
        .map_err(|e| ErrorData::storage_failure(format!("failed to create runtime: {e}")))?
        .block_on(op.execute_json(Arc::new(op_args)))
}

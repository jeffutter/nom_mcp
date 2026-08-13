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

    // Dispatch based on CLI subcommand (clap-backed: `--help`/`-h` print usage and exit)
    let (op, op_args) = cli_router::parse_and_dispatch(&registry, args)?;

    // Run the async operation in a tokio runtime
    tokio::runtime::Runtime::new()
        .map_err(|e| ErrorData::storage_failure(format!("failed to create runtime: {e}")))?
        .block_on(op.execute_json(op_args))
}

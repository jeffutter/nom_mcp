
<!-- BACKLOG.MD MCP GUIDELINES START -->
<!-- backlog.md-instructions-version: 1.49.3 -->

<CRITICAL_INSTRUCTION>

## BACKLOG WORKFLOW INSTRUCTIONS

This project uses Backlog.md MCP for all task and project management activities.

**CRITICAL GUIDANCE**

- If your client supports MCP resources, read `backlog://workflow/overview` to understand when and how to use Backlog for this project.
- If your client only supports tools or the above request fails, call `backlog.get_backlog_instructions()` to load the tool-oriented overview. Use the `instruction` selector when you need `task-creation`, `task-execution`, or `task-finalization`.

- **First time working here?** Read the overview resource IMMEDIATELY to learn the workflow
- **Already familiar?** You should have the overview cached ("## Backlog.md Overview (MCP)")
- **When to read it**: BEFORE creating tasks, or when you're unsure whether to track work

These guides cover:
- Decision framework for when to create tasks
- Search-first workflow to avoid duplicates
- Links to detailed guides for task creation, execution, and finalization
- MCP tools reference

You MUST read the overview resource to understand the complete workflow. The information is NOT summarized here.

</CRITICAL_INSTRUCTION>

<!-- BACKLOG.MD MCP GUIDELINES END -->

## Project overview

`nom_mcp` is a single-user Rust MCP server for tracking food, nutrition, and body weight — exposed identically over MCP (stdio or streamable-HTTP), local CLI, a REST HTTP API, and a remote-CLI thin client. It's a Cargo workspace with two crates:

- **`nom-core`** — all domain logic: entities (Food, Meal, Portion, Weight Entry, Goal), storage (turso/SQLite), external API clients (OpenFoodFacts, USDA FDC), config, and the `Operation` trait that drives every transport surface.
- **`nom-mcp`** — thin binaries: `nom-mcp` (local CLI, and `serve` subcommand for MCP stdio / HTTP+MCP server modes, registers all operations) and `nom-mcp-remote` (HTTP client CLI that talks to a running `nom-mcp serve http` server).

**Status: v1 complete.** All operations across food/meal/weight/goal/widget tracking, both `serve` transports (stdio and HTTP), the `nom://weekly-summary` MCP resource, and `nom-mcp-remote` are implemented and tested. The v1 tracking epic (`backlog/tasks/task-2 - Build-nom_mcp-v1.md`, status: Done) is closed. Ongoing work is tracked as new Backlog.md tasks as they come up.

## Commands

This project uses Nix flakes for a pinned toolchain (`nix develop`). CI runs everything through `nix develop .#ci -c <cmd>`; use the same prefix locally if you don't already have a shell with the pinned Rust toolchain active.

```sh
# Build
cargo build --workspace

# Run all tests (matches CI — nextest doesn't run doctests, so pair it with `cargo test --doc`)
cargo nextest run --all-features --workspace
cargo test --doc --all-features --workspace

# Run a single test
cargo nextest run --workspace test_name_substring
cargo nextest run -p nom-core storage::migration::tests::test_migration_idempotency

# Formatting (CI fails on unformatted code)
cargo fmt --all
cargo fmt --all --check

# Lint (CI fails on any clippy warning)
cargo clippy --all-targets --all-features --workspace -- -D warnings

# Docs (CI fails on any rustdoc warning)
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --document-private-items --all-features --workspace --examples

# Run the local CLI directly
cargo run -p nom-mcp --bin nom-mcp -- <operation> key=value ...

# Run the MCP/HTTP server
cargo run -p nom-mcp --bin nom-mcp -- serve stdio
cargo run -p nom-mcp --bin nom-mcp -- serve http --port 8000
```

The `nom-core/tests/lock_probe_integration.rs` integration test spawns the `lock_holder` helper binary (`nom-core/src/lock_holder.rs`) via `CARGO_BIN_EXE_lock_holder`; only Cargo's own integration-test harness sets that env var, so that test must stay an integration test rather than move into the lib's `#[cfg(test)]` modules.

## Architecture

### One `Operation`, four surfaces

The entire system is built around a single abstraction in `nom-core/src/operation/mod.rs`: the `Operation` trait. Every domain action (search_food, log_meal, get_weight_by_date, ...) is a struct implementing:

```rust
trait Operation {
    fn name(&self) -> &str;              // CLI subcommand, HTTP path segment, MCP tool name
    fn description(&self) -> &str;
    fn surfaces(&self) -> Surfaces;       // bitflags: CLI | HTTP | MCP (default ALL)
    fn input_schema(&self) -> Option<serde_json::Value>;
    async fn execute_json(&self, args: Arc<serde_json::Value>) -> Result<serde_json::Value, ErrorData>;
}
```

A single `OperationRegistry` (`nom-core/src/operation/registry.rs`) holds `Vec<Arc<dyn Operation>>` plus the shared `Clock`. All three routers read from the *same* registry instance:

- `cli_router.rs` — builds a clap `Command` tree from `Surfaces::CLI` ops; two-phase (static command tree, then raw-args-to-JSON at dispatch)
- `http_router.rs` — builds an axum `Router` with one `POST /api/{name}` route per `Surfaces::HTTP` op
- `mcp_handler.rs` — exposes `Surfaces::MCP` ops as MCP tools

Adding an operation and registering it once (see `build_registry()` in `nom-mcp/src/main.rs`) makes it appear on every surface it declares — this is what closes CLI/HTTP/MCP drift by construction. When adding a new domain action, implement `Operation` in the relevant entity module (`food`, `meal`, `weight`, `goal`, `widget`) and register it in `nom-mcp/src/main.rs`; do not hand-write a separate CLI arg parser, HTTP handler, or MCP tool definition. Widget display operations (`get_widget_display`, `set_widget_display`) are the one exception that restricts `surfaces()` to `Surfaces::MCP` only — they don't make sense as local-CLI or REST actions.

For fieldless request structs backing `input_schema()`, use `struct FooRequest {}`, not a unit struct (`struct FooRequest;`) — schemars derives `{"type": "null"}` for the latter, which MCP clients reject since `inputSchema.type` must be `"object"`.

`nom-mcp-remote` is a *separate* binary that does not use the registry at all — it POSTs directly to `/api/{operation}` on a configured remote server and renders the response through the same `cli_exit`/`render_error` functions, so its output is byte-identical to local-CLI output.

### `serve`: stdio and HTTP transports

`nom-mcp serve [stdio|http [--port N]]` (`nom-mcp/src/main.rs`) runs a long-lived server instead of the one-shot local-CLI dispatch; bare `serve` and `serve stdio` are equivalent, and HTTP defaults to port 8000. Both transports are built from the identical `build_serve_context()` (clock + registry construction), so they can never diverge in what operations they expose:

- **stdio** — a real MCP server over stdio (`rmcp::transport::stdio()`), for MCP clients that spawn the binary directly (e.g. Claude Desktop). Blocks until the client disconnects.
- **http** — a single axum listener exposing both the REST API (`POST /api/{operation}`, from `http_router.rs`) and a streamable-HTTP MCP endpoint at `/mcp` (from `mcp_handler.rs` via `rmcp`'s `StreamableHttpService`), so one process serves both `nom-mcp-remote` and remote MCP clients. Binds to `http_bind_address:port`, handles both SIGINT and SIGTERM for graceful shutdown.

Both serve modes log to stderr (`nom_core::logging::init_server`) since stdio mode reserves stdout for the MCP JSON-RPC protocol.

The MCP surface additionally exposes one read-only resource, `nom://weekly-summary` (`nom-core/src/weekly/mod.rs`), returning a rolling 7-day nutrition/weight summary computed against the active goal — resources are handled separately from tools in `mcp_handler.rs` (`list_resources`/`read_resource`).

### Unified error taxonomy

`nom-core/src/error.rs` defines `ErrorData { category, field, reason }` as the single error currency across all four surfaces. `ErrorCategory` (NotFound/Validation/Conflict/ExternalApiFailure/StorageFailure) maps deterministically to both an HTTP status code and a CLI exit code. `render_error`/`cli_exit` are shared by local-CLI and remote-CLI so their stderr output and exit codes are identical byte-for-byte. When adding a new failure mode, extend `ErrorData`'s constructors rather than introducing a new error type — every surface depends on this single shape.

### Storage: turso + advisory-lock handoff

`nom-core/src/storage/` wraps `turso` (SQLite-compatible, local-file mode). Two invariants matter:

1. **WAL checkpoint on close** (`connection.rs`) — every connection checkpoints the WAL before releasing, preventing data loss when handing off between local-CLI and server processes.
2. **Advisory lock probe before open** (`lock_probe.rs`) — `Connection::open()` uses a non-blocking POSIX `fcntl(F_GETLK)` probe (the same mechanism turso uses internally) to detect whether another process already holds the DB open, and fails fast with `StorageError::Conflict("local_db_locked")` instead of risking silent WAL corruption from two writers. This is why the CLI error path has a dedicated friendly message: "server is running — stop it or use the remote-CLI instead."

Migrations (`migration.rs`) follow the geni pattern: raw SQL embedded via `include_str!("schema.sql")`, tracked by SHA-256 hash in an `_migrations` table, applied atomically in a transaction. There is currently one migration (v1, the full initial schema).

### Config, Clock

`nom-core/src/config.rs` — `AppConfig::load()` layers hardcoded defaults < TOML file (`$XDG_CONFIG_HOME/nom_mcp/config.toml`) < env vars (`NOM_MCP_` prefix, `__` for nested keys like `NOM_MCP_remote__server_url`). Secrets (USDA API key) are wrapped in `RedactedString`, which redacts Debug/Display but still serializes normally — don't unwrap that pattern when adding new secret fields.

`nom-core/src/clock.rs` — `Clock` resolves an IANA timezone once at startup (config → OS-local via `iana_time_zone` → UTC fallback) and is shared through the `OperationRegistry` so "today" is computed consistently and freshly (never cached) everywhere a logged date is materialized.

### Domain language

`CONTEXT.md` at the repo root is the canonical ubiquitous-language glossary (Meal, Food, Portion, Weight Entry, Goal, Direction, Weekly Summary, Widget Display) — read it before naming new types or fields, since it also lists terms to avoid (e.g. "Log Entry", "Serving" for the wrong thing, "Limit" instead of a Goal target + Direction).

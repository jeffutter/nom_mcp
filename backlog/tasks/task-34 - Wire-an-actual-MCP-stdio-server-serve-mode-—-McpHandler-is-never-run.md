---
id: TASK-34
title: Wire an actual MCP stdio server ('serve' mode) — McpHandler is never run
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-13 11:47'
updated_date: '2026-08-13 12:17'
labels:
  - review-followup
  - planned
dependencies:
  - TASK-2.17
priority: high
ordinal: 108
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while reviewing TASK-2.17. Across TASK-2.7 through TASK-2.17, nom-core built a complete MCP ServerHandler (nom-core/src/operation/mcp_handler.rs: get_info, list_tools, call_tool, list_resources, read_resource) plus an axum HTTP router (nom-core/src/operation/http_router.rs: build_http_router) — but neither is ever instantiated or run by any binary. grep -rn "McpHandler" and grep -rn "build_http_router" across the whole repo turn up zero call sites outside their own definitions, and grep -rn "ServiceExt|serve(|stdio(" (the rmcp APIs needed to actually run an MCP server) finds nothing anywhere in nom-mcp/src. nom-mcp/src/main.rs's main() unconditionally calls execute_from_args(), which only ever does one-shot local-CLI dispatch via cli_router::parse_and_dispatch — there is no 'serve' subcommand or equivalent. This contradicts the project's own spec: AGENTS.md explicitly describes the nom-mcp binary as '(local CLI + MCP server + HTTP server, registers all operations)', and doc-5 §... lists 'Server modes (HTTP/MCP serve)' as a first-class v1 deliverable with its own logging defaults. Practically: the nom://weekly-summary resource and get_widget_display/set_widget_display tools that TASK-2.17 just shipped cannot be reached by any real MCP client today — there is no running server for them to connect to. No existing backlog ticket (checked: only TASK-2.18 'Testing harness and coverage' remains not-started in the TASK-2.x sequence) covers this. Scope this ticket to the MCP stdio transport only (the part TASK-2.17's own resource work depends on being reachable); wiring the HTTP router into a 'serve' mode is a similarly-shaped but separate gap left for a follow-up ticket.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 nom-mcp/src/main.rs gains a 'serve' subcommand (invoked as e.g. `nom-mcp serve`) that builds the same OperationRegistry + Clock the local-CLI path builds, wraps it in an McpHandler, and runs it as a real MCP server over stdio using rmcp's ServiceExt::serve(rmcp::transport::stdio()) (or rmcp::serve_server), blocking until the client disconnects or the process is signaled to stop
- [x] #2 registry construction (the block of registry.register(...) calls currently inline in execute_from_args) is extracted into a shared fn build_registry(clock: Arc<Clock>) -> OperationRegistry so both the local-CLI path and the new serve path register the identical set of operations with no drift
- [x] #3 serve mode initializes tracing at info level by default (RUST_LOG-overridable), distinct from local-CLI's warn default, matching doc-5 §14's stated server-vs-CLI logging split; logs go to stderr (stdout is reserved for the MCP stdio protocol)
- [x] #4 a manual smoke test is documented in the ticket's implementation notes: run `nix develop -c cargo run -p nom-mcp --bin nom-mcp -- serve`, and from another terminal send a raw MCP initialize + resources/read (nom://weekly-summary) JSON-RPC request over the process's stdin, confirming a well-formed JSON-RPC response comes back on stdout
- [x] #5 nix develop -c cargo build --workspace passes
- [x] #6 nix develop -c cargo clippy --all-targets --all-features --workspace -- -D warnings passes
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
SETUP: Rust Cargo workspace (nom-core/ library, nom-mcp/ binaries), rmcp 2.2.0. Prefix all commands with 'nix develop -c'. Work from repo root.

RESEARCH FINDINGS (verified against current tree before writing this plan):
- nom_core::logging::init_server() ALREADY EXISTS (nom-core/src/logging.rs:25-32) — info default, RUST_LOG-overridable, writes to stderr. Do NOT add a new one; just call it from run_serve(). init_cli() (warn default) is what main() already calls for the CLI path — leave that untouched.
- rmcp 2.2.0 is currently a dependency of nom-core only (features = ["server", "client"]), not of nom-mcp directly. nom-mcp/Cargo.toml needs `rmcp = { version = "2.2.0", features = ["server"] }` added so main.rs can import `rmcp::ServiceExt` and `rmcp::transport::stdio`.
- Two distinct ErrorData types will be in scope in main.rs once this lands: `nom_core::error::ErrorData` (already imported, used by execute_from_args/cli_exit) and `rmcp::ErrorData` (returned by serve()/waiting() failure paths). Import the rmcp one aliased, e.g. `use rmcp::ErrorData as McpError`, to avoid collision/shadowing.
- rmcp::transport::io::stdio() -> (tokio::io::Stdin, tokio::io::Stdout); rmcp::ServiceExt::serve(self, transport) -> impl Future<Output = Result<RunningService<R, Self>, R::InitializeError>>; RunningService::waiting(self) -> Result<QuitReason, JoinError>. All re-exported at crate root (rmcp::{ServiceExt, transport}).
- nom_core::operation::mcp_handler::McpHandler::new(registry: OperationRegistry, clock: Clock) — takes registry by value, not Arc; construct after build_registry() returns.
- OperationRegistry::new(clock: Arc<Clock>) and .register(Arc<dyn Operation>) — signatures already match what build_registry() will need.
- nom-mcp/src/main.rs currently has no #[cfg(test)] mod; nom-mcp-remote.rs's test module is unrelated (HTTP client tests) and won't be affected by this change.

STEPS:
1. In nom-mcp/Cargo.toml, add `rmcp = { version = "2.2.0", features = ["server"] }` to [dependencies].

2. In nom-mcp/src/main.rs, extract the registration block (current lines ~56-84: from `let mut registry = OperationRegistry::new(clock.clone());` through the last `registry.register(...)`) into:
   ```
   fn build_registry(clock: Arc<Clock>, off_client: Arc<OffClient>, fdc_client: Option<Arc<FdcClient>>) -> OperationRegistry {
       let mut registry = OperationRegistry::new(clock.clone());
       registry.register(Arc::new(SearchFood::new(off_client, fdc_client)));
       ... (identical body, unchanged) ...
       registry
   }
   ```
   execute_from_args calls `build_registry(clock.clone(), off_client, fdc_client)` in place of the inline block; behavior must be byte-identical (same registration order).

3. In main(), before the existing `let args = ...; cli_exit(execute_from_args(&args));`, check `std::env::args().nth(1) == Some("serve".to_string())`. If so, initialize server-mode tracing and call a new `run_serve()`, converting any error to a process exit (print to stderr, exit(1)) rather than going through cli_exit (which is CLI-JSON-shaped). If not "serve", fall through to the existing unchanged local-CLI behavior exactly as today (do not disturb any other subcommand's parsing/dispatch).

4. Implement:
   ```
   fn run_serve() -> Result<(), Box<dyn std::error::Error>> {
       let _ = nom_core::logging::init_server();
       let config = AppConfig::load()?;
       let clock = Arc::new(Clock::new(&config)?);
       let off_client = Arc::new(OffClient::new("https://world.openfoodfacts.org", &config.off_user_agent)?);
       let fdc_client = config.usda_api_key.as_ref()
           .map(|key| FdcClient::new("https://api.nal.usda.gov/fdc", key.get()).map(Arc::new))
           .transpose()?;
       let registry = build_registry(clock.clone(), off_client, fdc_client);
       let handler = nom_core::operation::mcp_handler::McpHandler::new(registry, *clock);

       tokio::runtime::Runtime::new()?.block_on(async {
           use rmcp::ServiceExt;
           let service = handler.serve(rmcp::transport::stdio()).await?;
           service.waiting().await?;
           Ok::<_, Box<dyn std::error::Error>>(())
       })
   }
   ```
   Adjust error conversions as needed (nom_core::error::ErrorData, std::io::Error, and rmcp's error types all need to unify under whatever error type run_serve() returns — Box<dyn std::error::Error> is the simplest; a dedicated enum is fine too if it reads cleaner). main() prints the error and exits with a non-zero code on failure; it must not panic.

5. Manual smoke test (record exact commands + observed output in this ticket's Implementation Notes when done):
   - Terminal A: `nix develop -c cargo run -p nom-mcp --bin nom-mcp -- serve`
   - Terminal B: pipe a minimal MCP `initialize` request followed by a `resources/read` request for `nom://weekly-summary` into Terminal A's stdin as raw JSON-RPC lines (check rmcp 2.2.0's own examples/tests — e.g. under its source in the Nix store or crates.io — for the minimal request shape to copy), confirm well-formed JSON-RPC responses appear on stdout and diagnostic/log output appears on stderr (not stdout).

6. Verification: `nix develop -c cargo build --workspace`, `nix develop -c cargo clippy --all-targets --all-features --workspace -- -D warnings`, `nix develop -c cargo fmt --all --check`, `nix develop -c cargo test --workspace` (confirms build_registry extraction didn't break anything).

No sub-tickets: this is a single tightly-scoped, cohesive change (one file's dispatch logic + one Cargo.toml dependency addition); splitting it would create conjoined pieces that can't be verified independently.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implementation:
- nom-mcp/Cargo.toml: added rmcp = { version = "2.2.0", features = ["server", "transport-io"] } (transport-io needed for rmcp::transport::stdio(); nom-core's rmcp dep does not enable it).
- nom-mcp/src/main.rs: extracted build_registry(clock, off_client, fdc_client) -> OperationRegistry and build_clients(&config) -> Result<(Arc<OffClient>, Option<Arc<FdcClient>>), ErrorData> out of execute_from_args, byte-identical registration order preserved. Both execute_from_args (local-CLI) and the new run_serve() call these shared helpers, so both surfaces register the identical operation set with no drift.
- main() now checks std::env::args().nth(1) == Some("serve") before initializing CLI tracing; if so it calls run_serve() and exits, converting any error to eprintln! + process::exit(1) rather than going through cli_exit. All other subcommands fall through to the existing unchanged local-CLI path.
- run_serve() calls nom_core::logging::init_server() (info default, RUST_LOG-overridable, stderr) — distinct from init_cli()'s warn default, per doc-5 §14. Then loads config/clock/clients, builds the registry, wraps it in nom_core::operation::mcp_handler::McpHandler, and runs it via rmcp::ServiceExt::serve(rmcp::transport::stdio()).await? followed by service.waiting().await?, inside a fresh single-use tokio::runtime::Runtime (mirrors the existing execute_from_args pattern; workspace feature-unifies tokio's rt-multi-thread via nom-core so Runtime::new() works).

Manual smoke test (AC #4), run from repo root:
  export XDG_DATA_HOME=/tmp/nom_mcp_smoke/data XDG_CONFIG_HOME=/tmp/nom_mcp_smoke/config RUST_LOG=info
  nix develop -c cargo build -p nom-mcp --bin nom-mcp
  nix develop -c cargo run -p nom-mcp --bin nom-mcp -- serve
Drove it with a small python3 harness (piped stdin/stdout, not a real terminal) sending, as raw JSON-RPC lines:
  1) initialize (protocolVersion 2024-11-05, empty capabilities, clientInfo smoke-test-client)
  2) notifications/initialized
  3) resources/read {"uri":"nom://weekly-summary"}
Observed on stdout:
  initialize -> {"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2024-11-05","capabilities":{"resources":{},"tools":{}},"serverInfo":{"name":"nom-mcp","version":"0.1.0"}}}
  resources/read -> {"jsonrpc":"2.0","id":2,"result":{"contents":[{"uri":"nom://weekly-summary","mimeType":"application/json","text":"{\"start_date\":\"2026-08-07\",\"end_date\":\"2026-08-13\",\"days_with_data\":0,\"nutrients\":{...},\"weight\":{}}"}]}}
Both are well-formed single-line JSON-RPC responses on stdout; stderr carried only tracing INFO lines (service initialized as server / received notification / client initialized / input stream terminated / serve finished quit_reason=Closed) — confirming stdout is protocol-only and diagnostics go to stderr as required.

Verification: cargo build --workspace, cargo clippy --all-targets --all-features --workspace -- -D warnings, cargo fmt --all --check, and cargo test --workspace (224 nom-core tests + 7 nom-mcp-remote tests) all pass.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added an MCP stdio 'serve' subcommand to nom-mcp: main.rs now branches on argv[1]=='serve' before CLI tracing init, extracted build_registry()/build_clients() so the CLI and serve paths register identical operations, and run_serve() wires OperationRegistry+Clock into McpHandler and runs it via rmcp::ServiceExt::serve(rmcp::transport::stdio()) with info-level stderr logging. Verified with a manual JSON-RPC smoke test (initialize + resources/read nom://weekly-summary) and full build/clippy/fmt/test passes.
<!-- SECTION:FINAL_SUMMARY:END -->

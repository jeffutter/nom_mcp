---
id: TASK-34
title: Wire an actual MCP stdio server ('serve' mode) — McpHandler is never run
status: To Do
assignee: []
created_date: '2026-08-13 11:47'
labels:
  - review-followup
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
- [ ] #1 nom-mcp/src/main.rs gains a 'serve' subcommand (invoked as e.g. `nom-mcp serve`) that builds the same OperationRegistry + Clock the local-CLI path builds, wraps it in an McpHandler, and runs it as a real MCP server over stdio using rmcp's ServiceExt::serve(rmcp::transport::stdio()) (or rmcp::serve_server), blocking until the client disconnects or the process is signaled to stop
- [ ] #2 registry construction (the block of registry.register(...) calls currently inline in execute_from_args) is extracted into a shared fn build_registry(clock: Arc<Clock>) -> OperationRegistry so both the local-CLI path and the new serve path register the identical set of operations with no drift
- [ ] #3 serve mode initializes tracing at info level by default (RUST_LOG-overridable), distinct from local-CLI's warn default, matching doc-5 §14's stated server-vs-CLI logging split; logs go to stderr (stdout is reserved for the MCP stdio protocol)
- [ ] #4 a manual smoke test is documented in the ticket's implementation notes: run `nix develop -c cargo run -p nom-mcp --bin nom-mcp -- serve`, and from another terminal send a raw MCP initialize + resources/read (nom://weekly-summary) JSON-RPC request over the process's stdin, confirming a well-formed JSON-RPC response comes back on stdout
- [ ] #5 nix develop -c cargo build --workspace passes
- [ ] #6 nix develop -c cargo clippy --all-targets --all-features --workspace -- -D warnings passes
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
SETUP (read first): This is a Rust Cargo workspace (nom-core/ library, nom-mcp/ binaries) — a single-user MCP nutrition-tracking server, using rmcp 2.2.0. ALL commands must run inside the Nix dev shell: prefix every command with 'nix develop -c'. Work from the repository root unless told otherwise. Do not change pinned dependency versions.

1. Open nom-mcp/src/main.rs. The block from 'let mut registry = OperationRegistry::new(clock.clone());' through the last 'registry.register(...)' call (currently inline in execute_from_args, ~lines 56-84) should be extracted into a new function:
   fn build_registry(clock: Arc<Clock>, off_client: Arc<OffClient>, fdc_client: Option<Arc<FdcClient>>) -> OperationRegistry { ... same body ... }
   (adjust the signature to whatever inputs the registration calls actually need — off_client/fdc_client are only used by SearchFood::new(), the rest only need clock). execute_from_args calls this and keeps its existing behavior unchanged (verify via existing tests in nom-mcp/src/main.rs and nom-mcp/src/bin/nom-mcp-remote.rs still passing).

2. In main(), before the current 'let args = ...; cli_exit(execute_from_args(&args));' logic, check std::env::args() for a first argument equal to "serve". If present, dispatch to a new function run_serve() instead of execute_from_args(). If absent, keep the existing local-CLI behavior exactly as-is (do not change the CLI's arg parsing/behavior for any other subcommand).

3. Implement run_serve() -> Result<(), ErrorData> (or similar):
   - Initialize tracing for server mode at info default (check nom_core::logging for an existing init_server()/equivalent to init_cli(); if none exists, add one following the same pattern as init_cli() in nom-core/src/logging.rs, defaulting to info instead of warn, writing to stderr).
   - Load AppConfig, build Clock, off_client, fdc_client exactly as execute_from_args does.
   - Call build_registry(...) from step 1.
   - Construct nom_core::operation::mcp_handler::McpHandler::new(registry, *clock).
   - Build a tokio runtime (or reuse the existing pattern from execute_from_args's block_on) and run: the handler served over rmcp::transport::stdio() via rmcp::ServiceExt::serve(...), then .waiting().await to block until the client disconnects. Import ServiceExt and stdio from the rmcp crate (already a dependency of nom-core; add rmcp as a direct dependency of nom-mcp's Cargo.toml if it isn't already re-exported through nom-core).
   - Map any setup/serve errors into this binary's existing error-exit path (ErrorData + cli_exit, or a dedicated exit path — match whatever main()'s existing error handling looks like) rather than panicking.

4. Manually verify per the acceptance criteria's smoke test: run `nix develop -c cargo run -p nom-mcp --bin nom-mcp -- serve` in one terminal, and in another, pipe a minimal MCP initialize request followed by a resources/read request for "nom://weekly-summary" into its stdin (construct these as raw JSON-RPC lines per the MCP spec — check rmcp's own examples/tests directory in the vendored crate source, e.g. ~/.cargo/registry/src/*/rmcp-2.2.0/examples/, for a minimal client request shape to copy). Confirm well-formed JSON-RPC responses appear on stdout. Record the exact commands and observed output in this ticket's Implementation Notes.

5. Run: nix develop -c cargo build --workspace. Run: nix develop -c cargo clippy --all-targets --all-features --workspace -- -D warnings. Run: nix develop -c cargo fmt --all --check. Run: nix develop -c cargo test --workspace (confirm nothing in the existing suite broke from the build_registry extraction).
<!-- SECTION:PLAN:END -->

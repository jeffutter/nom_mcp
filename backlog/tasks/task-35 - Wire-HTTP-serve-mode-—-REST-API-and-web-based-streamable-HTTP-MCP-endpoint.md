---
id: TASK-35
title: Wire HTTP serve mode — REST API and web-based (streamable-HTTP) MCP endpoint
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-13 12:11'
updated_date: '2026-08-13 12:33'
labels:
  - planned
dependencies:
  - TASK-34
references:
  - >-
    ~/src/notectl (sibling project, reference implementation of serve stdio /
    serve http mounting both REST and streamable-HTTP MCP)
priority: high
type: feature
ordinal: 40000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
TASK-34 wires the MCP stdio transport but explicitly scopes out HTTP, leaving it "a similarly-shaped but separate gap left for a follow-up ticket." nom-core already has a working REST router (`build_http_router`, gated on `Surfaces::HTTP`) and a full MCP `ServerHandler` (`McpHandler`) — reachable via stdio once TASK-34 lands — but nothing exposes either one over HTTP today. `grep`-ing the repo confirms `build_http_router` has zero call sites outside its own definition, and no binary starts an HTTP listener.

The original architecture decision (TASK-1.6's Final Summary) and the v1 spec (doc-5 §3 "exposed identically over MCP, local CLI, HTTP, and a remote-CLI thin client"; doc-5 §14 "Server modes (HTTP/MCP serve)") describe nom-mcp's `serve` command family as covering stdio MCP + HTTP MCP + REST together — mirroring the sibling `notectl` project (`~/src/notectl`), whose `serve http` subcommand starts one axum server that nests rmcp's streamable-HTTP MCP service at `/mcp` alongside one REST route per operation on the same port, all backed by one shared operation registry.

Right now nom-mcp is unreachable by any HTTP-speaking client: REST clients (including `nom-mcp-remote`, the thin remote-CLI binary that currently has no live server to talk to) and any MCP client that connects over the network rather than spawning a local stdio subprocess (e.g. a hosted/remote MCP client). This ticket delivers that HTTP serve mode so nom-mcp is usable both as a REST API and as a network-reachable ("web-based") MCP server, sharing the same registry/tool logic as the stdio transport rather than duplicating it.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 nom-mcp gains an HTTP serve mode that starts a single, configurable-port HTTP listener
- [x] #2 Every operation registered with HTTP surface support (Surfaces::HTTP) is reachable as a REST endpoint through that listener
- [x] #3 The same listener also exposes a streamable-HTTP MCP endpoint that a standard MCP client can initialize against, list tools/resources on, and call tools through — including the nom://weekly-summary resource and the MCP-only widget-display tools already implemented for the stdio transport
- [x] #4 The REST and MCP-over-HTTP surfaces operate against the same shared registry/clock construction path as the stdio serve mode (TASK-34), so operation behavior never drifts between transports
- [x] #5 Server-mode logging defaults to info level, written to stderr, RUST_LOG-overridable — consistent with the stdio serve mode and doc-5 §14
- [x] #6 Manual verification demonstrates a REST call and a separate MCP-over-HTTP tool call both succeed against the same running instance
- [x] #7 cargo build --workspace, cargo clippy --all-targets --all-features --workspace -- -D warnings, cargo fmt --all --check, and cargo test --workspace all pass
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
SETUP: Rust Cargo workspace (nom-core/ library, nom-mcp/ binaries), rmcp 2.2.0, axum 0.8.9. Prefix all commands with 'nix develop -c'. Work from repo root.

RESEARCH FINDINGS (verified against current tree and against ~/src/notectl's `serve http` reference implementation before writing this plan):
- nom-core/src/operation/http_router.rs::build_http_router(registry: OperationRegistry) -> Router already exists and is fully correct (routes `/api/{op.name()}` for every Surfaces::HTTP operation via execute_json). grep -rn "build_http_router" confirms zero call sites outside its own definition/tests today — safe to change its signature.
- nom-core/src/operation/mcp_handler.rs::McpHandler already implements list_tools/call_tool/list_resources/read_resource (including nom://weekly-summary) via ServerHandler, reused as-is for stdio (TASK-34). No new MCP-protocol logic is needed for HTTP — only a second transport wrapping the same handler.
- nom-mcp/src/main.rs (post-TASK-34) already has build_registry(clock, off_client, fdc_client) -> OperationRegistry and build_clients(&config) -> Result<(Arc<OffClient>, Option<Arc<FdcClient>>), ErrorData> extracted and shared between local-CLI and stdio-serve. Both are reused unchanged for HTTP serve mode — the shared-construction-path requirement (AC #4) is already satisfied by calling these same two functions; this ticket does not touch their bodies.
- nom_core::logging::init_server() (info default, RUST_LOG-overridable, stderr) already exists and is what stdio-serve calls (TASK-34, AC #5's HTTP requirement is the same policy) — call it unchanged from the new HTTP path too, no new logging code needed.
- config::AppConfig already has `http_bind_address: String` (default "127.0.0.1", NOM_MCP_HTTP_BIND_ADDRESS-overridable, doc-5 §127's "binds 127.0.0.1 by default" requirement) but it is currently unused anywhere in the codebase — this ticket is what wires it in as the HTTP listener's bind IP. No config field exists yet for the port; add it as a CLI flag instead (see STEPS), mirroring notectl's `serve http --port` convention exactly since doc-5 has no stated default port.
- ~/src/notectl/src/main.rs (lines ~101-164) is the direct reference: builds `StreamableHttpService::new(factory_closure, Arc::new(LocalSessionManager::default()), StreamableHttpServerConfig::default().with_cancellation_token(ct.clone()))`, nests it at `/mcp` via `axum::Router::new().nest_service("/mcp", service)`, separately adds REST routes to the same Router, binds a `tokio::net::TcpListener`, and runs `axum::serve(listener, router.into_make_service()).with_graceful_shutdown(...)`. nom-mcp's version is simpler because `build_http_router()` already builds all REST routes generically from the registry (notectl loops registrations by hand; nom-mcp doesn't need to).
- rmcp 2.2.0 feature `transport-streamable-http-server` (not yet enabled in nom-mcp/Cargo.toml) provides `rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager}`. Verified feature name and re-export paths directly against the vendored crate source (~/.cargo/registry/…/rmcp-2.2.0/src/transport/streamable_http_server.rs and tower.rs).
- `StreamableHttpService::new(service_factory: impl Fn() -> Result<S, io::Error> + Send + Sync + 'static, session_manager: Arc<M>, config: StreamableHttpServerConfig)` calls `service_factory` once **per session**, not once total (verified in tower.rs: `spawn_session_worker` calls `self.get_service()` per new session). So the handler must be cheaply cloneable, not consumed once.
- **Registry/handler sharing refactor required** (this is the one non-trivial design decision in this ticket): today `build_http_router(registry: OperationRegistry)` and `McpHandler::new(registry: OperationRegistry, clock: Clock)` both *consume* an owned `OperationRegistry` and each wraps it in its own internal `Arc::new(...)`. HTTP serve mode needs ONE registry shared by both the REST router and every MCP-over-HTTP session. Fix: build the registry once in nom-mcp, wrap it in `Arc<OperationRegistry>` there, and change both nom-core entry points to accept the `Arc` directly instead of re-wrapping internally:
  - `http_router::build_http_router(registry: Arc<OperationRegistry>) -> Router` (drop the internal `Arc::new`; only self-contained test call sites affected — update `nom-core/src/operation/http_router.rs`'s two tests to pass `Arc::new(reg)`).
  - `McpHandler::new(registry: Arc<OperationRegistry>, clock: Clock) -> Self` (drop the internal `Arc::new`; update the 7 existing call sites inside `nom-core/src/operation/mcp_handler.rs`'s own test module to pass `Arc::new(reg)`, and nom-mcp's stdio `run_serve_stdio()` call site — no behavior change, purely a wrapping-location move).
  - Add `#[derive(Clone)]` to `McpHandler` — trivially valid since its fields are `Arc<OperationRegistry>` (Clone), `Clock` (already `Copy`/`Clone`), and `#[cfg(test)] db_path: Option<PathBuf>` (Clone). This is what lets the per-session `service_factory` closure do `{ let handler = handler.clone(); move || Ok(handler.clone()) }` — every session shares the same underlying registry Arc, no per-session rebuild.
- Clock is `#[derive(Clone, Copy, Debug)]` (nom-core/src/clock.rs) — confirmed safe to copy freely, already relied on by TASK-34's `*clock` dereferences.
- nom-mcp/src/main.rs currently branches on `std::env::args().nth(1) == Some("serve")` with NO subcommand parsing beneath it (bare `nom-mcp serve` = stdio, per TASK-34). This ticket extends that same hand-rolled style (no clap involved for `serve`, consistent with TASK-34's explicit decision not to route `serve` through the dynamic CLI-operation clap tree) rather than introducing clap here: `nom-mcp serve` or `nom-mcp serve stdio` stays stdio (backward compatible with TASK-34's shipped behavior/docs); `nom-mcp serve http [--port N]` (default port 8000, matching notectl's convention since doc-5 states no specific value) is new.
- nom-mcp-remote (`nom-mcp/src/bin/nom-mcp-remote.rs`) already POSTs to `{server_url}/api/{operation}` — confirmed byte-compatible with `build_http_router`'s `/api/{op.name()}` routes, so no remote-CLI changes are needed; it will "just work" against the new HTTP serve mode once it's up (can be used as part of manual verification, AC #6, instead of raw curl).
- Cargo dependency additions needed only in nom-mcp/Cargo.toml (nom-core already has axum 0.8.9 as a dependency for building the Router type, but doesn't run a server):
  - `axum = "0.8.9"` (match nom-core's pinned version, confirmed compatible with rmcp 2.2's `server-side-http`/`transport-streamable-http-server` features — both resolve to `http` 1.5.0 per Cargo.lock)
  - `tokio-util = "0.7"` (for `CancellationToken`, matches notectl's version)
  - `rmcp` features gain `"transport-streamable-http-server"` alongside the existing `"server"`, `"transport-io"`
  - `tokio` features gain `"net"` (TcpListener) and `"signal"` (ctrl_c); `"rt"` already present and — per TASK-34's own notes — workspace feature-unification with nom-core's `rt-multi-thread` already makes plain `tokio::runtime::Runtime::new()` work, so no explicit `rt-multi-thread` needed in nom-mcp's own Cargo.toml.

STEPS:
1. nom-core/src/operation/http_router.rs: change `pub fn build_http_router(registry: super::OperationRegistry) -> Router` to `pub fn build_http_router(registry: Arc<super::OperationRegistry>) -> Router`, removing the internal `let registry = Arc::new(registry);` line (the parameter itself is now already the Arc). Update the module's two tests (`test_build_http_router_has_routes`, and the setup path for `test_handle_operation_error_serializes_error_data_body` if it constructs a registry — check both) to pass `Arc::new(reg)` instead of `reg`.

2. nom-core/src/operation/mcp_handler.rs:
   a. Change `McpHandler::new(registry: OperationRegistry, clock: Clock) -> Self` to `McpHandler::new(registry: Arc<OperationRegistry>, clock: Clock) -> Self`, removing the internal `Arc::new(registry)` wrap (store the passed-in Arc directly).
   b. Add `#[derive(Clone)]` to the `McpHandler` struct definition.
   c. Update all 7 in-module test call sites (`McpHandler::new(reg, clock)` / `McpHandler::new(OperationRegistry::new(make_clock()), clock)`) to wrap their registry argument in `Arc::new(...)`.

3. nom-mcp/Cargo.toml: add `axum = "0.8.9"` and `tokio-util = "0.7"` to `[dependencies]`; change the `rmcp` line's features to `["server", "transport-io", "transport-streamable-http-server"]`; change the `tokio` line's features to `["rt", "net", "signal"]`.

4. nom-mcp/src/main.rs:
   a. Rename the existing `run_serve()` to `run_serve_stdio()` (pure rename, no behavior change) and update its single `McpHandler::new(registry, *clock)` call site to `McpHandler::new(Arc::new(registry), *clock)`.
   b. Replace the current `if std::env::args().nth(1).as_deref() == Some("serve") { ... }` block in `main()` with dispatch to a new small pure function:
      ```rust
      enum ServeMode {
          Stdio,
          Http { port: u16 },
          Unknown(String),
      }

      /// Parse `serve [stdio|http [--port N]]` from raw argv (args[0] is the
      /// binary name, args[1] is "serve"). Bare `serve` and `serve stdio` are
      /// equivalent (TASK-34 backward compatibility). Default HTTP port is
      /// 8000 (matches notectl's `serve http` convention; doc-5 states no
      /// specific default).
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
      ```
      Then in `main()`:
      ```rust
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
      ```
   c. Add `run_serve_http(port: u16) -> Result<(), Box<dyn std::error::Error>>`:
      ```rust
      fn run_serve_http(port: u16) -> Result<(), Box<dyn std::error::Error>> {
          let _ = nom_core::logging::init_server();

          let config = AppConfig::load()?;
          let clock = Arc::new(Clock::new(&config)?);
          let (off_client, fdc_client) = build_clients(&config)?;
          let registry = Arc::new(build_registry(clock.clone(), off_client, fdc_client));
          let handler = nom_core::operation::mcp_handler::McpHandler::new(registry.clone(), *clock);
          let bind_address = config.http_bind_address.clone();

          tokio::runtime::Runtime::new()?.block_on(async {
              use rmcp::transport::streamable_http_server::{
                  StreamableHttpServerConfig, StreamableHttpService,
                  session::local::LocalSessionManager,
              };
              use tokio_util::sync::CancellationToken;

              let ct = CancellationToken::new();
              let mcp_config =
                  StreamableHttpServerConfig::default().with_cancellation_token(ct.clone());
              let mcp_service = StreamableHttpService::new(
                  move || Ok(handler.clone()),
                  Arc::new(LocalSessionManager::default()),
                  mcp_config,
              );

              let router = nom_core::operation::http_router::build_http_router(registry)
                  .nest_service("/mcp", mcp_service);

              let addr = format!("{bind_address}:{port}");
              let listener = tokio::net::TcpListener::bind(&addr).await?;
              tracing::info!(%addr, "nom-mcp HTTP serve mode listening (REST at /api/*, MCP at /mcp)");

              axum::serve(listener, router.into_make_service())
                  .with_graceful_shutdown(async move {
                      let _ = tokio::signal::ctrl_c().await;
                      ct.cancel();
                  })
                  .await?;

              Ok::<_, Box<dyn std::error::Error>>(())
          })
      }
      ```
   d. Add a `#[cfg(test)] mod tests` for `parse_serve_mode` covering: bare `["nom-mcp", "serve"]` → Stdio, `["nom-mcp", "serve", "stdio"]` → Stdio, `["nom-mcp", "serve", "http"]` → Http{port:8000}, `["nom-mcp", "serve", "http", "--port", "9999"]` → Http{port:9999}, `["nom-mcp", "serve", "bogus"]` → Unknown("bogus".into()). Pure function, no I/O, no tokio needed.

5. Manual smoke test (record exact commands + observed output in this ticket's Implementation Notes when done, mirroring TASK-34's style):
   - Terminal A: `XDG_DATA_HOME=/tmp/nom_mcp_http_smoke/data XDG_CONFIG_HOME=/tmp/nom_mcp_http_smoke/config RUST_LOG=info nix develop -c cargo run -p nom-mcp --bin nom-mcp -- serve http --port 8000`
   - Terminal B, REST call (AC #2/#6): `curl -s -X POST http://127.0.0.1:8000/api/get_weight_today -H 'content-type: application/json' -d '{}'` (or another zero-required-arg operation) — confirm a 200 with well-formed JSON body; alternatively drive it through `nom-mcp-remote` after setting `NOM_MCP_REMOTE_SERVER_URL=http://127.0.0.1:8000` to exercise the real remote-CLI client end-to-end.
   - Terminal B, MCP-over-HTTP call (AC #3/#6): use a small script (python3 + `requests`, or rmcp's own client if quicker) to POST an `initialize` request to `http://127.0.0.1:8000/mcp`, then `tools/list` and one `tools/call`, and a `resources/read` for `nom://weekly-summary` — confirm well-formed JSON-RPC responses. Record the exact requests/responses observed.
   - Confirm both calls succeed against the *same* running server process (single terminal A instance, not restarted between the two checks) — this is what AC #6 requires.
   - Confirm stderr carries only tracing INFO lines and stdout carries only HTTP responses (no protocol confusion now that stdout isn't reserved the way it is for stdio mode — still worth a glance to confirm no stray prints).

6. Verification: `nix develop -c cargo build --workspace`, `nix develop -c cargo clippy --all-targets --all-features --workspace -- -D warnings`, `nix develop -c cargo fmt --all --check`, `nix develop -c cargo test --workspace` (confirms the http_router.rs/mcp_handler.rs signature changes and their updated tests, plus the new parse_serve_mode tests, all pass alongside the existing 224+7 tests).

No sub-tickets: this is one cohesive change with a single tightly-coupled design decision (Arc-sharing the registry between the REST router and every MCP-over-HTTP session) threaded through two nom-core files and one nom-mcp binary file plus its Cargo.toml — the same shape and size as TASK-34, which also shipped as a single ticket. Splitting the nom-core signature refactor from the nom-mcp wiring that depends on it would create two tickets that can't be independently verified (the refactor has no purpose without the caller that needs shared ownership, and the caller can't compile without the refactor).
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implementation followed the ticket's own plan exactly, matching the notectl reference implementation (~/src/notectl/src/main.rs ~lines 101-164):

1. nom-core/src/operation/http_router.rs: build_http_router(registry: super::OperationRegistry) -> Router changed to accept Arc<super::OperationRegistry> directly, removing the internal Arc::new wrap. Updated test_build_http_router_has_routes to pass Arc::new(reg). (test_handle_operation_error_serializes_error_data_body calls handle_operation directly and never constructed a registry, so it needed no change.)

2. nom-core/src/operation/mcp_handler.rs: McpHandler::new(registry: OperationRegistry, ...) changed to McpHandler::new(registry: Arc<OperationRegistry>, ...), storing the passed-in Arc directly. Added #[derive(Clone)] to McpHandler (all fields — Arc<OperationRegistry>, Clock, #[cfg(test)] Option<PathBuf> — are Clone). Updated all 7 in-module test call sites to wrap their registry in Arc::new(...).

3. nom-mcp/Cargo.toml: added axum = "0.8.9", tokio-util = "0.7", tracing = "0.1" (needed for the new tracing::info! call in run_serve_http); rmcp features gained "transport-streamable-http-server"; tokio features gained "net" and "signal".

4. nom-mcp/src/main.rs:
   - Renamed run_serve() to run_serve_stdio(), updated its McpHandler::new call site to wrap the registry in Arc::new(...).
   - Added ServeMode enum (Stdio / Http{port} / Unknown(String)) and parse_serve_mode(args) parsing `serve [stdio|http [--port N]]` from raw argv — bare `serve`/`serve stdio` stay stdio (TASK-34 backward compat), `serve http` defaults to port 8000.
   - Added run_serve_http(port): builds registry/clock/clients via the same build_registry/build_clients functions as stdio serve (AC #4), wraps the registry in one shared Arc, constructs McpHandler::new(registry.clone(), *clock), builds a StreamableHttpService (rmcp::transport::streamable_http_server, LocalSessionManager, per-session service_factory closure cloning the shared handler), nests it at /mcp on the same axum Router returned by build_http_router(registry) (REST at /api/*), binds config.http_bind_address:port via tokio::net::TcpListener, and serves with graceful shutdown on ctrl_c. Logging uses nom_core::logging::init_server() (info default, stderr, RUST_LOG-overridable) — same as stdio serve.
   - Added #[cfg(test)] mod tests for parse_serve_mode covering bare serve, explicit stdio, http default port, http explicit port, and unknown mode.

Verification:
- cargo build --workspace: clean.
- cargo clippy --all-targets --all-features --workspace -- -D warnings: clean.
- cargo fmt --all --check: clean (after running cargo fmt --all once to fix one multi-line assert_eq wrap).
- cargo test --workspace: 224 nom-core unit tests + 1 lock_probe integration test + 5 new parse_serve_mode tests + 7 nom-mcp-remote tests all pass (237 total, 0 failed).

Manual smoke test (AC #6), single running `nom-mcp serve http --port 8123` instance, isolated XDG dirs:
- REST: `curl -X POST http://127.0.0.1:8123/api/get_weight_today -d '{}'` -> HTTP 200, body `[]`.
- MCP-over-HTTP against the same running instance: POSTed initialize (got session id + protocolVersion/serverInfo), notifications/initialized (202), tools/list (returned all 17 registered tools including get_widget_display/set_widget_display), tools/call get_weight_today (isError:false, text "[]"), resources/read nom://weekly-summary (returned well-formed weekly summary JSON with start_date/end_date/nutrients/weight).
- Server stderr carried only tracing INFO lines (listening banner, session-created, service-initialized, notification-received, client-initialized) for the whole run — no stray stdout prints, no protocol confusion.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added nom-mcp serve http mode: a single axum listener exposing every Surfaces::HTTP operation as a REST endpoint at /api/{name} and a streamable-HTTP MCP endpoint at /mcp (tools, resources including nom://weekly-summary, and the MCP-only widget-display tools), both backed by one shared Arc<OperationRegistry>/Clock built via the same build_registry/build_clients path as stdio serve mode. Required making build_http_router and McpHandler::new accept a pre-wrapped Arc<OperationRegistry> instead of each wrapping their own, and making McpHandler Clone so each MCP session gets a cheap clone of the same shared handler. serve/serve stdio remain unchanged (TASK-34 compat); serve http [--port N] is new (default 8000). Verified with a live REST call and a live MCP tools/list+tools/call+resources/read sequence against the same running server instance; full workspace build/clippy/fmt/test suite passes (237 tests).
<!-- SECTION:FINAL_SUMMARY:END -->

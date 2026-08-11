---
id: doc-1
title: 'Research: rmcp crate capabilities for a multi-transport Operation pattern'
type: other
created_date: '2026-08-11 04:44'
updated_date: '2026-08-11 04:45'
---
# Research: rmcp crate capabilities for a multi-transport Operation pattern

Primary sources consulted: [crates.io/crates/rmcp](https://crates.io/crates/rmcp) (crates.io API), [docs.rs/rmcp](https://docs.rs/rmcp/latest/rmcp/), [github.com/modelcontextprotocol/rust-sdk](https://github.com/modelcontextprotocol/rust-sdk) (README, on `main`), and the notectl source itself via `gh api repos/jeffutter/notectl/contents/...` (files: `notectl-core/src/operation.rs`, `src/main.rs`, `src/mcp.rs`, `src/capabilities/mod.rs`, `src/http_router.rs`, `src/cli_router.rs`, `Cargo.toml`, `notectl-core/Cargo.toml`, `notectl-core/src/config.rs`).

## 1. Current version and feature flags

- **rmcp is on 3.1.2** as of 2026-08-07 (`max_stable_version` / `default_version` / `newest_version` all report `3.1.2` — [crates.io API](https://crates.io/api/v1/crates/rmcp)). Release cadence is fast: `2.2.0` (2026-07-08) → `3.0.0-beta.1..5` (2026-07-23–28) → `3.0.0` (2026-07-28) → `3.0.1` → `3.1.0` (2026-07-31) → `3.1.1` (2026-08-05) → `3.1.2` (2026-08-07). notectl pins `rmcp = "2.2"` (its `Cargo.toml`, workspace deps), which is now **one major version behind**.
- The crate is maintained under the official repo `github.com/modelcontextprotocol/rust-sdk` (`repository` field on crates.io) — this is the canonical/official Rust MCP SDK, containing two crates: `rmcp` (protocol/runtime) and `rmcp-macros` (the `#[tool]`/`#[prompt]` proc-macros).
- Feature flags notectl declares — `server`, `transport-io`, `transport-streamable-http-server` — **all three still exist verbatim in 3.1.2** (confirmed via docs.rs features page for `rmcp@3.1.2`, which lists 28 features total including these three, plus `default = base64 + macros + server`).
- **Gotcha found:** in 3.1.2, `LocalSessionManager` (the type notectl uses for the streamable-HTTP server, `rmcp::transport::streamable_http_server::session::local::LocalSessionManager`) is gated behind a *separate* feature, `transport-streamable-http-server-session` (plus `client` or `server`) — see [docs.rs LocalSessionManager](https://docs.rs/rmcp/3.1.2/rmcp/transport/streamable_http_server/session/local/struct.LocalSessionManager.html). notectl's `Cargo.toml` does **not** list this feature explicitly, only `transport-streamable-http-server`. Either (a) `transport-streamable-http-server-session` was pulled in transitively by `transport-streamable-http-server` back on 2.2 and the split happened later in the 3.x line, or (b) notectl's feature list is already incomplete and only compiles because some other enabled feature drags it in transitively. **Action for nom_mcp:** when pinning rmcp, explicitly add `transport-streamable-http-server-session` if using `LocalSessionManager`, and re-verify feature names against whatever the then-current rmcp minor is — this crate's feature surface has clearly been reshuffled between 2.x and 3.x (see `StreamableHttpServerConfig` note below).
- Related config-shape drift: current README (`main` branch) shows `StreamableHttpServerConfig` built with `.with_legacy_session_mode(false)` / `.with_json_response(true)`, whereas notectl's `main.rs` calls `.with_cancellation_token(ct.clone())`. The changelog (fetched from `crates/rmcp/CHANGELOG.md` on the rust-sdk repo) notes `StreamableHttpServerConfig::stateful_mode` was renamed to `legacy_session_mode` "to clarify it only affects legacy protocol versions," and that SSE transport support was removed entirely back in `0.11.0`. This confirms the streamable-HTTP-server config struct's field set is actively being renamed/reshaped across releases — don't copy notectl's exact builder-method calls without checking the target rmcp version's docs.rs page for that struct.

## 2. How MCP tools are defined

Macro-based, via the `rmcp-macros` crate re-exported through `rmcp`: `#[tool]`, `#[tool_router]`, `#[tool_handler]` (and the prompt equivalents `#[prompt]`/`#[prompt_router]`/`#[prompt_handler]`). Confirmed on both the crate's own docs (`docs.rs/rmcp/latest` module summary: "handler — Client and server request handling... macros feature: `#[tool]` and `#[tool_router]`... `#[tool_handler]`") and the rust-sdk README.

Canonical pattern, matching notectl's own `src/mcp.rs` exactly:

```rust
#[tool_router]
impl TaskSearchService {
    #[tool(description = "Search for tasks in Markdown files with optional filtering by status, dates, and tags")]
    async fn search_tasks(&self, Parameters(request): Parameters<SearchTasksRequest>)
        -> Result<Json<TaskSearchResponse>, ErrorData> { ... }
}

#[tool_handler(router = self.tool_router.clone())]
impl ServerHandler for TaskSearchService {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_instructions(instructions)
    }
}
```

`Parameters<T>` and `Json<T>` (from `rmcp::handler::server::wrapper`) are wrapper types the macro uses to pull the request out of the JSON-RPC `arguments` and wrap the response back into an MCP `CallToolResult`.

**JSON schema generation is schemars-based.** The `schemars` Cargo feature is what pulls in the `schemars` dependency; the `server` feature enables it. Tool parameter structs derive `schemars::JsonSchema` and the macro calls `schema_for!`/equivalent under the hood to build the `inputSchema` advertised in `tools/list`. This is exactly the same tool notectl-core's `Operation::input_schema()` doc-comment says to use ("Implementations should use `schemars::schema_for!` on their request type" — `notectl-core/src/operation.rs`), so the same schema derivation serves rmcp's tool schema *and* notectl's own HTTP/CLI operation schema. This is the crux of why the shared-`Operation`-trait pattern works: one `#[derive(JsonSchema)]` request struct feeds both rmcp's tool registration and the generic `input_schema()` used for HTTP/CLI introspection.

**Escape hatch for tools that don't fit the macro:** rmcp also exposes a lower-level manual API — `ToolRouter::with_async_tool::<T>()` plus a hand-written `impl ToolBase for T` (`name()`, `description()`, associated `Parameter`/`Output`/`Error` types) and `impl AsyncTool<Service> for T` (`invoke()`). notectl uses this in `src/mcp.rs` for its `search` and `build_search_index` tools ("Trait-based search tools (added manually to the router, not via `#[tool]`)"), apparently because those two tools' MCP-facing parameter struct (`McpSearchParams`) diverges slightly from the shared HTTP/CLI request struct (extra fields like `no_reindex`, different defaults) — a concrete example of the "one Operation, one schema" ideal not being airtight in practice; sometimes the MCP surface needs its own params type and the router is built by hand instead of purely through the macro.

## 3. How MCP Resources are defined

**No macro exists for Resources** — no `#[resource]`/`#[resource_router]` counterpart to `#[tool]`/`#[tool_router]`. Confirmed by inspecting `docs.rs/rmcp/3.1.2/rmcp/handler/server/` module listing, which only shows `common`, `prompt`, `router`, `tool`, `tool_name_validation`, `wrapper` — nothing resource-specific.

Resources are implemented by hand on `ServerHandler`:

```rust
impl ServerHandler for MyServer {
    async fn list_resources(&self, _req: Option<PaginatedRequestParams>, _ctx: RequestContext<RoleServer>)
        -> Result<ListResourcesResult, McpError> {
        Ok(ListResourcesResult { resources: vec![Resource::new("file:///config.json", "config")], next_cursor: None, meta: None })
    }

    async fn read_resource(&self, request: ReadResourceRequestParams, _ctx: RequestContext<RoleServer>)
        -> Result<ReadResourceResult, McpError> {
        match request.uri.as_str() {
            "file:///config.json" => Ok(ReadResourceResult::new(vec![ResourceContents::text(r#"{"key":"value"}"#, &request.uri)])),
            _ => Err(McpError::resource_not_found(/* ... */)),
        }
    }
}
```

and the capability must be declared explicitly: `ServerCapabilities::builder().enable_resources().build()` (parallel to notectl's `enable_tools()` call). Source: [rust-sdk README, `main` branch](https://github.com/modelcontextprotocol/rust-sdk).

**Implication for nom_mcp's "weekly-summary" Resource:** this will not fit the `#[tool]`/`Operation` pattern at all — it's a different `ServerHandler` capability with a different pair of methods (`list_resources`/`read_resource`), addressed by URI rather than by tool name/JSON-RPC arguments. If nom_mcp wants a shared "Operation"-style abstraction to also cover Resources, that abstraction has to be invented in nom_mcp's own core (the way `notectl-core::operation::Operation` was invented for tools/HTTP/CLI) — rmcp gives no ready-made generalization across tools and resources. A resource can still be implemented as a thin adapter that calls into an existing `Operation` (e.g. `read_resource` for `weekly-summary` calls `WeeklySummaryOperation::execute_json(json!({}))` and reformats the result as `ResourceContents::text`), but that's userland glue, not something rmcp provides.

## 4. Transport support: stdio vs streamable-http-server

**stdio** — feature `transport-io`. Setup is a one-liner: `MyServer.serve(rmcp::transport::stdio()).await?`, returning a `RunningService` you `.waiting()` on (notectl's `main.rs` does exactly this, racing `service.waiting()` against `tokio::signal::ctrl_c()`). No session manager needed — each stdio process is implicitly a single session tied to the process's lifetime; this is the natural fit for a local CLI-invoked MCP server (one client, one process).

**streamable-http-server** — feature `transport-streamable-http-server` (+ `transport-streamable-http-server-session` for `LocalSessionManager`, see §1). This transport is an axum `Service` you `nest_service` into a router:

```rust
let service = StreamableHttpService::new(
    move || Ok(TaskSearchService::new(base_path_clone.clone())),
    Arc::new(LocalSessionManager::default()),
    config,
);
let router = axum::Router::new().nest_service("/mcp", service);
```

Two things this requires that stdio doesn't:
- **A factory closure**, not a single instance — `StreamableHttpService::new` takes `impl Fn() -> Result<Service, _>`, because a fresh service instance is constructed per session (matches notectl's `move || Ok(TaskSearchService::new(...))`).
- **An explicit `SessionManager`** — `LocalSessionManager` is the in-memory, single-process implementation (a `RwLock`-backed `HashMap` of session state per docs.rs). It exists because streamable-HTTP is stateful across multiple requests (unlike stdio's implicit single session) and multiple HTTP clients can be talking to the same server process concurrently; something has to track session IDs, initialization state, and message routing per session. `LocalSessionManager` is the built-in single-node choice; presumably a distributed/shared implementation is possible for multi-node deployments (not confirmed from source; not needed for nom_mcp's likely single-instance deployment) — flagged as unverified.
- Server can be cleanly shut down via a `tokio_util::sync::CancellationToken` passed to `StreamableHttpServerConfig` (though note §1's caveat that this exact builder call may have moved/renamed on the current version).

Both transports run the *same* `ServerHandler` impl — the transport is fully decoupled from tool/resource definitions, which is what makes bolting a second transport onto an existing rmcp server cheap.

## 5. "MCP-only" extras with no HTTP/CLI equivalent

**rmcp has no first-class concept of this at all — and it doesn't need one.** The `Operation` trait, the CLI router, and the HTTP router are 100% notectl-core/notectl-application inventions (`notectl-core/src/operation.rs`, `src/cli_router.rs`, `src/http_router.rs`); rmcp only knows about whatever gets registered on its `ToolRouter`/`ServerHandler`. Concretely, in `src/capabilities/mod.rs`, `CapabilityRegistry::create_operations()` returns the `Vec<Arc<dyn Operation>>` that feeds *both* `cli_router::build_cli` and `http_router::register_operation` — but `src/mcp.rs`'s `#[tool_router]` block is a **separate, independently-maintained list** of `#[tool]` methods on `TaskSearchService`. A tool defined with `#[tool]` that is never wrapped in an `Operation` impl and never added to `create_operations()` is exactly an "MCP-only" tool: it shows up in `tools/list` and is callable over stdio/HTTP-MCP, but has no CLI subcommand and no REST route, because nothing ever calls `get_command()`/`path()` for it.

**Implication for nom_mcp's widget-toggle tools:** implement them as plain `#[tool]` methods on the MCP `ServerHandler` (or via manual `ToolBase`/`AsyncTool` if their params need to diverge from any shared type), and simply never implement `Operation` for them / never register them in whatever `create_operations()`-equivalent list feeds the CLI and HTTP routers. This is a *convention* enforced by which list you push to, not a distinct rmcp feature — the same trick notectl already uses for `search`/`build_search_index` (though those two *are* still registered as Operations too; the widget-toggle case would be the "not registered anywhere else" version of that same technique).

## 6. Other gotchas in notectl's actual code

- **`Config::default()` vs `Config::load_from_base_path`:** `src/main.rs` builds a throwaway `CapabilityRegistry` with `Config::default()` and `PathBuf::from(".")` *before* parsing CLI args, purely to call `create_operations()` and get each operation's `get_command()` for `clap::Command` assembly (`cli_router::build_cli`) — clap needs the full subcommand tree built before it can parse anything, but the *real* base path/config isn't known until args are parsed. Once the `serve`/HTTP-mode branch has a parsed `base_path`, it builds a second, real `CapabilityRegistry` via `Config::load_from_base_path(&base_path)`. This is a two-phase bootstrap forced by clap's "build the whole command tree up front" model colliding with "config depends on a CLI argument." Any nom_mcp CLI built the same way (dynamic subcommands generated from Operation impls) will hit the same chicken-and-egg problem and likely need the same two-pass Config::default-then-reload pattern, unless the CLI's operation list doesn't need per-instance config to produce its `clap::Command` (i.e., static config all the way).
- **`execute_json` type erasure and rmcp's tool-calling convention:** `Operation::execute_json(&self, json: serde_json::Value) -> Result<serde_json::Value, ErrorData>` is notectl-core's own generic dispatch method for HTTP/CLI-via-remote reuse — it is *not* what rmcp calls when a tool is invoked over MCP. rmcp's `#[tool]`-generated dispatch instead goes through the typed `Parameters<Req>`/`Json<Resp>` wrappers and calls the concrely-typed method directly (e.g. `search_tasks(&self, Parameters<SearchTasksRequest>)`), which itself just delegates to the same capability method that `execute_json` would call. In other words: the MCP path and the `execute_json` path are two independently-typed call paths into the same underlying capability logic — they don't compose (rmcp doesn't call through `execute_json`, and `execute_json` doesn't route through rmcp's `ToolRouter`). Both exist because rmcp's macro wants concrete types for its schema/dispatch, while `Operation::execute_json` wants an erased `Value` for the generic CLI/HTTP router to call without knowing the concrete request type at compile time. Anything added to the shared `Operation` trait has to be manually kept in sync with an equivalent `#[tool]` method (or vice versa) — there's no single point where "add operation → tool automatically exists" happens; notectl's own `search`/`build_search_index` tools show this can drift (separate `McpSearchParams` vs the HTTP/CLI request type).
- **Feature/version drift risk:** given the 2.2 → 3.1.2 major-version jump in a few weeks (§1) and the observed renames (`stateful_mode`→`legacy_session_mode`, session-manager feature split, SSE removal in 0.11), rmcp is evolving quickly. Any nom_mcp implementation should pin a specific version, not `"2"` or a loose range, and expect to re-verify builder-method names and feature flags against docs.rs for that exact pinned version at implementation time rather than trusting notectl's snapshot.

## Summary answer

rmcp (3.1.2, official `modelcontextprotocol/rust-sdk` crate) is a solid fit for the tool + transport half of a notectl-style `Operation` pattern: `#[tool]`/`#[tool_router]`/`#[tool_handler]` macros generate MCP tool registration and dispatch from schemars-derived request structs — the same schema notectl-core's `Operation::input_schema()` doc-comment says to reuse for HTTP/CLI — and stdio (`transport-io`) and streamable-HTTP (`transport-streamable-http-server` + `transport-streamable-http-server-session` for `LocalSessionManager`) are both thin, swappable transports over one `ServerHandler` impl. It has no equivalent concept for Resources (no macro; hand-written `list_resources`/`read_resource` on `ServerHandler`) and no concept of "MCP-only" tools — both of those, and the entire CLI/HTTP-vs-MCP unification, are 100% invented in notectl-core (the `Operation` trait plus the two independent registration lists in `main.rs`/`mcp.rs`), and an "MCP-only" tool is achieved simply by defining a `#[tool]` and never wiring it into the CLI/HTTP operation list. The biggest risk for nom_mcp isn't fit, it's version drift: rmcp jumped 2.2→3.1.2 in about a month with real breaking renames, so any version pinned today should be re-verified against docs.rs at implementation time rather than assumed stable.

//! MCP ServerHandler — hand-written list_tools/call_tool that loops the
//! OperationRegistry directly.
//!
//! Deliberately NOT using rmcp's `#[tool]` macro/ToolRouter because ToolBase
//! requires compile-time-associated-function types incompatible with
//! `Vec<Arc<dyn Operation>>`.

use std::sync::Arc;

use rmcp::{
    ErrorData, RoleServer,
    handler::server::ServerHandler,
    model::{
        CacheScope, CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock,
        ErrorCode, Implementation, InitializeResult, ListResourcesResult, ListToolsResult,
        PaginatedRequestParams, ReadResourceResponse, ReadResourceResult, Resource,
        ResourceContents, ServerCapabilities, Tool,
    },
    service::RequestContext,
};

use super::{Operation, OperationRegistry, Surfaces};
use crate::clock::Clock;
use crate::storage::Connection;

/// URI of the MCP Apps UI resource for `get_goal_progress`'s widget.
///
/// Not listed in `build_resources()`/`list_resources()` — per the MCP Apps
/// spec, UI-only resources are discovered via the tool's own
/// `_meta.ui.resourceUri` (see `goal_progress_ui_meta`), not the general
/// resource listing.
const GOAL_PROGRESS_UI_RESOURCE_URI: &str = "ui://nom-mcp/goal-progress";

/// Static widget HTML served for [`GOAL_PROGRESS_UI_RESOURCE_URI`].
///
/// Self-contained (inline CSS/JS, no external requests) because the MCP Apps
/// spec's restrictive default CSP — applied whenever a UI resource declares
/// no `csp` domains, which this one deliberately doesn't — blocks anything
/// else (`connect-src 'none'`, `script-src 'self' 'unsafe-inline'`).
const GOAL_PROGRESS_WIDGET_HTML: &str = include_str!("../../assets/goal_progress_widget.html");

/// Cache TTLs (SEP-2549: `ReadResourceResult`/`ListToolsResult`/
/// `ListResourcesResult` all carry a required `ttlMs`/`cacheScope` pair once
/// a client negotiates a protocol version that mandates them — omitting them
/// entirely, as bare `::new`/`::with_all_items` do, fails client-side
/// validation on such clients even though older clients tolerate the
/// omission.
///
/// `LISTING_TTL_MS` covers `tools/list`/`resources/list`: short, because
/// `get_goal_progress`'s `_meta.ui` is gated on the `widget_display_enabled`
/// setting (see `build_tools_gated`), which can change between requests.
/// `WIDGET_HTML_TTL_MS` covers the goal-progress widget's `ui://` resource:
/// long, because that HTML is `include_str!`-baked into the binary and is
/// byte-identical for every request until the next deploy.
/// `WEEKLY_SUMMARY_TTL_MS` covers the weekly-summary resource: short and
/// private, since it reflects live, single-user data that changes as meals
/// and weights are logged.
const LISTING_TTL_MS: u64 = 300_000; // 5 minutes
const WIDGET_HTML_TTL_MS: u64 = 86_400_000; // 24 hours
const WEEKLY_SUMMARY_TTL_MS: u64 = 60_000; // 1 minute

/// Build the `_meta.ui` object that points a Tool declaration at
/// [`GOAL_PROGRESS_UI_RESOURCE_URI`], per the MCP Apps extension
/// (SEP-1865 / modelcontextprotocol/ext-apps spec 2026-01-26).
fn goal_progress_ui_meta() -> rmcp::model::MetaObject {
    let mut ui = serde_json::Map::new();
    ui.insert(
        "resourceUri".to_string(),
        serde_json::Value::String(GOAL_PROGRESS_UI_RESOURCE_URI.to_string()),
    );
    let mut meta = serde_json::Map::new();
    meta.insert("ui".to_string(), serde_json::Value::Object(ui));
    meta.into()
}

/// An MCP server handler backed by an OperationRegistry.
///
/// Implements `list_tools` and `call_tool` by iterating the registry directly,
/// deliberately bypassing rmcp's `#[tool]` macro/ToolRouter.
/// Also implements resource support for `nom://weekly-summary`.
#[derive(Clone)]
pub struct McpHandler {
    registry: Arc<OperationRegistry>,
    clock: Clock,
    #[cfg(test)]
    db_path: Option<std::path::PathBuf>,
}

impl McpHandler {
    pub fn new(registry: Arc<OperationRegistry>, clock: Clock) -> Self {
        Self {
            registry,
            clock,
            #[cfg(test)]
            db_path: None,
        }
    }

    #[cfg(test)]
    pub fn with_db_path(mut self, path: std::path::PathBuf) -> Self {
        self.db_path = Some(path);
        self
    }

    /// Build the list of tools from the registry, filtering by MCP surface
    /// and skipping operations with invalid schemas.
    pub(crate) fn build_tools(&self) -> Vec<Tool> {
        self.registry
            .filter_by_surface(Surfaces::MCP)
            .iter()
            .filter_map(|op| {
                tool_from_operation(op.as_ref())
                    .map_err(|err| {
                        tracing::warn!(
                            operation = op.name(),
                            error = %err,
                            "skipping operation with invalid input_schema",
                        );
                    })
                    .ok()
            })
            .collect()
    }

    /// Build the tool list, additionally gating `get_goal_progress`'s MCP
    /// Apps `_meta.ui` pointer on the widget-display setting (TASK-41).
    ///
    /// Kept as a plain inherent method (mirroring `build_tools`/
    /// `dispatch_read_resource`) so tests can call it directly without
    /// constructing a `RequestContext<RoleServer>`.
    ///
    /// The widget-display setting is a cosmetic nicety, not a prerequisite
    /// for tool discovery: any failure opening the DB or reading the
    /// setting is treated the same as "no settings row" (gating disabled),
    /// logged and otherwise ignored, so a DB hiccup never removes tools
    /// from `tools/list`.
    pub(crate) async fn build_tools_gated(&self) -> Vec<Tool> {
        let mut tools = self.build_tools();

        #[cfg(test)]
        let conn = if let Some(ref path) = self.db_path {
            Connection::open_at(path).await
        } else {
            Connection::open().await
        };

        #[cfg(not(test))]
        let conn = Connection::open().await;

        let widget_display_enabled = match conn {
            Ok(conn) => crate::widget::widget_display_enabled(&conn)
                .await
                .unwrap_or_else(|e| {
                    tracing::warn!(
                        error = %e,
                        "failed to read widget-display setting; defaulting to disabled"
                    );
                    false
                }),
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "failed to open db for widget-display gating; defaulting to disabled"
                );
                false
            }
        };

        if widget_display_enabled
            && let Some(tool) = tools
                .iter_mut()
                .find(|t| t.name.as_ref() == "get_goal_progress")
        {
            tool.meta = Some(goal_progress_ui_meta());
        }

        tools
    }

    /// Build the list of MCP resources this handler exposes.
    pub(crate) fn build_resources(&self) -> Vec<Resource> {
        vec![
            Resource::new("nom://weekly-summary", "weekly-summary")
                .with_title("Weekly Summary")
                .with_description("Rolling 7-day nutrition and weight summary")
                .with_mime_type("application/json"),
        ]
    }

    /// Dispatch a resource read by URI, fetching and serializing the
    /// underlying data. Returns an error for unrecognized URIs.
    pub(crate) async fn dispatch_read_resource(
        &self,
        uri: &str,
    ) -> Result<ReadResourceResult, ErrorData> {
        match uri {
            "nom://weekly-summary" => {
                #[cfg(test)]
                let conn = if let Some(ref path) = self.db_path {
                    Connection::open_at(path).await.map_err(|e| {
                        ErrorData::new(
                            ErrorCode::INTERNAL_ERROR,
                            format!("failed to open db: {e}"),
                            None,
                        )
                    })?
                } else {
                    Connection::open().await.map_err(|e| {
                        ErrorData::new(
                            ErrorCode::INTERNAL_ERROR,
                            format!("failed to open db: {e}"),
                            None,
                        )
                    })?
                };

                #[cfg(not(test))]
                let conn = Connection::open().await.map_err(|e| {
                    ErrorData::new(
                        ErrorCode::INTERNAL_ERROR,
                        format!("failed to open db: {e}"),
                        None,
                    )
                })?;

                let summary = crate::weekly::fetch_weekly_summary(&conn, &self.clock)
                    .await
                    .map_err(|e| {
                        ErrorData::new(
                            ErrorCode::INTERNAL_ERROR,
                            format!("failed to fetch weekly summary: {e}"),
                            None,
                        )
                    })?;

                let json = serde_json::to_string(&summary).map_err(|e| {
                    ErrorData::new(
                        ErrorCode::INTERNAL_ERROR,
                        format!("serialization failed: {e}"),
                        None,
                    )
                })?;

                let contents = ResourceContents::TextResourceContents {
                    uri: uri.to_string(),
                    mime_type: Some("application/json".to_string()),
                    text: json,
                    meta: None,
                };

                Ok(ReadResourceResult::new(vec![contents])
                    .with_ttl_ms(WEEKLY_SUMMARY_TTL_MS)
                    .with_cache_scope(CacheScope::Private))
            }
            GOAL_PROGRESS_UI_RESOURCE_URI => {
                // No DB access needed: this serves the static widget shell.
                // Live data reaches it via the host's `ui/notifications/
                // tool-result` bridge off the unchanged `call_tool` response,
                // not by this handler re-querying on resource fetch.
                let contents = ResourceContents::TextResourceContents {
                    uri: uri.to_string(),
                    mime_type: Some("text/html;profile=mcp-app".to_string()),
                    text: GOAL_PROGRESS_WIDGET_HTML.to_string(),
                    meta: None,
                };
                Ok(ReadResourceResult::new(vec![contents])
                    .with_ttl_ms(WIDGET_HTML_TTL_MS)
                    .with_cache_scope(CacheScope::Public))
            }
            other => Err(ErrorData::new(
                ErrorCode::INVALID_PARAMS,
                format!("unknown resource URI: {}", other),
                None,
            )),
        }
    }

    /// Dispatch a `call_tool` request: look up the operation and invoke it,
    /// wrapping the result/error into a `CallToolResult`.
    ///
    /// Pulled out as a plain async method (mirroring `dispatch_read_resource`)
    /// so tests can exercise the exact same path the `ServerHandler` trait
    /// method uses, without constructing a `RequestContext<RoleServer>`.
    pub(crate) async fn dispatch_call_tool(
        &self,
        name: &str,
        arguments: serde_json::Value,
    ) -> Result<CallToolResult, ErrorData> {
        let op = self
            .registry
            .get(name)
            .ok_or_else(|| ErrorData::invalid_params(format!("unknown tool: {}", name), None))?;

        match op.execute_json(Arc::new(arguments)).await {
            Ok(result) => {
                // Serialize result as text content
                let text = serde_json::to_string(&result).unwrap_or_else(|_| result.to_string());
                Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
            }
            Err(error_data) => {
                // Return as tool-level error per doc-5 §10
                let message = format!("{}", error_data);
                Ok(CallToolResult::error(vec![ContentBlock::text(message)]))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// ServerHandler implementation — override tool/resource/info methods;
// everything else uses the default (no-op / not-found) implementations.
// ---------------------------------------------------------------------------

impl ServerHandler for McpHandler {
    // -- Info --

    fn get_info(&self) -> InitializeResult {
        let capabilities = ServerCapabilities::builder()
            .enable_tools()
            .enable_resources()
            .build();
        InitializeResult::new(capabilities)
            .with_server_info(Implementation::new("nom-mcp", env!("CARGO_PKG_VERSION")))
    }

    // -- Tools --

    fn get_tool(&self, name: &str) -> Option<Tool> {
        match self.registry.get(name) {
            Some(op) => tool_from_operation(op.as_ref()).ok(),
            None => None,
        }
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        Ok(
            ListToolsResult::with_all_items(self.build_tools_gated().await)
                .with_ttl_ms(LISTING_TTL_MS)
                .with_cache_scope(CacheScope::Public),
        )
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        // Convert arguments to serde_json::Value
        let args = match request.arguments {
            Some(map) => serde_json::Value::Object(map),
            None => serde_json::Value::Object(serde_json::Map::new()),
        };

        self.dispatch_call_tool(&request.name, args)
            .await
            .map(Into::into)
    }

    // -- Resources --

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, ErrorData> {
        Ok(ListResourcesResult::with_all_items(self.build_resources())
            .with_ttl_ms(LISTING_TTL_MS)
            .with_cache_scope(CacheScope::Public))
    }

    async fn read_resource(
        &self,
        request: rmcp::model::ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, ErrorData> {
        self.dispatch_read_resource(&request.uri)
            .await
            .map(Into::into)
    }
}

// ---------------------------------------------------------------------------
// Helper: build rmcp::Tool from an Operation reference
// ---------------------------------------------------------------------------

fn tool_from_operation(op: &dyn Operation) -> Result<Tool, ErrorData> {
    let input_schema = op.input_schema().unwrap_or_else(|| {
        serde_json::json!({
            "type": "object",
            "properties": {}
        })
    });

    // Extract JsonObject from schema value.
    // If the operation returns a non-object schema, it violates the contract
    // and we reject it gracefully rather than panicking.
    let schema = match input_schema {
        serde_json::Value::Object(obj) => Arc::new(obj),
        other => {
            return Err(ErrorData::new(
                ErrorCode::INVALID_PARAMS,
                format!(
                    "operation '{}' returned a non-object schema: {:?}",
                    op.name(),
                    other
                ),
                None,
            ));
        }
    };

    Ok(Tool::new(
        op.name().to_string(),
        op.description().to_string(),
        schema,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::Clock;
    use crate::operation::registry::OperationRegistry;
    use crate::storage::test::TempDb;

    fn make_clock() -> Arc<Clock> {
        Arc::new(Clock { tz: chrono_tz::UTC })
    }

    struct TestOp;

    #[async_trait::async_trait]
    impl Operation for TestOp {
        fn name(&self) -> &str {
            "test-op"
        }
        fn description(&self) -> &str {
            "A test operation"
        }
        fn surfaces(&self) -> Surfaces {
            Surfaces::MCP
        }
        async fn execute_json(
            &self,
            args: Arc<serde_json::Value>,
        ) -> Result<serde_json::Value, crate::error::ErrorData> {
            Ok(args.as_object().cloned().unwrap_or_default().into())
        }
    }

    /// Operation whose input_schema() returns a non-object JSON Value.
    /// Used to verify graceful handling of malformed schemas.
    struct BadSchemaOp;

    #[async_trait::async_trait]
    impl Operation for BadSchemaOp {
        fn name(&self) -> &str {
            "bad-schema-op"
        }
        fn description(&self) -> &str {
            "An operation with a broken schema"
        }
        fn surfaces(&self) -> Surfaces {
            Surfaces::MCP
        }
        fn input_schema(&self) -> Option<serde_json::Value> {
            Some(serde_json::json!(["not", "an", "object"]))
        }
        async fn execute_json(
            &self,
            _args: Arc<serde_json::Value>,
        ) -> Result<serde_json::Value, crate::error::ErrorData> {
            Ok(serde_json::json!(null))
        }
    }

    #[test]
    fn test_mcp_handler_new() {
        let mut reg = OperationRegistry::new(make_clock());
        reg.register(Arc::new(TestOp));
        let clock = Clock { tz: chrono_tz::UTC };
        let handler = McpHandler::new(Arc::new(reg), clock);
        assert_eq!(
            handler.get_tool("test-op").map(|t| t.name.to_string()),
            Some("test-op".to_string())
        );
        assert!(handler.get_tool("nonexistent").is_none());
    }

    #[test]
    fn test_tool_from_operation_has_required_fields() {
        let tool = tool_from_operation(&TestOp).unwrap();
        assert_eq!(tool.name.as_ref(), "test-op");
        assert_eq!(tool.description.as_deref(), Some("A test operation"));
    }

    #[test]
    fn test_empty_registry_list_tools() {
        let clock = Clock { tz: chrono_tz::UTC };
        let handler = McpHandler::new(Arc::new(OperationRegistry::new(make_clock())), clock);
        let tools = handler.build_tools();
        assert!(tools.is_empty(), "empty registry should produce no tools");
    }

    #[test]
    fn test_bad_schema_does_not_panic() {
        // tool_from_operation should return Err, not panic
        let result = tool_from_operation(&BadSchemaOp);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("bad-schema-op"));
        assert!(err.message.contains("non-object schema"));
    }

    #[test]
    fn test_get_tool_skips_bad_schema() {
        let clock = Clock { tz: chrono_tz::UTC };
        let mut reg = OperationRegistry::new(make_clock());
        reg.register(Arc::new(BadSchemaOp));
        let handler = McpHandler::new(Arc::new(reg), clock);
        // get_tool should return None for operations with bad schemas (no panic)
        assert!(handler.get_tool("bad-schema-op").is_none());
    }

    #[test]
    fn test_list_tools_omits_bad_schema_but_keeps_good_ops() {
        let clock = Clock { tz: chrono_tz::UTC };
        let mut reg = OperationRegistry::new(make_clock());
        reg.register(Arc::new(TestOp));
        reg.register(Arc::new(BadSchemaOp));
        let handler = McpHandler::new(Arc::new(reg), clock);

        let tools = handler.build_tools();

        // Should have exactly 1 tool (the good one), not 2
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name.as_ref(), "test-op");
    }

    #[test]
    fn test_build_resources_lists_weekly_summary() {
        let clock = Clock { tz: chrono_tz::UTC };
        let handler = McpHandler::new(Arc::new(OperationRegistry::new(make_clock())), clock);
        let resources = handler.build_resources();
        assert_eq!(resources.len(), 1);
        assert_eq!(resources[0].uri, "nom://weekly-summary");
        assert_eq!(resources[0].title.as_deref(), Some("Weekly Summary"));
        assert_eq!(resources[0].mime_type.as_deref(), Some("application/json"));
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_dispatch_read_resource_returns_weekly_summary_json() {
        let db = TempDb::new().await;
        let clock = Clock { tz: chrono_tz::UTC };
        let handler = McpHandler::new(Arc::new(OperationRegistry::new(make_clock())), clock)
            .with_db_path(db.path.clone());

        let result = handler.dispatch_read_resource("nom://weekly-summary").await;
        assert!(result.is_ok());
        let ReadResourceResult { contents, .. } = result.unwrap();
        let ResourceContents::TextResourceContents { text, .. } = &contents[0] else {
            panic!("expected text contents")
        };
        let value: serde_json::Value = serde_json::from_str(text).unwrap();
        assert!(value.get("start_date").is_some());
        assert!(value.get("end_date").is_some());
        assert!(value.get("nutrients").is_some());
        assert!(value.get("weight").is_some());
    }

    #[serial_test::serial]
    #[tokio::test]
    async fn test_dispatch_read_resource_unknown_uri_errors() {
        let clock = Clock { tz: chrono_tz::UTC };
        let handler = McpHandler::new(Arc::new(OperationRegistry::new(make_clock())), clock);
        let result = handler.dispatch_read_resource("nom://bogus").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("unknown resource URI"));
    }

    // ---- TASK-41: widget-display-gated `_meta.ui` on get_goal_progress ----

    fn handler_with_goal_progress(db_path: std::path::PathBuf) -> McpHandler {
        let clock = Clock { tz: chrono_tz::UTC };
        let mut reg = OperationRegistry::new(make_clock());
        reg.register(Arc::new(
            crate::goal::GetGoalProgress::new(clock).with_db_path(db_path.clone()),
        ));
        // A second, unrelated tool proves the gate is scoped to
        // get_goal_progress and doesn't leak onto every tool.
        reg.register(Arc::new(TestOp));
        McpHandler::new(Arc::new(reg), clock).with_db_path(db_path)
    }

    /// AC#4: with no `settings` row (today's default), `get_goal_progress`'s
    /// `meta` is `None`, matching every other tool — list_tools output is
    /// unchanged from pre-widget behavior.
    #[serial_test::serial]
    #[tokio::test]
    async fn test_list_tools_widget_display_default_false_no_meta() {
        let db = TempDb::new().await;
        let handler = handler_with_goal_progress(db.path.clone());

        let tools = handler.build_tools_gated().await;

        let goal_progress = tools
            .iter()
            .find(|t| t.name.as_ref() == "get_goal_progress")
            .expect("get_goal_progress should be registered");
        assert!(goal_progress.meta.is_none());

        let test_op = tools
            .iter()
            .find(|t| t.name.as_ref() == "test-op")
            .expect("test-op should be registered");
        assert!(test_op.meta.is_none());
    }

    /// AC#1: with `widget_display_enabled = true`, `get_goal_progress`'s
    /// tool declaration carries `_meta.ui.resourceUri` pointing at the
    /// registered `ui://` resource — and no other tool is affected.
    #[serial_test::serial]
    #[tokio::test]
    async fn test_list_tools_widget_display_true_adds_ui_meta_to_goal_progress_only() {
        let db = TempDb::new().await;

        let set_widget = crate::widget::SetWidgetDisplay::new().with_db_path(db.path.clone());
        set_widget
            .execute_json(Arc::new(serde_json::json!({ "enabled": true })))
            .await
            .unwrap();

        let handler = handler_with_goal_progress(db.path.clone());
        let tools = handler.build_tools_gated().await;

        let goal_progress = tools
            .iter()
            .find(|t| t.name.as_ref() == "get_goal_progress")
            .expect("get_goal_progress should be registered");
        let meta = goal_progress.meta.as_ref().expect("meta should be set");
        assert_eq!(
            meta.0.get("ui").and_then(|ui| ui.get("resourceUri")),
            Some(&serde_json::Value::String(
                GOAL_PROGRESS_UI_RESOURCE_URI.to_string()
            ))
        );

        // Scoped to get_goal_progress only — the other registered tool is
        // untouched.
        let test_op = tools
            .iter()
            .find(|t| t.name.as_ref() == "test-op")
            .expect("test-op should be registered");
        assert!(test_op.meta.is_none());
    }

    /// TASK-44: a DB-open failure while resolving the widget-display
    /// setting must not take down `tools/list` for every other tool — it
    /// should be treated exactly like "no settings row" (gating disabled).
    #[serial_test::serial]
    #[tokio::test]
    async fn test_list_tools_db_open_failure_falls_back_to_no_gating() {
        let dir = tempfile::TempDir::with_prefix("nom_test").unwrap();
        // A plain file where a directory is expected: `Connection::open_at`
        // calls `create_dir_all` on the db path's parent, which fails when
        // that parent already exists as a regular file — a reliable,
        // permission-independent way to force an open error.
        let blocking_file = dir.path().join("not_a_dir");
        std::fs::write(&blocking_file, b"not a directory").unwrap();
        let bad_db_path = blocking_file.join("unreachable.db");

        let handler = handler_with_goal_progress(bad_db_path);
        let tools = handler.build_tools_gated().await;

        let goal_progress = tools
            .iter()
            .find(|t| t.name.as_ref() == "get_goal_progress")
            .expect("get_goal_progress should still be discoverable");
        assert!(
            goal_progress.meta.is_none(),
            "gating should default to disabled when the DB can't be opened"
        );

        let test_op = tools
            .iter()
            .find(|t| t.name.as_ref() == "test-op")
            .expect("unrelated tool must remain discoverable despite the DB failure");
        assert!(test_op.meta.is_none());
    }

    /// AC#2: `resources/read` for the widget URI returns a valid
    /// `text/html;profile=mcp-app` document.
    #[serial_test::serial]
    #[tokio::test]
    async fn test_dispatch_read_resource_goal_progress_widget() {
        let clock = Clock { tz: chrono_tz::UTC };
        let handler = McpHandler::new(Arc::new(OperationRegistry::new(make_clock())), clock);

        let result = handler
            .dispatch_read_resource(GOAL_PROGRESS_UI_RESOURCE_URI)
            .await;
        assert!(result.is_ok());
        let ReadResourceResult { contents, .. } = result.unwrap();
        let ResourceContents::TextResourceContents {
            mime_type, text, ..
        } = &contents[0]
        else {
            panic!("expected text contents")
        };
        assert_eq!(mime_type.as_deref(), Some("text/html;profile=mcp-app"));
        assert!(!text.is_empty());
        assert!(text.contains("<!DOCTYPE html>"));
    }

    /// AC#3: `call_tool("get_goal_progress")` returns byte-identical content
    /// whether `widget_display_enabled` is true or false — the widget only
    /// changes how a capable host *renders* the result, never the tool's own
    /// response.
    #[serial_test::serial]
    #[tokio::test]
    async fn test_call_tool_get_goal_progress_unchanged_by_widget_flag() {
        let db = TempDb::new().await;
        let handler = handler_with_goal_progress(db.path.clone());

        let args = serde_json::json!({ "date": "2025-01-15" });

        let result_before = handler
            .dispatch_call_tool("get_goal_progress", args.clone())
            .await
            .unwrap();

        let set_widget = crate::widget::SetWidgetDisplay::new().with_db_path(db.path.clone());
        set_widget
            .execute_json(Arc::new(serde_json::json!({ "enabled": true })))
            .await
            .unwrap();

        let result_after = handler
            .dispatch_call_tool("get_goal_progress", args)
            .await
            .unwrap();

        assert_eq!(
            serde_json::to_string(&result_before).unwrap(),
            serde_json::to_string(&result_after).unwrap(),
        );
    }
}

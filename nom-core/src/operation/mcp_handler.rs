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
        CallToolRequestParams, CallToolResult, ContentBlock, ErrorCode, Implementation,
        InitializeResult, ListResourcesResult, ListToolsResult, PaginatedRequestParams,
        ReadResourceResult, Resource, ResourceContents, ServerCapabilities, Tool,
    },
    service::RequestContext,
};

use super::{Operation, OperationRegistry, Surfaces};
use crate::clock::Clock;
use crate::storage::Connection;

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

                Ok(ReadResourceResult::new(vec![contents]))
            }
            other => Err(ErrorData::new(
                ErrorCode::INVALID_PARAMS,
                format!("unknown resource URI: {}", other),
                None,
            )),
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
        let tools = self.build_tools();
        Ok(ListToolsResult::with_all_items(tools))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let op = self.registry.get(&request.name).ok_or_else(|| {
            ErrorData::invalid_params(format!("unknown tool: {}", request.name), None)
        })?;

        // Convert arguments to serde_json::Value
        let args = match request.arguments {
            Some(map) => serde_json::Value::Object(map),
            None => serde_json::Value::Object(serde_json::Map::new()),
        };

        match op.execute_json(Arc::new(args)).await {
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

    // -- Resources --

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, ErrorData> {
        Ok(ListResourcesResult::with_all_items(self.build_resources()))
    }

    async fn read_resource(
        &self,
        request: rmcp::model::ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, ErrorData> {
        self.dispatch_read_resource(&request.uri).await
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
}

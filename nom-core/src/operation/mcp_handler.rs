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
        CallToolRequestParams, CallToolResult, ContentBlock, ErrorCode, ListToolsResult,
        PaginatedRequestParams, Tool,
    },
    service::RequestContext,
};

use super::{Operation, OperationRegistry, Surfaces};

/// An MCP server handler backed by an OperationRegistry.
///
/// Implements `list_tools` and `call_tool` by iterating the registry directly,
/// deliberately bypassing rmcp's `#[tool]` macro/ToolRouter.
pub struct McpHandler {
    registry: Arc<OperationRegistry>,
}

impl McpHandler {
    pub fn new(registry: OperationRegistry) -> Self {
        Self {
            registry: Arc::new(registry),
        }
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
}

// ---------------------------------------------------------------------------
// ServerHandler implementation — only override tool-related methods;
// everything else uses the default (no-op / not-found) implementations.
// ---------------------------------------------------------------------------

impl ServerHandler for McpHandler {
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
    use crate::operation::registry::OperationRegistry;

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
        let mut reg = OperationRegistry::new();
        reg.register(Arc::new(TestOp));
        let handler = McpHandler::new(reg);
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
        let handler = McpHandler::new(OperationRegistry::new());
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
        let mut reg = OperationRegistry::new();
        reg.register(Arc::new(BadSchemaOp));
        let handler = McpHandler::new(reg);
        // get_tool should return None for operations with bad schemas (no panic)
        assert!(handler.get_tool("bad-schema-op").is_none());
    }

    #[test]
    fn test_list_tools_omits_bad_schema_but_keeps_good_ops() {
        let mut reg = OperationRegistry::new();
        reg.register(Arc::new(TestOp));
        reg.register(Arc::new(BadSchemaOp));
        let handler = McpHandler::new(reg);

        let tools = handler.build_tools();

        // Should have exactly 1 tool (the good one), not 2
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name.as_ref(), "test-op");
    }
}

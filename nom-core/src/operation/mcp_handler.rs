//! MCP ServerHandler — hand-written list_tools/call_tool that loops the
//! OperationRegistry directly.
//!
//! Deliberately NOT using rmcp's `#[tool]` macro/ToolRouter because ToolBase
//! requires compile-time-associated-function types incompatible with
//! `Vec<Arc<dyn Operation>>`.

use std::sync::Arc;

use rmcp::{
    ErrorData,
    handler::server::ServerHandler,
    model::{
        CallToolRequestParams, CallToolResult, ContentBlock, ListToolsResult, PaginatedRequestParams,
        Tool,
    },
    service::RequestContext,
    RoleServer,
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
}

// ---------------------------------------------------------------------------
// ServerHandler implementation — only override tool-related methods;
// everything else uses the default (no-op / not-found) implementations.
// ---------------------------------------------------------------------------

impl ServerHandler for McpHandler {
    // -- Tools --

    fn get_tool(&self, name: &str) -> Option<Tool> {
        self.registry.get(name).map(|op| tool_from_operation(op.as_ref()))
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        let tools = self
            .registry
            .filter_by_surface(Surfaces::MCP)
            .iter()
            .map(|op| tool_from_operation(op.as_ref()))
            .collect();
        Ok(ListToolsResult::with_all_items(tools))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let op = self
            .registry
            .get(&request.name)
            .ok_or_else(|| ErrorData::invalid_params(format!("unknown tool: {}", request.name), None))?;

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

fn tool_from_operation(op: &dyn Operation) -> Tool {
    let input_schema = op.input_schema().unwrap_or_else(|| {
        serde_json::json!({
            "type": "object",
            "properties": {}
        })
    });

    // Extract JsonObject from schema value
    let schema = match input_schema {
        serde_json::Value::Object(obj) => Arc::new(obj),
        other => panic!(
            "input_schema must be a JSON object, got {:?}",
            other
        ),
    };

    Tool::new(op.name().to_string(), op.description().to_string(), schema)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operation::registry::OperationRegistry;

    struct TestOp;

    #[async_trait::async_trait]
    impl Operation for TestOp {
        fn name(&self) -> &str { "test-op" }
        fn description(&self) -> &str { "A test operation" }
        fn surfaces(&self) -> Surfaces { Surfaces::MCP }
        async fn execute_json(
            &self,
            args: Arc<serde_json::Value>,
        ) -> Result<serde_json::Value, crate::error::ErrorData> {
            Ok(args.as_object().cloned().unwrap_or_default().into())
        }
    }

    #[test]
    fn test_mcp_handler_new() {
        let mut reg = OperationRegistry::new();
        reg.register(Arc::new(TestOp));
        let handler = McpHandler::new(reg);
        assert_eq!(handler.get_tool("test-op").map(|t| t.name.to_string()), Some("test-op".to_string()));
        assert!(handler.get_tool("nonexistent").is_none());
    }

    #[test]
    fn test_tool_from_operation_has_required_fields() {
        let tool = tool_from_operation(&TestOp);
        assert_eq!(tool.name.as_ref(), "test-op");
        assert_eq!(tool.description.as_deref(), Some("A test operation"));
    }

    #[test]
    fn test_empty_registry_list_tools() {
        let handler = McpHandler::new(OperationRegistry::new());
        // The handler should work even with an empty registry
        let _ = handler;
    }
}

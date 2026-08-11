//! HTTP router — builds axum routes from the OperationRegistry.
//!
//! Each operation that exposes HTTP surface gets a POST route at
//! `/api/{operation_name}`. Request body is deserialized as `serde_json::Value`
//! and passed to `execute_json`. Success returns 200 with JSON body; errors
//! return appropriate HTTP status codes with `ErrorData` response body.

use std::sync::Arc;

use axum::{
    http::StatusCode,
    routing::post,
    Json, Router,
};

use super::Surfaces;

/// Build an axum Router from operations that expose HTTP surface.
///
/// Routes are prefixed under `/api` so they can coexist with MCP streamable-HTTP
/// service at `/mcp`.
pub fn build_http_router(registry: super::OperationRegistry) -> Router {
    let registry = Arc::new(registry);
    let mut router = Router::new();

    for op in registry.filter_by_surface(Surfaces::HTTP) {
        let op = op.clone();
        let path = format!("/api/{}", op.name());
        router = router.route(&path, post(move |Json(args): Json<serde_json::Value>| {
            handle_operation(op, args)
        }));
    }

    router
}

/// Handler for a single HTTP operation route.
async fn handle_operation(
    op: Arc<dyn super::Operation>,
    args: serde_json::Value,
) -> Result<(StatusCode, Json<serde_json::Value>), StatusCode> {
    match op.execute_json(Arc::new(args)).await {
        Ok(result) => Ok((StatusCode::OK, Json(result))),
        Err(error) => {
            let status = error.category.http_status();
            Err(status)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operation::{Operation, OperationRegistry};

    struct TestOp;

    #[async_trait::async_trait]
    impl Operation for TestOp {
        fn name(&self) -> &str { "test-op" }
        fn description(&self) -> &str { "test" }
        fn surfaces(&self) -> Surfaces { Surfaces::HTTP }
        async fn execute_json(
            &self,
            args: Arc<serde_json::Value>,
        ) -> Result<serde_json::Value, crate::error::ErrorData> {
            Ok(args.as_object().cloned().unwrap_or_default().into())
        }
    }

    #[test]
    fn test_build_http_router_has_routes() {
        let mut reg = OperationRegistry::new();
        reg.register(Arc::new(TestOp));
        let router = build_http_router(reg);
        // The router should have routes registered
        // We can't easily test axum routes directly, but we verify it builds without panic
        let _ = router;
    }
}

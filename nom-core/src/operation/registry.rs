//! OperationRegistry — single source of truth for all transport surfaces.
//!
//! Holds a `Vec<Arc<dyn Operation>>` and provides lookup, iteration, and
//! surface-filtered queries. CLI, HTTP, and MCP routers all read from this
//! same registry instance.

use std::sync::Arc;

use super::{Operation, Surfaces};

/// Registry of domain operations, shared across CLI, HTTP, and MCP surfaces.
///
/// Adding an operation to the registry automatically makes it available on
/// every surface the operation declares via `.surfaces()`.
#[derive(Default)]
pub struct OperationRegistry {
    operations: Vec<Arc<dyn Operation>>,
}

impl OperationRegistry {
    pub fn new() -> Self {
        Self {
            operations: Vec::new(),
        }
    }

    /// Register an operation in the registry.
    pub fn register(&mut self, op: Arc<dyn Operation>) {
        self.operations.push(op);
    }

    /// Look up an operation by name.
    pub fn get(&self, name: &str) -> Option<&Arc<dyn Operation>> {
        self.operations.iter().find(|op| op.name() == name)
    }

    /// Iterate over all registered operations.
    pub fn iter(&self) -> impl Iterator<Item = &Arc<dyn Operation>> {
        self.operations.iter()
    }

    /// Return only operations whose surfaces intersect the given filter.
    pub fn filter_by_surface(&self, surfaces: Surfaces) -> Vec<&Arc<dyn Operation>> {
        self.operations
            .iter()
            .filter(|op| op.surfaces().intersects(surfaces))
            .collect()
    }

    /// Number of registered operations.
    pub fn len(&self) -> usize {
        self.operations.len()
    }

    /// True if no operations are registered.
    pub fn is_empty(&self) -> bool {
        self.operations.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Minimal mock operation for testing
    struct MockOp {
        name: &'static str,
        description: &'static str,
        surfaces: Surfaces,
    }

    #[async_trait::async_trait]
    impl Operation for MockOp {
        fn name(&self) -> &str {
            self.name
        }

        fn description(&self) -> &str {
            self.description
        }

        fn surfaces(&self) -> Surfaces {
            self.surfaces
        }

        async fn execute_json(
            &self,
            _args: Arc<serde_json::Value>,
        ) -> Result<serde_json::Value, crate::error::ErrorData> {
            Ok(serde_json::json!({ "mock": true }))
        }
    }

    #[test]
    fn test_register_and_get() {
        let mut registry = OperationRegistry::new();
        let op = Arc::new(MockOp {
            name: "test_op",
            description: "A test operation",
            surfaces: Surfaces::ALL,
        });
        registry.register(op);
        assert!(registry.get("test_op").is_some());
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn test_filter_by_cli_surface() {
        let mut registry = OperationRegistry::new();
        registry.register(Arc::new(MockOp {
            name: "cli_only",
            description: "CLI only",
            surfaces: Surfaces::CLI,
        }));
        registry.register(Arc::new(MockOp {
            name: "http_only",
            description: "HTTP only",
            surfaces: Surfaces::HTTP,
        }));
        registry.register(Arc::new(MockOp {
            name: "all_surfaces",
            description: "All surfaces",
            surfaces: Surfaces::ALL,
        }));

        let cli_ops = registry.filter_by_surface(Surfaces::CLI);
        assert_eq!(cli_ops.len(), 2);
        assert_eq!(cli_ops[0].name(), "cli_only");
        assert_eq!(cli_ops[1].name(), "all_surfaces");
    }

    #[test]
    fn test_filter_by_http_surface() {
        let mut registry = OperationRegistry::new();
        registry.register(Arc::new(MockOp {
            name: "cli_only",
            description: "CLI only",
            surfaces: Surfaces::CLI,
        }));
        registry.register(Arc::new(MockOp {
            name: "http_only",
            description: "HTTP only",
            surfaces: Surfaces::HTTP,
        }));

        let http_ops = registry.filter_by_surface(Surfaces::HTTP);
        assert_eq!(http_ops.len(), 1);
        assert_eq!(http_ops[0].name(), "http_only");
    }

    #[test]
    fn test_filter_by_mcp_surface() {
        let mut registry = OperationRegistry::new();
        registry.register(Arc::new(MockOp {
            name: "mcp_only",
            description: "MCP only",
            surfaces: Surfaces::MCP,
        }));
        registry.register(Arc::new(MockOp {
            name: "all_surfaces",
            description: "All surfaces",
            surfaces: Surfaces::ALL,
        }));

        let mcp_ops = registry.filter_by_surface(Surfaces::MCP);
        assert_eq!(mcp_ops.len(), 2);
        assert_eq!(mcp_ops[0].name(), "mcp_only");
        assert_eq!(mcp_ops[1].name(), "all_surfaces");
    }

    #[test]
    fn test_default_surfaces_is_all() {
        struct DefaultSurfacesOp;

        #[async_trait::async_trait]
        impl Operation for DefaultSurfacesOp {
            fn name(&self) -> &str {
                "default_surfaces"
            }
            fn description(&self) -> &str {
                "Uses default surfaces"
            }
            // Does not override surfaces() — should default to ALL
            async fn execute_json(
                &self,
                _args: Arc<serde_json::Value>,
            ) -> Result<serde_json::Value, crate::error::ErrorData> {
                Ok(serde_json::json!({}))
            }
        }

        let op = Arc::new(DefaultSurfacesOp);
        assert_eq!(op.surfaces(), Surfaces::ALL);
    }

    #[test]
    fn test_empty_registry() {
        let registry = OperationRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
        assert!(registry.get("anything").is_none());
        assert_eq!(registry.filter_by_surface(Surfaces::ALL).len(), 0);
    }
}

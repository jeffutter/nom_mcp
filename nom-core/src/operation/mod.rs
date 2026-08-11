//! Operation trait and multi-surface registry.
//!
//! Every domain operation implements `Operation`, declaring which transport
//! surfaces (CLI, HTTP, MCP) it supports via `.surfaces()`. A single
//! `OperationRegistry` instance drives CLI subcommand registration, HTTP route
//! registration, and MCP `list_tools/call_tool` — adding one Operation appears
//! on all three surfaces it declares, closing CLI/HTTP-vs-MCP drift by construction.

use std::sync::Arc;

pub mod cli_router;
pub mod http_router;
pub mod mcp_handler;
pub mod registry;

pub use registry::OperationRegistry;

// ---------------------------------------------------------------------------
// Surfaces bitmask — which transports an Operation exposes itself through
// ---------------------------------------------------------------------------

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
    pub struct Surfaces: u8 {
        const CLI = 0b001;
        const HTTP = 0b010;
        const MCP = 0b100;
        const ALL = Self::CLI.bits() | Self::HTTP.bits() | Self::MCP.bits();
    }
}

impl Default for Surfaces {
    fn default() -> Self {
        Self::ALL
    }
}

// ---------------------------------------------------------------------------
// Operation trait — the single interface every domain action implements
// ---------------------------------------------------------------------------

/// A domain operation that can be executed across multiple transport surfaces.
///
/// Each operation declares its name, description, input schema, supported
/// surfaces, and provides an async execution method that accepts raw JSON
/// arguments and returns structured results or unified errors.
#[async_trait::async_trait]
pub trait Operation: Send + Sync {
    /// Unique operation name used as CLI subcommand, HTTP path, and MCP tool name.
    fn name(&self) -> &str;

    /// Human-readable description shown in help text and tool listings.
    fn description(&self) -> &str;

    /// Which transport surfaces this operation is available on.
    /// Defaults to `Surfaces::ALL` (CLI + HTTP + MCP).
    fn surfaces(&self) -> Surfaces {
        Surfaces::ALL
    }

    /// JSON Schema describing the expected input parameters.
    /// Derived from request structs using `schemars::JsonSchema`.
    fn input_schema(&self) -> Option<serde_json::Value> {
        None
    }

    /// Execute the operation with the given JSON arguments.
    ///
    /// Deserializes `args` internally via the caller's request struct, performs
    /// the domain logic, and returns structured JSON on success or `ErrorData`
    /// on failure. This is the type-erasure point confirmed by both rpc-toolkit
    /// and traitclaw prior art.
    async fn execute_json(
        &self,
        args: Arc<serde_json::Value>,
    ) -> Result<serde_json::Value, crate::error::ErrorData>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_surfaces_default_is_all() {
        assert_eq!(Surfaces::default(), Surfaces::ALL);
    }

    #[test]
    fn test_surfaces_cli_only() {
        assert_eq!(Surfaces::CLI.bits(), 0b001);
    }

    #[test]
    fn test_surfaces_http_only() {
        assert_eq!(Surfaces::HTTP.bits(), 0b010);
    }

    #[test]
    fn test_surfaces_mcp_only() {
        assert_eq!(Surfaces::MCP.bits(), 0b100);
    }

    #[test]
    fn test_surfaces_intersection() {
        let op_surfaces = Surfaces::CLI | Surfaces::HTTP;
        assert!(op_surfaces.contains(Surfaces::CLI));
        assert!(op_surfaces.contains(Surfaces::HTTP));
        assert!(!op_surfaces.contains(Surfaces::MCP));
    }
}

//! CLI router — builds clap subcommands from the OperationRegistry.
//!
//! Two-phase bootstrap per doc-1 §6:
//! 1. Build static command tree (name + description only, no argument definitions)
//! 2. At dispatch time, extract raw args as JSON and pass to `execute_json`

use std::sync::Arc;

use clap::{Arg, ArgAction, Command};

use super::{Operation, Surfaces};

/// Build a clap Command tree from operations that expose CLI surface.
pub fn build_cli_command(registry: &super::OperationRegistry) -> Command {
    let mut cmd = Command::new("nom-mcp")
        .about("NOM nutrition tracker — local CLI mode");

    for op in registry.filter_by_surface(Surfaces::CLI) {
        let name: &'static str = Box::leak(op.name().to_string().into_boxed_str());
        let desc: &'static str = Box::leak(op.description().to_string().into_boxed_str());
        let subcmd = Command::new(name)
            .about(desc)
            .arg(
                Arg::new("args")
                    .num_args(0..)
                    .action(ArgAction::Set),
            );
        cmd = cmd.subcommand(subcmd);
    }

    cmd
}

/// Parse CLI arguments and dispatch to the matching operation.
///
/// Returns the operation name and raw arguments as JSON-compatible values,
/// or an error if no matching subcommand was found.
pub fn parse_and_dispatch(
    registry: &super::OperationRegistry,
    args: &[String],
) -> Result<(Arc<dyn Operation>, Arc<serde_json::Value>), crate::error::ErrorData> {
    let cmd = build_cli_command(registry);
    let matches = cmd.clone().try_get_matches_from(args).map_err(|e| {
        crate::error::ErrorData::validation("arguments", e.to_string())
    })?;

    // Extract subcommand name
    let subcommand_name = matches
        .subcommand()
        .ok_or_else(|| crate::error::ErrorData::validation("arguments", "no subcommand provided"))?
        .0;

    // Look up the operation by name
    let op = registry.get(subcommand_name).ok_or_else(|| {
        crate::error::ErrorData::validation(
            "arguments",
            format!("unknown subcommand: {subcommand_name}"),
        )
    })?;

    // Extract remaining arguments as JSON-compatible values
    let args_json = match matches.subcommand_matches(subcommand_name) {
        Some(sub_matches) => {
            let mut map = serde_json::Map::new();
            for val in sub_matches.get_many::<String>("args").into_iter().flatten() {
                // Try key=value format first
                if let Some((key, value)) = val.split_once('=') {
                    map.insert(key.to_string(), parse_value(value));
                } else {
                    map.insert(val.clone(), serde_json::Value::Bool(true));
                }
            }
            serde_json::Value::Object(map)
        }
        None => serde_json::Value::Object(serde_json::Map::new()),
    };

    Ok((op.clone(), Arc::new(args_json)))
}

/// Parse a CLI argument value into a JSON-compatible type.
fn parse_value(s: &str) -> serde_json::Value {
    // Try parsing as JSON first (handles numbers, booleans, null)
    if let Ok(val) = serde_json::from_str(s) {
        return val;
    }
    serde_json::Value::String(s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operation::registry::OperationRegistry;

    struct TestOp {
        name: &'static str,
        surfaces: Surfaces,
    }

    #[async_trait::async_trait]
    impl Operation for TestOp {
        fn name(&self) -> &str { self.name }
        fn description(&self) -> &str { "test" }
        fn surfaces(&self) -> Surfaces { self.surfaces }
        async fn execute_json(
            &self,
            _args: Arc<serde_json::Value>,
        ) -> Result<serde_json::Value, crate::error::ErrorData> {
            Ok(serde_json::json!({}))
        }
    }

    #[test]
    fn test_build_cli_command_includes_cli_ops() {
        let mut reg = OperationRegistry::new();
        reg.register(Arc::new(TestOp {
            name: "food-search",
            surfaces: Surfaces::ALL,
        }));
        reg.register(Arc::new(TestOp {
            name: "internal-op",
            surfaces: Surfaces::MCP,
        }));

        let cmd = build_cli_command(&reg);
        // Only food-search should appear (internal-op is MCP-only)
        let subs: Vec<&Command> = cmd.get_subcommands().collect();
        assert_eq!(subs.len(), 1);
        assert_eq!(subs[0].get_name(), "food-search");
    }

    #[test]
    fn test_parse_and_dispatch_no_subcommand() {
        let reg = OperationRegistry::new();
        let result = parse_and_dispatch(&reg, &["nom-mcp".to_string()]);
        assert!(result.is_err());
    }
}

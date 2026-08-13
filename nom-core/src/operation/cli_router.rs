//! CLI router — builds clap subcommands from the OperationRegistry.
//!
//! Two-phase bootstrap per doc-1 §6:
//! 1. Build static command tree with per-field Args derived from each
//!    operation's `input_schema()` — names, types, descriptions, required-ness
//! 2. At dispatch time, extract named args as JSON and pass to `execute_json`

use std::sync::Arc;

use clap::{Arg, ArgAction, Command};

use super::{Operation, Surfaces};

/// Build a clap Command tree from operations that expose CLI surface.
///
/// For each operation, walks `input_schema()` properties into individual
/// clap `Arg`s so `--help` shows real field names, types, required-ness,
/// and doc-comment descriptions.
pub fn build_cli_command(registry: &super::OperationRegistry) -> Command {
    let mut cmd = Command::new("nom-mcp")
        .about("NOM nutrition tracker — local CLI mode")
        .arg_required_else_help(true);

    for op in registry.filter_by_surface(Surfaces::CLI) {
        let name: &'static str = Box::leak(op.name().to_string().into_boxed_str());
        let desc: &'static str = Box::leak(op.description().to_string().into_boxed_str());

        let mut subcmd = Command::new(name).about(desc);

        // Derive per-field Args from input_schema()
        // Collect owned field data first to satisfy 'static lifetime requirements
        let fields: Vec<(&'static str, &'static str, bool)> = if let Some(schema) =
            op.input_schema()
        {
            let required_keys: std::collections::HashSet<&str> = schema
                .get("required")
                .and_then(|r| r.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
                .unwrap_or_default();

            if let Some(properties) = schema.get("properties").and_then(|p| p.as_object()) {
                properties
                    .iter()
                    .map(|(key, prop)| {
                        let key_owned: &'static str = Box::leak(key.to_string().into_boxed_str());
                        let help_owned: &'static str = Box::leak(
                            prop.get("description")
                                .and_then(|d| d.as_str())
                                .unwrap_or(key_owned)
                                .to_string()
                                .into_boxed_str(),
                        );
                        let req = required_keys.contains(key_owned);
                        (key_owned, help_owned, req)
                    })
                    .collect()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        // Build clap Args from collected field data
        if fields.is_empty() {
            subcmd = subcmd.arg_required_else_help(false);
        }

        for (key, help, required) in fields {
            let mut arg = Arg::new(key)
                .long(key)
                .help(help)
                .num_args(1)
                .action(ArgAction::Set);
            if required {
                arg = arg.required(true);
            }
            subcmd = subcmd.arg(arg);
        }

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
    // `-h`/`--help` (and any genuine usage error) get clap's own formatted
    // output and exit code via `e.exit()` rather than being wrapped in
    // ErrorData, which would mangle the help text with an error prefix.
    let matches = cmd
        .clone()
        .try_get_matches_from(args)
        .unwrap_or_else(|e| e.exit());

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

    // Extract named args from clap matches into a JSON map
    let args_json = match matches.subcommand_matches(subcommand_name) {
        Some(sub_matches) => {
            let mut map = serde_json::Map::new();
            for id in sub_matches.ids() {
                let key = id.as_str();
                if let Some(val) = sub_matches.get_one::<String>(key) {
                    map.insert(key.to_string(), parse_value(val));
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
    use crate::clock::Clock;
    use crate::operation::registry::OperationRegistry;

    fn make_clock() -> Arc<Clock> {
        Arc::new(Clock { tz: chrono_tz::UTC })
    }

    // ---- Mock operations for testing ----

    /// Operation with one required field (`value`)
    struct RequiredFieldOp {
        surfaces: Surfaces,
    }

    #[async_trait::async_trait]
    impl Operation for RequiredFieldOp {
        fn name(&self) -> &str {
            "required_field_op"
        }
        fn description(&self) -> &str {
            "An operation with a required field"
        }
        fn surfaces(&self) -> Surfaces {
            self.surfaces
        }
        fn input_schema(&self) -> Option<serde_json::Value> {
            Some(serde_json::json!({
                "$schema": "http://json-schema.org/draft-07/schema#",
                "title": "RequiredFieldRequest",
                "type": "object",
                "properties": {
                    "value": {
                        "description": "The weight value.",
                        "type": "number"
                    }
                },
                "required": ["value"]
            }))
        }
        async fn execute_json(
            &self,
            _args: Arc<serde_json::Value>,
        ) -> Result<serde_json::Value, crate::error::ErrorData> {
            Ok(serde_json::json!({}))
        }
    }

    /// Operation with both required and optional fields
    struct MixedFieldOp {
        surfaces: Surfaces,
    }

    #[async_trait::async_trait]
    impl Operation for MixedFieldOp {
        fn name(&self) -> &str {
            "mixed_field_op"
        }
        fn description(&self) -> &str {
            "An operation with required and optional fields"
        }
        fn surfaces(&self) -> Surfaces {
            self.surfaces
        }
        fn input_schema(&self) -> Option<serde_json::Value> {
            Some(serde_json::json!({
                "$schema": "http://json-schema.org/draft-07/schema#",
                "title": "MixedFieldRequest",
                "type": "object",
                "properties": {
                    "entry_id": {
                        "description": "The entry ID to update.",
                        "type": "integer"
                    },
                    "value": {
                        "description": "New value (optional).",
                        "type": "number"
                    }
                },
                "required": ["entry_id"]
            }))
        }
        async fn execute_json(
            &self,
            _args: Arc<serde_json::Value>,
        ) -> Result<serde_json::Value, crate::error::ErrorData> {
            Ok(serde_json::json!({}))
        }
    }

    /// Operation with no fields (empty schema)
    struct NoFieldsOp {
        surfaces: Surfaces,
    }

    #[async_trait::async_trait]
    impl Operation for NoFieldsOp {
        fn name(&self) -> &str {
            "no_fields_op"
        }
        fn description(&self) -> &str {
            "An operation with no arguments"
        }
        fn surfaces(&self) -> Surfaces {
            self.surfaces
        }
        fn input_schema(&self) -> Option<serde_json::Value> {
            Some(serde_json::json!({
                "$schema": "http://json-schema.org/draft-07/schema#",
                "title": "EmptyRequest",
                "type": "object"
            }))
        }
        async fn execute_json(
            &self,
            _args: Arc<serde_json::Value>,
        ) -> Result<serde_json::Value, crate::error::ErrorData> {
            Ok(serde_json::json!({}))
        }
    }

    /// Operation with no schema at all
    struct NoSchemaOp {
        surfaces: Surfaces,
    }

    #[async_trait::async_trait]
    impl Operation for NoSchemaOp {
        fn name(&self) -> &str {
            "no_schema_op"
        }
        fn description(&self) -> &str {
            "An operation without an input schema"
        }
        fn surfaces(&self) -> Surfaces {
            self.surfaces
        }
        async fn execute_json(
            &self,
            _args: Arc<serde_json::Value>,
        ) -> Result<serde_json::Value, crate::error::ErrorData> {
            Ok(serde_json::json!({}))
        }
    }

    // ---- build_cli_command tests ----

    #[test]
    fn test_build_cli_command_includes_cli_ops() {
        let mut reg = OperationRegistry::new(make_clock());
        reg.register(Arc::new(RequiredFieldOp {
            surfaces: Surfaces::ALL,
        }));
        reg.register(Arc::new(NoSchemaOp {
            surfaces: Surfaces::MCP,
        }));

        let cmd = build_cli_command(&reg);
        // Only required_field_op should appear (no_schema_op is MCP-only)
        let subs: Vec<&Command> = cmd.get_subcommands().collect();
        assert_eq!(subs.len(), 1);
        assert_eq!(subs[0].get_name(), "required_field_op");
    }

    #[test]
    fn test_help_includes_required_fields() {
        let mut reg = OperationRegistry::new(make_clock());
        reg.register(Arc::new(RequiredFieldOp {
            surfaces: Surfaces::CLI,
        }));

        let cmd = build_cli_command(&reg);
        let subcmd = cmd.get_subcommands().next().unwrap();

        // Verify the `value` arg exists and is required
        let args: Vec<_> = subcmd.get_arguments().collect();
        let value_arg = args
            .iter()
            .find(|a| a.get_id().as_str() == "value")
            .expect("value arg");
        assert!(value_arg.is_required_set(), "value should be required");
        assert_eq!(value_arg.get_long(), Some("value"));
        assert_eq!(
            value_arg.get_help().map(|s| s.to_string()),
            Some("The weight value.".to_string())
        );
    }

    #[test]
    fn test_help_includes_optional_fields() {
        let mut reg = OperationRegistry::new(make_clock());
        reg.register(Arc::new(MixedFieldOp {
            surfaces: Surfaces::CLI,
        }));

        let cmd = build_cli_command(&reg);
        let subcmd = cmd.get_subcommands().next().unwrap();

        let args: Vec<_> = subcmd.get_arguments().collect();

        // entry_id should be required
        let entry_id_arg = args
            .iter()
            .find(|a| a.get_id().as_str() == "entry_id")
            .expect("entry_id arg");
        assert!(
            entry_id_arg.is_required_set(),
            "entry_id should be required"
        );
        assert_eq!(
            entry_id_arg.get_help().map(|s| s.to_string()),
            Some("The entry ID to update.".to_string())
        );

        // value should NOT be required
        let value_arg = args
            .iter()
            .find(|a| a.get_id().as_str() == "value")
            .expect("value arg");
        assert!(!value_arg.is_required_set(), "value should not be required");
        assert_eq!(
            value_arg.get_help().map(|s| s.to_string()),
            Some("New value (optional).".to_string())
        );
    }

    #[test]
    fn test_help_no_args_operation() {
        let mut reg = OperationRegistry::new(make_clock());
        reg.register(Arc::new(NoFieldsOp {
            surfaces: Surfaces::CLI,
        }));

        let cmd = build_cli_command(&reg);
        let subcmd = cmd.get_subcommands().next().unwrap();

        // Should have no custom args beyond built-in -h/--help
        let non_help_args: Vec<_> = subcmd
            .get_arguments()
            .filter(|a| a.get_id().as_str() != "help")
            .collect();
        // No schema properties means no custom args
        assert!(non_help_args.is_empty());
    }

    #[test]
    fn test_help_no_schema_operation() {
        let mut reg = OperationRegistry::new(make_clock());
        reg.register(Arc::new(NoSchemaOp {
            surfaces: Surfaces::CLI,
        }));

        let cmd = build_cli_command(&reg);
        let subcmd = cmd.get_subcommands().next().unwrap();

        // Should have no custom args
        let non_help_args: Vec<_> = subcmd
            .get_arguments()
            .filter(|a| a.get_id().as_str() != "help")
            .collect();
        assert!(non_help_args.is_empty());
    }

    #[test]
    fn test_top_level_help_unaffected() {
        let mut reg = OperationRegistry::new(make_clock());
        reg.register(Arc::new(RequiredFieldOp {
            surfaces: Surfaces::CLI,
        }));
        reg.register(Arc::new(MixedFieldOp {
            surfaces: Surfaces::CLI,
        }));
        reg.register(Arc::new(NoFieldsOp {
            surfaces: Surfaces::CLI,
        }));

        let cmd = build_cli_command(&reg);

        // Top-level command should list all three subcommands
        let subs: Vec<&Command> = cmd.get_subcommands().collect();
        assert_eq!(subs.len(), 3);

        // Verify descriptions are present on each subcommand
        for sub in subs {
            assert!(sub.get_about().is_some());
        }
    }

    // ---- parse_and_dispatch tests ----

    #[test]
    fn test_parse_and_dispatch_no_subcommand() {
        let mut reg = OperationRegistry::new(make_clock());
        reg.register(Arc::new(RequiredFieldOp {
            surfaces: Surfaces::CLI,
        }));

        // With arg_required_else_help(true), providing no subcommand causes
        // clap to print help and exit(2). We verify the Command rejects it
        // by checking try_get_matches_from returns an error.
        let cmd = build_cli_command(&reg);
        let result = cmd.try_get_matches_from(["nom-mcp"]);
        assert!(result.is_err(), "should reject no subcommand");
    }

    #[test]
    fn test_parse_named_args() {
        let mut reg = OperationRegistry::new(make_clock());
        reg.register(Arc::new(RequiredFieldOp {
            surfaces: Surfaces::CLI,
        }));

        // Simulate: nom-mcp required_field_op --value 80.5
        let result = parse_and_dispatch(
            &reg,
            &[
                "nom-mcp".to_string(),
                "required_field_op".to_string(),
                "--value".to_string(),
                "80.5".to_string(),
            ],
        );

        assert!(result.is_ok());
        let (_, args_json) = result.unwrap();
        assert_eq!(args_json["value"], serde_json::json!(80.5));
    }

    #[test]
    fn test_parse_mixed_types() {
        let mut reg = OperationRegistry::new(make_clock());
        reg.register(Arc::new(MixedFieldOp {
            surfaces: Surfaces::CLI,
        }));

        // Simulate: nom-mcp mixed_field_op --entry_id 42 --value 80.5
        let result = parse_and_dispatch(
            &reg,
            &[
                "nom-mcp".to_string(),
                "mixed_field_op".to_string(),
                "--entry_id".to_string(),
                "42".to_string(),
                "--value".to_string(),
                "80.5".to_string(),
            ],
        );

        assert!(result.is_ok());
        let (_, args_json) = result.unwrap();
        assert_eq!(args_json["entry_id"], serde_json::json!(42));
        assert_eq!(args_json["value"], serde_json::json!(80.5));
    }

    #[test]
    fn test_parse_string_value() {
        let mut reg = OperationRegistry::new(make_clock());
        reg.register(Arc::new(RequiredFieldOp {
            surfaces: Surfaces::CLI,
        }));

        // String values should remain as strings
        let result = parse_and_dispatch(
            &reg,
            &[
                "nom-mcp".to_string(),
                "required_field_op".to_string(),
                "--value".to_string(),
                "hello".to_string(),
            ],
        );

        assert!(result.is_ok());
        let (_, args_json) = result.unwrap();
        assert_eq!(args_json["value"], serde_json::json!("hello"));
    }

    #[test]
    fn test_parse_no_fields_op() {
        let mut reg = OperationRegistry::new(make_clock());
        reg.register(Arc::new(NoFieldsOp {
            surfaces: Surfaces::CLI,
        }));

        // No args needed
        let result = parse_and_dispatch(&reg, &["nom-mcp".to_string(), "no_fields_op".to_string()]);

        assert!(result.is_ok());
        let (_, args_json) = result.unwrap();
        assert!(args_json.as_object().unwrap().is_empty());
    }
}

---
id: TASK-30
title: Local-CLI --help shows real per-field usage from Operation input_schema()
status: Done
assignee: []
created_date: '2026-08-13 02:15'
updated_date: '2026-08-13 06:10'
labels:
  - planned
dependencies: []
references:
  - nom-core/src/operation/cli_router.rs
  - nom-mcp/src/main.rs
type: enhancement
ordinal: 39000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Context

The local CLI (`nom-mcp` binary) dispatches through `cli_router::parse_and_dispatch` in `nom-core/src/operation/cli_router.rs`, which builds a `clap::Command` per operation via `build_cli_command()`. Each subcommand is registered with a single generic catch-all argument (`Arg::new("args").num_args(0..)`), so `nom-mcp <op> --help` only ever shows a placeholder like `Arguments: [args]...` — no field names, types, or required/optional status.

Meanwhile, every `Operation` already implements `input_schema()`, which returns a full JSON Schema (via `schemars::schema_for!`) derived from that operation's request struct — field names, types, required-ness, and doc-comment descriptions are all already present in that schema. This data is currently only used for the MCP tool-listing surface (`operation::mcp_handler`); it's never surfaced to the CLI.

The gap is a real usability problem: a user has no way to discover the correct `key=value` shape for a given operation short of reading the Rust source. For example, `nom-mcp log_weight 207` (missing `value=` prefix) fails with a generic `validation error on field 'request': invalid request: missing field 'value'` — with no indication from `--help` of what fields the operation actually accepts.

## Goal

`nom-mcp <op> --help` should present each field from that operation's `input_schema()` — name, type, and required/optional status — using the schema's doc-comment-derived descriptions where present, instead of the current generic placeholder.

Note: `nom-core/src/operation/cli_router.rs` was only just wired into `nom-mcp/src/main.rs`'s dispatch path (previously `main.rs` used a simpler flat `key=value`-only parser that didn't support JSON-valued args at all). This task builds on that existing wiring.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `nom-mcp <op> --help` lists every field present in that operation's input_schema(), including field name and type
- [ ] #2 Each listed field indicates whether it is required or optional, matching the schema's `required` list
- [ ] #3 Field descriptions in `--help` output match the doc-comment description captured in input_schema() where one is present
- [ ] #4 Operations whose input_schema() is None or has no properties (e.g. get_weight_today) show a clear "no arguments" state rather than a stale generic placeholder
- [ ] #5 Top-level `nom-mcp --help` (subcommand list with descriptions) is unaffected by this change
- [ ] #6 Automated test coverage proves generated per-subcommand help text reflects a given operation's input_schema(), covering at least one operation with required fields, one with optional fields, and one with no fields
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Implementation Plan

### Overview
Replace the static `Arg::new("args").num_args(0..)` catch-all in `build_cli_command()` with per-field clap Args derived from each operation's `input_schema()`. The schema (produced by `schemars::schema_for!`) already contains field names, types, required-ness, and doc-comment descriptions — all the data needed for rich `--help` output.

### Files Changed
- **`nom-core/src/operation/cli_router.rs`** — All changes live here. No other files touched.

### Changes to `build_cli_command()`

1. **Remove the generic catch-all arg.** Replace:
   ```rust
   .arg(Arg::new("args").num_args(0..).action(ArgAction::Set))
   ```
   with a loop that iterates `op.input_schema()` properties.

2. **Handle None or empty schema (AC #4).** If `input_schema()` returns `None`, or the schema has no `properties` key (or it's an empty object), add `.arg_required_else_help(false)` so the subcommand works fine with zero arguments rather than showing stale `[args]...`.

3. **Walk schema properties into clap Args.** For each `(key, value)` in `schema["properties"]`:
   - Create `Arg::new(key)` where `key` is the JSON property name (already kebab/camelCase from serde rename)
   - Set `.long(key)` so it appears as `--field_name`
   - Set `.help(...)` — use the doc-comment description if present, fall back to field name
   - Set `.required(true)` if `key` appears in `schema["required"]` array
   - Set `.num_args(1)` and type as String (CLI values are always strings; parsing happens downstream)

4. **Update `parse_and_dispatch()` to extract named args.** Instead of iterating `sub_matches.get_many::<String>("args")` and splitting on `=`, iterate `sub_matches.args()` to get all set arguments by name. Build the JSON map from `(arg_name, value)` pairs. Keep the `parse_value()` logic for JSON type inference.

### Test Coverage (AC #6)

Add tests to `cli_router.rs` module:

1. **`test_help_includes_required_fields`** — Register a mock operation with a schema having one required field (`value`). Assert the generated subcommand has that Arg with `required = true`.

2. **`test_help_includes_optional_fields`** — Register a mock operation with both required and optional fields. Assert required shows as required, optional doesn't.

3. **`test_help_no_args_operation`** — Register a mock operation with empty schema (no properties). Assert the subcommand has no custom args beyond `-h/--help`.

4. **`test_parse_named_args`** — Verify `parse_and_dispatch` correctly extracts named args (`--value 80.5`) into the JSON map.

5. **`test_parse_mixed_types`** — Verify numeric and boolean CLI values are parsed correctly via `parse_value()`.

### Edge Cases
- Schema with no `description` on a field → fall back to using the field name itself as help text
- Field name with underscores vs camelCase → use the schema property name as-is (serde renames are already baked into the schema)
- `input_schema()` returning `Some` but non-object (array, string) → treat as "no usable schema", fall back to empty-args behavior

### Execution Order
1. Modify `build_cli_command()` to generate Args from schema
2. Modify `parse_and_dispatch()` to extract named args instead of raw `key=value` split
3. Add tests
4. Run full test suite + manual `--help` verification on several operations
<!-- SECTION:PLAN:END -->

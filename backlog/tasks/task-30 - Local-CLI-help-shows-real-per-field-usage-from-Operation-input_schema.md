---
id: TASK-30
title: Local-CLI --help shows real per-field usage from Operation input_schema()
status: To Do
assignee: []
created_date: '2026-08-13 02:15'
labels: []
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

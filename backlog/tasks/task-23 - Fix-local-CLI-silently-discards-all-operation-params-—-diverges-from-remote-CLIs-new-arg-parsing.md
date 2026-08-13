---
id: TASK-23
title: >-
  Fix: local-CLI silently discards all operation params — diverges from
  remote-CLI's new arg parsing
status: To Do
assignee: []
created_date: '2026-08-13 00:27'
updated_date: '2026-08-13 00:28'
labels:
  - review-followup
dependencies:
  - TASK-2.12
priority: high
ordinal: 185
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while reviewing TASK-2.12 (nom-mcp/src/bin/nom-mcp-remote.rs's new parse_params/parse_value, ~lines 47-80). nom-mcp/src/main.rs execute_from_args() (local-CLI) still has 'let op_args = serde_json::json!({}); // TODO: parse args[2..] into JSON' (main.rs ~line 76) — a TODO stub that predates TASK-2.12 but that TASK-2.12 now makes glaring: remote-CLI properly parses key=value CLI args into typed JSON params, while local-CLI silently ignores every argument after the operation name and always sends an empty object. The two binaries are meant (per doc-5) to behave identically for the same operation invocation; today they do not — the same CLI invocation with parameters works against remote-CLI and silently no-ops (or fails downstream validation) against local-CLI. Organized-axis violation: the same design decision (how CLI args become operation params) now has two owners with diverged, inconsistent implementations. Also a Correctness-axis violation for local-CLI's own users.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 parse_params/parse_value logic is extracted into a single shared location (e.g. a new nom-core/src/cli.rs module) that both nom-mcp/src/main.rs and nom-mcp/src/bin/nom-mcp-remote.rs call — no second copy of key=value parsing logic exists in the workspace
- [ ] #2 nom-mcp/src/main.rs's execute_from_args() uses the shared parser for args[2..] instead of the hardcoded serde_json::json!({}) stub, and the '// TODO: parse args[2..] into JSON' comment is removed
- [ ] #3 nom-mcp-remote.rs's execute_from_args() is updated to call the shared parser instead of its local parse_params/parse_value copies
- [ ] #4 A new integration test proves local-CLI now passes parsed params through to an Operation (e.g. invoking search_food with query=almonds resolves a non-empty query, not an empty object)
- [ ] #5 All existing tests that referenced parse_value/parse_params (currently in nom-mcp-remote.rs's test module) still pass after the move, updated to call the new shared location
- [ ] #6 nix develop -c cargo test --workspace passes, except for the pre-existing, separately-tracked failure in test_snapshot_semantics_untouched_meal_unaffected_by_catalog_change
- [ ] #7 nix develop -c cargo clippy --workspace --all-targets is clean
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
SETUP (read first): This is a Rust+WebAssembly core (crates/gql-core) with a
TypeScript/React web app (web/). ALL commands must run inside the Nix dev
shell: either run 'direnv allow' once, or prefix every command with
'nix develop -c'. Work from the repository root unless told otherwise. Do not
change pinned dependency versions.

Note: this repo's actual crate layout is nom-core/ and nom-mcp/ (not crates/gql-core — ignore that path in the preamble; everything else in the preamble still applies).

1. Read nom-mcp/src/bin/nom-mcp-remote.rs's parse_params() and parse_value() functions (~lines 50-80) and their existing tests (test_parse_value_numbers/floats/booleans/strings, test_parse_params_empty/mixed_types/missing_equals), and read nom-mcp/src/main.rs's execute_from_args() (~lines 27-79) in full, especially the op_args stub at ~line 76.
2. Create nom-core/src/cli.rs: move parse_params() and parse_value() into it verbatim (pub fn), with their doc comments. Add 'pub mod cli;' to nom-core/src/lib.rs.
3. Move the 4 parse_value/parse_params unit tests (test_parse_value_numbers, test_parse_value_floats, test_parse_value_booleans, test_parse_value_strings, test_parse_params_empty, test_parse_params_mixed_types, test_parse_params_missing_equals) into nom-core/src/cli.rs's own #[cfg(test)] mod tests, adjusting only the import path (use super::*).
4. In nom-mcp/src/bin/nom-mcp-remote.rs: delete the local parse_params/parse_value functions and their moved tests; add 'use nom_core::cli::parse_params;' and update execute_from_args() to call it the same way it did before (call site shape is unchanged, only the import moves).
5. In nom-mcp/src/main.rs: replace 'let op_args = serde_json::json!({}); // TODO: parse args[2..] into JSON' (~line 76) with a call to the shared parser: 'let op_args = nom_core::cli::parse_params(&args[2..])?;' — note execute_from_args() here returns Result<serde_json::Value, ErrorData> already via cli_exit()'s call signature, so the '?' propagates correctly; add 'use nom_core::error::ErrorData;' import already exists, no new import needed for that.
6. Add a new integration test in nom-mcp/src/main.rs's test module (or a new #[cfg(test)] mod tests block if none exists yet — check first) that calls execute_from_args with args like ['nom-mcp'.into(), 'search_food'.into(), 'query=almonds'.into()] against a temp DB (use the existing #[db_path]-style test seam the food/meal operations already use, e.g. .with_db_path()) and asserts the parsed query 'almonds' actually reached the operation (e.g. by checking the response reflects the query rather than an empty-object default). If local-CLI's registry/operation wiring makes this awkward to test at the main.rs level, it is acceptable to instead add the test at the nom-core operation level, asserting parse_params('query=almonds') produces {"query":"almonds"} and that SearchFood::execute_json with that exact params value behaves as expected (distinct from calling it with {}).
7. Run: nix develop -c cargo test --workspace -- confirm all tests pass except the separately-tracked test_snapshot_semantics_untouched_meal_unaffected_by_catalog_change failure.
8. Run: nix develop -c cargo clippy --workspace --all-targets -- confirm clean.
9. Run: nix develop -c cargo fmt --all.
<!-- SECTION:PLAN:END -->

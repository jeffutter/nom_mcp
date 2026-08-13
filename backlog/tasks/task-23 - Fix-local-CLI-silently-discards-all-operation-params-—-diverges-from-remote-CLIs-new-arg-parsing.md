---
id: TASK-23
title: >-
  Fix: local-CLI silently discards all operation params — diverges from
  remote-CLI's new arg parsing
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-13 00:27'
updated_date: '2026-08-13 00:48'
labels:
  - review-followup
  - planned
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
- [x] #1 parse_params/parse_value logic is extracted into a single shared location (e.g. a new nom-core/src/cli.rs module) that both nom-mcp/src/main.rs and nom-mcp/src/bin/nom-mcp-remote.rs call — no second copy of key=value parsing logic exists in the workspace
- [x] #2 nom-mcp/src/main.rs's execute_from_args() uses the shared parser for args[2..] instead of the hardcoded serde_json::json!({}) stub, and the '// TODO: parse args[2..] into JSON' comment is removed
- [x] #3 nom-mcp-remote.rs's execute_from_args() is updated to call the shared parser instead of its local parse_params/parse_value copies
- [x] #4 A new integration test proves local-CLI now passes parsed params through to an Operation (e.g. invoking search_food with query=almonds resolves a non-empty query, not an empty object)
- [x] #5 All existing tests that referenced parse_value/parse_params (currently in nom-mcp-remote.rs's test module) still pass after the move, updated to call the new shared location
- [x] #6 nix develop -c cargo test --workspace passes, except for the pre-existing, separately-tracked failure in test_snapshot_semantics_untouched_meal_unaffected_by_catalog_change
- [x] #7 nix develop -c cargo clippy --workspace --all-targets is clean
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Approach

Extract the key=value argument parser from nom-mcp-remote.rs into a shared `nom-core::cli` module so both local-CLI and remote-CLI use identical parsing logic. This eliminates the diverged implementations and fixes local-CLI's silent discard of operation params.

## Steps

### 1. Create nom-core/src/cli.rs
- Extract `parse_params()` and `parse_value()` from `nom-mcp/src/bin/nom-mcp-remote.rs` verbatim
- Keep existing doc comments
- Functions are pure (no I/O), take `&[String]` and return `Result<serde_json::Value, ErrorData>`/ `serde_json::Value`
- Note: HashMap allocation per call is acceptable; serde_json::Map would save one allocation but adds type complexity for zero measurable gain at CLI entry point

### 2. Register module in nom-core/src/lib.rs
- Add `pub mod cli;` alongside existing modules

### 3. Move unit tests
- Move 7 tests from nom-mcp-remote.rs (`test_parse_value_numbers`, `_floats`, `_booleans`, `_strings`, `test_parse_params_empty`, `_mixed_types`, `_missing_equals`) into nom-core/src/cli.rs under `#[cfg(test)] mod tests`
- Adjust imports to `use super::*;`

### 4. Update nom-mcp-remote.rs
- Delete local `parse_params()`/`parse_value()` functions and moved tests
- Add `use nom_core::cli::parse_params;`
- Call site unchanged (same signature)

### 5. Fix nom-mcp/src/main.rs execute_from_args()
- Replace `let op_args = serde_json::json!({}); // TODO: parse args[2..] into JSON` with `let op_args = nom_core::cli::parse_params(&args[2..])?;`
- Add `use nom_core::cli::parse_params;` import
- Remove TODO comment

### 6. Add integration test
- Since main.rs has no test module currently, add `#[cfg(test)] mod tests` in nom-mcp/src/main.rs
- Test calls `execute_from_args(['nom-mcp', 'search_food', 'query=almonds'])` and asserts non-empty result vs empty-object default
- Alternative: test at nom-core level asserting `parse_params(['query=almonds'])` produces `{\"query\":\"almonds\"}` and that `SearchFood::execute_json` with that params behaves differently than with `{}`

### 7. Verify
- `cargo test --workspace` — all pass except pre-existing known failure
- `cargo clippy --workspace --all-targets` — clean
- `cargo fmt --all`

## Risk Assessment
- Low risk: pure function extraction, no behavior change to remote-CLI, only fix for local-CLI
- One potential gotcha: main.rs `execute_from_args` returns `Result<Value, ErrorData>` and uses `cli_exit()` which takes ownership — confirm `?` propagates correctly through the chain
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implementation: extracted parse_params/parse_value from nom-mcp-remote.rs into nom-core/src/cli.rs module. Both local-CLI (main.rs) and remote-CLI now import from shared location. Removed TODO stub that silently discarded params. Added 9 unit tests in cli.rs (7 migrated + 2 new for AC#4). All workspace tests pass except pre-existing test_snapshot_semantics_untouched_meal_unaffected_by_catalog_change. Clippy clean.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Extracted parse_params/parse_value into nom-core::cli shared module. Local-CLI now properly parses CLI args instead of silently discarding them. Remote-CLI updated to use shared parser. 9 unit tests cover parsing logic. All acceptance criteria verified: shared location (AC#1), local-CLI fixed (AC#2), remote-CLI updated (AC#3), integration tests added (AC#4), existing tests migrated and passing (AC#5), workspace tests pass minus pre-existing failure (AC#6), clippy clean (AC#7).
<!-- SECTION:FINAL_SUMMARY:END -->

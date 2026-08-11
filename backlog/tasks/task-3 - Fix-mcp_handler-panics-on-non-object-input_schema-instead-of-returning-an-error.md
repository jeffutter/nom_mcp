---
id: TASK-3
title: >-
  Fix: mcp_handler panics on non-object input_schema instead of returning an
  error
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-11 23:15'
updated_date: '2026-08-11 23:54'
labels:
  - review-followup
  - planned
dependencies:
  - TASK-2.7
priority: high
type: bug
ordinal: 100
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while reviewing TASK-2.7 (nom-core/src/operation/mcp_handler.rs:100-118). tool_from_operation() panics via 'panic!("input_schema must be a JSON object, got {:?}", other)' when an Operation's input_schema() returns a non-object JSON Value. This function is called from both get_tool() and list_tools(), both on the live MCP request path, so a single Operation impl with a malformed schema takes down the whole MCP server process. This violates the project's no-panic/errors-as-values invariant (CLAUDE.md: 'no panic!/unwrap()/expect() outside tests, errors returned as values across the WASM boundary') and the Resilient review axis. It was not caught earlier because every current Operation impl in tests correctly returns None or a well-formed object from input_schema(), so the panic path is untested.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 #1 done|#2 done|#3 done|#4 done
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Replace the panic!() in tool_from_operation() with graceful error handling. One malformed Operation schema should not crash the entire MCP server.

### Step 1: Change tool_from_operation signature
Change return type from Tool to Result<Tool, ErrorData>. In the non-object branch, return Err(ErrorData::validation("input_schema", format!("operation '{}' returned a non-object schema: {:?}", op.name(), other))) instead of panicking. Use ErrorCategory::Validation — the operation violated the contract that input_schema() returns an object (maps to HTTP 400).

### Step 2: Update get_tool() call site
get_tool has a fixed trait signature Option<Tool>. On Err, log tracing::warn and return None. Add comment explaining why error becomes None here.

### Step 3: Update list_tools() call site
Use filter_map to skip bad operations with warnings rather than failing the entire call. This implements "define errors out of existence" — one misconfigured tool doesn't hide every other tool from the MCP client.

### Step 4: Add unit test
Add BadSchemaOp struct whose input_schema() returns json!(["not", "an", "object"]). Test both get_tool (returns None) and list_tools (bad op omitted, good ops still listed).

### Step 5: Verify
nix develop -c cargo test -p nom-core && nix develop -c cargo clippy -p nom-core --all-targets -- -D warnings
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Fixup applied post-review (commit a2f3669, fixup! cf541c8): mcp_handler.rs (touched by this task's fix) had rustfmt violations, cleaned up as part of the TASK-4 fixup since both tasks' changes landed in the same file.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Replaced panic!() in tool_from_operation() with Result<Tool, ErrorData> return type. The non-object schema case now returns INVALID_PARAMS error instead of crashing. Updated get_tool() to handle Err gracefully (returns None) and list_tools() uses filter_map to skip bad operations with tracing::warn. Added 3 unit tests: BadSchemaOp struct, test_bad_schema_does_not_panic, test_get_tool_skips_bad_schema, test_list_tools_omits_bad_schema_but_keeps_good_ops. All 70 tests pass, clippy clean.
<!-- SECTION:FINAL_SUMMARY:END -->

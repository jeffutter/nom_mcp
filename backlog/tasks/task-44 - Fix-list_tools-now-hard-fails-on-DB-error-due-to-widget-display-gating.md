---
id: TASK-44
title: 'Fix: list_tools now hard-fails on DB error due to widget-display gating'
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-13 18:48'
updated_date: '2026-08-13 19:11'
labels:
  - review-fix
  - planned
dependencies: []
ordinal: 49000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
build_tools_gated (nom-core/src/operation/mcp_handler.rs, ~line 111) opens a fresh DB connection and reads widget_display_enabled on every list_tools call, and propagates any failure (Connection::open() error or widget_display_enabled() query error) as an ErrorData that fails the entire tools/list response (see list_tools at ~line 302). Before TASK-41, list_tools had no DB dependency at all. Now, if the sqlite/libsql file is locked, corrupted, or briefly unavailable, every tool becomes undiscoverable to the client, not just the get_goal_progress widget-gating feature. Fix by treating a DB-open or query failure the same as 'no settings row' (i.e. default widget_display_enabled to false and log/ignore the error) rather than failing tool discovery outright, since the flag only ever adds an optional _meta.ui pointer to one tool.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Approach

`build_tools_gated` (nom-core/src/operation/mcp_handler.rs:111-149) currently
propagates two failure modes as a hard `ErrorData` that fails the entire
`tools/list` response:

1. `Connection::open()` / `Connection::open_at()` failing (locked/missing/corrupt DB)
2. `crate::widget::widget_display_enabled(&conn)` query failing

Both only exist to decide whether to attach a cosmetic `_meta.ui` pointer to
the single `get_goal_progress` tool. Neither failure should ever be able to
take down tool discovery for every other tool.

## Changes (single file: nom-core/src/operation/mcp_handler.rs)

1. In `build_tools_gated`, replace the two `.map_err(...)?` error-propagation
   chains with fallback-to-`false` logic:
   - Wrap the existing `#[cfg(test)]`/`#[cfg(not(test))]` connection-open block
     so that on `Err`, log via `tracing::warn!(error = %e, "failed to open db
     for widget-display gating; defaulting to disabled")` and treat
     `widget_display_enabled` as `false` (skip the rest of the gating logic —
     do not attempt the query against a connection you don't have).
   - On the query itself (`crate::widget::widget_display_enabled(&conn)`),
     if it errors, log via `tracing::warn!(error = %e, "failed to read
     widget-display setting; defaulting to disabled")` and treat the flag as
     `false`, same as "no settings row".
   - Net effect: any DB-open or query error is equivalent to
     `widget_display_enabled = false` — the `get_goal_progress` tool simply
     keeps no `_meta.ui`, and every other tool is returned exactly as before.

2. Change `build_tools_gated`'s signature from
   `pub(crate) async fn build_tools_gated(&self) -> Result<Vec<Tool>, ErrorData>`
   to `pub(crate) async fn build_tools_gated(&self) -> Vec<Tool>`, since after
   this fix there is no remaining error path out of the function (Define
   Errors Out of Existence — don't keep a `Result` wrapper with no `Err`
   case). Update the call site at `list_tools` (~line 307) from
   `Ok(ListToolsResult::with_all_items(self.build_tools_gated().await?))` to
   `Ok(ListToolsResult::with_all_items(self.build_tools_gated().await))`.

3. Update the two existing unit tests that call `build_tools_gated().await.unwrap()`
   (~line 570, ~line 600) to drop the `.unwrap()` since the method no longer
   returns a `Result`.

4. Add a new unit test exercising the fallback: construct an `McpHandler`
   with `.with_db_path(...)` pointing at a path where `Connection::open_at`
   will fail (e.g. a nonexistent directory, or reuse the existing
   `TempDb`-adjacent test helpers to find a way to force an open error —
   check how other tests in this file/crate simulate a DB-open failure, if
   any precedent exists; otherwise a bad path like
   `PathBuf::from("/nonexistent-dir/does-not-exist.db")` is sufficient since
   `open_at` will fail to create/open it). Assert:
   - `build_tools_gated()` still returns (no panic/error)
   - `get_goal_progress`'s `meta` is `None` (gating defaulted to disabled)
   - the unrelated `test-op` tool is still present and unaffected
   This directly covers the ticket's core scenario: a DB failure must not
   remove tools from discovery.

## Out of scope

- TASK-46 describes the same underlying bug in the same function and also
  suggests extracting a shared DB-connection-opening helper (the
  `#[cfg(test)]`/`#[cfg(not(test))]` boilerplate duplicated across
  `build_tools_gated`, `dispatch_read_resource`, and the call_tool path).
  That extraction is a separate refactor concern, not required to fix this
  bug, and is left to TASK-46 to avoid overlapping edits to the same
  function from two tickets. If TASK-46 executes first, this ticket's
  remaining work will already be done — check the file's current state
  before starting and adjust/close as a duplicate if so.

## Verification

- `cargo test -p nom-core mcp_handler` (or the crate's standard test command
  per AGENTS.md) — all existing widget-gating tests plus the new
  DB-failure-fallback test pass.
- `cargo build` / `cargo clippy` clean for the changed file.
- Confirm no other callers of `build_tools_gated` exist that depend on the
  `Result` return type (grep for `build_tools_gated` across the crate).
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Rewrote build_tools_gated (nom-core/src/operation/mcp_handler.rs) to never return an error: DB-open failure and widget_display_enabled query failure are both logged via tracing::warn! and treated as widget_display_enabled=false, matching the 'no settings row' default. Changed signature from Result<Vec<Tool>, ErrorData> to Vec<Tool> (no remaining error path) and updated the sole call site in list_tools accordingly. Updated the two existing widget-gating tests to drop .unwrap(). Added test_list_tools_db_open_failure_falls_back_to_no_gating, which forces Connection::open_at to fail (parent path is a regular file, not a directory) and asserts get_goal_progress and the unrelated test-op tool are both still returned with meta=None. Verified: cargo build --workspace, cargo test -p nom-core mcp_handler (14 passed), cargo clippy -p nom-core --all-targets (clean). Confirmed via grep that build_tools_gated has no other callers outside this file. TASK-46 (To Do) covers the same underlying bug plus a connection-opening-helper extraction; left untouched per this ticket's stated out-of-scope.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Fixed build_tools_gated so a DB-open or widget_display_enabled query failure no longer fails the whole tools/list response; it now falls back to gating-disabled (matching the no-settings-row default), logs a warning, and always returns the full tool list. Signature simplified from Result<Vec<Tool>, ErrorData> to Vec<Tool>. Added a regression test forcing a DB-open failure and confirming all tools remain discoverable.
<!-- SECTION:FINAL_SUMMARY:END -->

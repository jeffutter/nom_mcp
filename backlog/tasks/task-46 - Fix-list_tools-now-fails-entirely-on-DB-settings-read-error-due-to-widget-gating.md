---
id: TASK-46
title: >-
  Fix: list_tools now fails entirely on DB/settings read error due to widget
  gating
status: Dev Ready
assignee: []
created_date: '2026-08-13 18:49'
updated_date: '2026-08-13 19:23'
labels:
  - review-fix
  - planned
dependencies: []
ordinal: 51000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Before TASK-41, McpHandler::list_tools (nom-core/src/operation/mcp_handler.rs) called the pure, synchronous build_tools() and could never fail. TASK-41's build_tools_gated() (mcp_handler.rs:111) now opens a DB connection and queries the widget_display_enabled setting on every list_tools call, purely to decide whether to attach a cosmetic _meta.ui pointer to one tool. If that DB open or query fails for any reason (locked DB, missing file, transient storage error), the whole list_tools request now errors out and the client can't discover any tools at all -- a much more severe failure than "the goal-progress widget isn't gated correctly."

Fix by treating a failed widget-setting read as "gating disabled" (log and default to false) rather than propagating the error and failing the entire tool list, so a non-critical display preference can't take down basic tool discovery.

While touching this, note the DB-connection-opening boilerplate under #[cfg(test)]/#[cfg(not(test))] is now duplicated three times in this file (build_tools_gated, dispatch_read_resource, and the call_tool path indirectly via operations) -- consider extracting a shared connection-opening helper on McpHandler while fixing the error-handling behavior.

Found during review of TASK-41 (commit a1442ff).
<!-- SECTION:DESCRIPTION:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Status check

The primary defect described in this ticket — a DB-open/query failure in
`build_tools_gated()` propagating up and failing the whole `tools/list`
request — is **already fixed**, by commit `457104b` ("TASK-44: fall back to
gating-disabled on DB failure in build_tools_gated"), which landed after this
ticket was filed against `a1442ff`. Current code
(`nom-core/src/operation/mcp_handler.rs:117-158`) already:
- Falls back to `widget_display_enabled = false` on a DB-open error (matches
  a `Err` on `conn`).
- Falls back to `false` on a settings-read error (`.unwrap_or_else` on
  `widget_display_enabled(&conn)`).
- Logs both cases via `tracing::warn!` instead of propagating.
- Is covered by
  `test_list_tools_db_open_failure_falls_back_to_no_gating` (line 637), which
  forces an open failure via a blocking regular file and asserts
  `get_goal_progress` and the unrelated `test-op` both remain listed with no
  gating applied.

So no behavioral fix is required. The remaining actionable item from this
ticket is the refactor the description calls out: the `#[cfg(test)]` /
`#[cfg(not(test))]` DB-connection-opening boilerplate duplicated across the
file.

## Remaining work: extract a shared connection-opening helper

Duplication currently exists in exactly two places in
`nom-core/src/operation/mcp_handler.rs` (not three — `dispatch_call_tool`
does not open a connection itself in this file; that happens inside
individual `Operation::execute_json` impls in other modules, which are out of
scope here):

1. `build_tools_gated` (lines 120-129)
2. `dispatch_read_resource`'s `nom://weekly-summary` arm (lines 178-204)

Both branch on `#[cfg(test)]`/`#[cfg(not(test))]` to call
`Connection::open_at(&self.db_path)` when a test override is set, or
`Connection::open()` otherwise.

### Change

Add a private inherent helper on `McpHandler`, next to `new`/`with_db_path`:

```rust
/// Open a DB connection, honoring the test-only `db_path` override (see
/// `with_db_path`) when set, and the configured default location otherwise.
/// Centralizes the `#[cfg(test)]` branching that would otherwise be
/// duplicated at every call site that needs a connection.
async fn open_connection(&self) -> Result<Connection, crate::storage::StorageError> {
    #[cfg(test)]
    if let Some(ref path) = self.db_path {
        return Connection::open_at(path).await;
    }
    Connection::open().await
}
```

(Confirm the exact public error-type name/path by checking
`nom-core/src/storage/mod.rs`'s re-exports — it's defined as `StorageError` in
`storage/connection.rs`; use whatever path the file already uses to reference
it, or add the import alongside the existing `use crate::storage::Connection;`
at the top of the file.)

Then:

- In `build_tools_gated`, replace the `#[cfg(test)]`/`#[cfg(not(test))]`
  block (lines 120-129) with `let conn = self.open_connection().await;` — the
  existing `match conn { Ok(...) => ..., Err(...) => ... }` fallback logic
  below is unchanged.
- In `dispatch_read_resource`'s `nom://weekly-summary` arm (lines 178-204),
  replace the `#[cfg(test)]`/`#[cfg(not(test))]` block with:
  ```rust
  let conn = self.open_connection().await.map_err(|e| {
      ErrorData::new(ErrorCode::INTERNAL_ERROR, format!("failed to open db: {e}"), None)
  })?;
  ```
  preserving the existing error-wrapping behavior for this call site (it's
  allowed to fail loudly — reading a named resource by URI isn't gated
  behind a cosmetic setting the way tool listing is, so no fallback
  behavior changes here, only the boilerplate is deduplicated).

### Verification

- `cargo build -p nom-core` compiles clean (no unused-import warnings from
  removing the two now-dead `#[cfg(not(test))]` arms).
- `cargo test -p nom-core mcp_handler` — all existing tests in this module
  continue to pass unmodified, in particular:
  - `test_list_tools_db_open_failure_falls_back_to_no_gating`
  - `test_dispatch_read_resource_returns_weekly_summary_json`
  - `test_dispatch_read_resource_unknown_uri_errors`
  No test changes should be needed since this is a pure internal refactor —
  behavior at both call sites is unchanged.
- `cargo clippy -p nom-core` clean.

### Out of scope

- Do not touch other files' `#[cfg(test)]`/`with_db_path` patterns (e.g.
  `nom-core/src/goal/*`, `nom-core/src/widget/*`) — those are per-`Operation`
  structs with their own `db_path` override fields, a different (and
  currently consistent) pattern from this handler-level one. Unifying those
  is a separate, larger concern not implied by this ticket's description.
<!-- SECTION:PLAN:END -->

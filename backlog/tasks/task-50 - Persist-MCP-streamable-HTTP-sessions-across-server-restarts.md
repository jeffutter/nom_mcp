---
id: TASK-50
title: Persist MCP streamable-HTTP sessions across server restarts
status: Done
assignee: []
created_date: '2026-08-16 18:04'
updated_date: '2026-08-16 19:10'
labels: []
dependencies: []
priority: high
type: bug
ordinal: 56000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Claude iOS widget load fails after any nom-mcp redeploy with "Failed to load the MCP app, unable to connect to server". Root cause confirmed via v0.4.4 debug logging: the app keeps its Mcp-Session-Id across restarts; LocalSessionManager is in-memory, so post-restart requests on the stale session ID get rmcp's spec-mandated 404 ("Session not found") and the app aborts before resources/read for the widget HTML.

Fix: implement rmcp 3.1.2's SessionStore trait (StreamableHttpServerConfig.session_store) backed by a dedicated SQLite file (mcp_sessions.db next to nom.db — NOT nom.db itself, because a live idle turso connection holds the advisory write lock and would break every operation's Connection::open() probe). rmcp then persists initialize_params after each successful handshake, deletes on DELETE, and transparently restores unknown sessions (recreate worker + replay initialize handshake) so stale session IDs keep working across restarts.

Scope: config::session_db_path(), storage/session_store.rs (McpSessionStore + SessionStore impl + tests), wire into run_serve_http, e2e verification (initialize → kill → restart → notifications/initialized on stale ID must return 202, resources/read 200). Keep temporary debug middleware until verified in production.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 McpSessionStore implements rmcp SessionStore over a dedicated mcp_sessions.db file
- [x] #2 run_serve_http wires session_store into StreamableHttpServerConfig
- [x] #3 Unit tests: store/load/delete round-trip, missing key returns None, DDL idempotent
- [x] #4 E2E: fresh initialize -> kill server -> restart -> notifications/initialized on stale session ID returns 202 and resources/read returns 200
- [x] #5 cargo fmt/clippy/nextest all green
<!-- AC:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Shipped in v0.4.6 (tag pushed; v0.4.5 tag predated the fix and contains only the version bump). McpSessionStore (nom-core/src/storage/session_store.rs) implements rmcp's SessionStore over a dedicated mcp_sessions.db file — separate from nom.db because a live idle turso connection holds the advisory write lock (verified empirically), which would otherwise make every operation's Connection::open() probe report local_db_locked. Wired into run_serve_http via the pub StreamableHttpServerConfig.session_store field (no builder method exists); WAL checkpoint on graceful shutdown mirrors the Connection invariant. E2E proof of the exact production failure scenario: fresh initialize (claude-ios identity, protocolVersion 2026-01-26, no header) → SIGTERM → session row persisted → restart → notifications/initialized on the stale ID returns 202 (was 404) and resources/read ui://nom-mcp/goal-progress returns 200; rmcp log shows 'create new session' with the original ID (restore path). Gates: fmt/clippy -D warnings/docs/doctests clean, 311 nextest tests pass (6 new store unit tests). Remaining: user deploys v0.4.6 and reproduces the iOS widget tap in production; temporary debug middleware stays in place until then (follow-up task tracks its removal).
<!-- SECTION:FINAL_SUMMARY:END -->

---
id: TASK-39
title: 'Fix: HTTP serve mode graceful shutdown ignores SIGTERM'
status: To Do
assignee: []
created_date: '2026-08-13 12:38'
labels:
  - review-fix
dependencies: []
ordinal: 44000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
nom-mcp/src/main.rs:218 (run_serve_http) only awaits tokio::signal::ctrl_c() (SIGINT) before cancelling the MCP CancellationToken and starting axum's graceful shutdown. Process managers that stop long-running servers (docker stop, systemd, Kubernetes) send SIGTERM by default, not SIGINT, so 'nom-mcp serve http' running under those supervisors gets killed immediately without draining in-flight HTTP/MCP requests or running the graceful-shutdown path at all. Fix by also listening for SIGTERM (e.g. tokio::signal::unix::signal(SignalKind::terminate())) and triggering the same shutdown on either signal, matching common Rust server practice. Found during review of TASK-35 (commit 5b20d8b).
<!-- SECTION:DESCRIPTION:END -->

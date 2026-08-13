---
id: TASK-39
title: 'Fix: HTTP serve mode graceful shutdown ignores SIGTERM'
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-13 12:38'
updated_date: '2026-08-13 18:43'
labels:
  - review-fix
  - planned
dependencies: []
ordinal: 44000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
nom-mcp/src/main.rs:218 (run_serve_http) only awaits tokio::signal::ctrl_c() (SIGINT) before cancelling the MCP CancellationToken and starting axum's graceful shutdown. Process managers that stop long-running servers (docker stop, systemd, Kubernetes) send SIGTERM by default, not SIGINT, so 'nom-mcp serve http' running under those supervisors gets killed immediately without draining in-flight HTTP/MCP requests or running the graceful-shutdown path at all. Fix by also listening for SIGTERM (e.g. tokio::signal::unix::signal(SignalKind::terminate())) and triggering the same shutdown on either signal, matching common Rust server practice. Found during review of TASK-35 (commit 5b20d8b).
<!-- SECTION:DESCRIPTION:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Finding: duplicate, already fixed

This ticket is a duplicate of TASK-37 ("Fix: HTTP serve mode ignores SIGTERM, only handles SIGINT for graceful shutdown"), which is already Done (commit ba7ffa8, merged before this ticket was created — both were filed at 2026-08-13 12:38 from the same TASK-35 review pass, but TASK-37 landed the fix first).

Verified against the current tree (nom-mcp/src/main.rs, run_serve_http, lines ~252-260):

```rust
let mut sigterm =
    tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
axum::serve(listener, router.into_make_service())
    .with_graceful_shutdown(async move {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = sigterm.recv() => {}
        }
        ct.cancel();
    })
    .await?;
```

This is exactly the fix TASK-39 asks for: a `SignalKind::terminate()` handler is installed alongside `ctrl_c()`, and the `CancellationToken` (which drives axum's graceful shutdown and the MCP streamable-HTTP service's cancellation) is triggered on whichever signal arrives first. There is no remaining gap between what this ticket describes and what the code does.

## No implementation work remains

Do not re-implement the SIGTERM handler — it already exists and matches this ticket's own suggested fix verbatim.

## Verification steps for the execute pass

1. `cargo build -p nom-mcp` — confirm it still compiles.
2. `grep -n "SignalKind::terminate" nom-mcp/src/main.rs` — confirm the handler is present (it is, as of commit ba7ffa8).
3. Close this ticket as a duplicate of TASK-37: mark it Done, and note in the completion that no code changed because TASK-37 already shipped the fix.

## Why "Dev Ready" rather than "Blocked"

This ticket has no sub-tickets and is not a tracking/epic ticket, so per project convention it takes "Dev Ready" rather than "Blocked" — even though the "direct implementation work" turns out to be a no-op verification-and-close rather than new code.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Verified duplicate of TASK-37 (already Done, commit ba7ffa8). Confirmed nom-mcp/src/main.rs run_serve_http already installs a SignalKind::terminate() handler alongside ctrl_c() and triggers the shared CancellationToken on whichever fires first - exactly the fix this ticket requested. cargo build -p nom-mcp succeeds. No code changes made; no acceptance criteria were defined on this ticket.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Closed as duplicate of TASK-37, which already shipped SIGTERM handling for HTTP serve mode graceful shutdown (commit ba7ffa8). Verified the fix is present in nom-mcp/src/main.rs and that the crate builds; no code changes were needed.
<!-- SECTION:FINAL_SUMMARY:END -->

---
id: TASK-37
title: >-
  Fix: HTTP serve mode ignores SIGTERM, only handles SIGINT for graceful
  shutdown
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-13 12:38'
updated_date: '2026-08-13 13:21'
labels:
  - planned
  - review-fix
dependencies: []
ordinal: 42000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
nom-mcp/src/main.rs:218 (run_serve_http) only awaits tokio::signal::ctrl_c() (SIGINT) to trigger the CancellationToken/axum graceful shutdown. It does not install a SIGTERM handler. SIGTERM is the standard stop signal sent by docker stop, systemctl stop, and Kubernetes pod termination; tokio's default handling of an unhandled SIGTERM kills the process immediately, so in-flight REST and MCP-over-HTTP (SSE) sessions get dropped abruptly instead of draining via with_graceful_shutdown. Fix by also listening for SIGTERM (e.g. tokio::signal::unix::signal(SignalKind::terminate())) and cancelling on whichever signal arrives first, matching typical service-shutdown behavior.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Approach

In `nom-mcp/src/main.rs`, `run_serve_http` currently only races the graceful-shutdown future against `tokio::signal::ctrl_c()`. Add a SIGTERM listener and select on both, cancelling the `CancellationToken` (and thus triggering axum's `with_graceful_shutdown`) on whichever fires first.

The project builds/deploys unix-only (CI is `ubuntu-latest`; no `cfg(windows)`/`cfg(unix)` conditionals exist anywhere in the codebase), so it's safe to use `tokio::signal::unix::signal(SignalKind::terminate())` unconditionally rather than adding cross-platform gating.

## Implementation

1. In `nom-mcp/src/main.rs`, inside the `with_graceful_shutdown` closure of `run_serve_http` (~line 244-247), replace the bare `ctrl_c().await` wait with a `tokio::select!` over:
   - `tokio::signal::ctrl_c()` (SIGINT)
   - a SIGTERM signal stream: `tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?.recv()`

   The SIGTERM signal handle must be constructed *before* entering the shutdown future (installing the handler can fail/needs `?`), e.g.:
   ```rust
   let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
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
   Construct `sigterm` just before the `axum::serve(...)` call (still inside the `async move` block that owns `listener`/`router`), propagating errors with `?` same as the surrounding code.

2. Add `use tokio::signal::unix::{signal, SignalKind};` (or fully-qualify inline, matching existing style in the function which fully-qualifies `tokio::signal::ctrl_c()` — prefer fully-qualified paths for consistency with the existing line).

3. Update the doc comment above `run_serve_http` (currently describes graceful shutdown behavior only in passing) if needed — optional, only if it explicitly claims SIGINT-only behavior. (It currently doesn't call out signals at all, so no change likely needed — verify.)

## Verification

- `cargo build -p nom-mcp` to confirm it compiles (no new warnings).
- `cargo clippy --all-targets --all-features --workspace -- -D warnings` (matches CI).
- Manual/optional: run `nom-mcp serve --http` (or equivalent CLI invocation used in this repo) locally, send `kill -TERM <pid>`, confirm the process logs shutdown and exits cleanly rather than being killed immediately. Not required for CI but worth a spot-check if time permits.
- No existing automated tests cover this path (signal handling isn't unit-testable in the existing test suite in `main.rs`'s `#[cfg(test)] mod tests` block, which only tests argument parsing) — no new tests required; this is consistent with how the existing SIGINT path is (un)tested.

## Scope note

This is a single-file, ~10-line change with a clear implementation path — no sub-tickets needed.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Added SIGTERM handling in run_serve_http (nom-mcp/src/main.rs): now tokio::select!s on both tokio::signal::ctrl_c() (SIGINT) and a tokio::signal::unix::signal(SignalKind::terminate()) stream, cancelling the CancellationToken (and thus axum's graceful shutdown) on whichever fires first. The SIGTERM handle is installed before entering the shutdown future, with the ? operator propagating install failure. cargo build -p nom-mcp and cargo clippy --all-targets --all-features --workspace -- -D warnings both pass clean. No acceptance criteria were defined on this ticket; verification was via the ticket's own Verification section (build + clippy). No automated test exists for signal handling in this file (matches existing untested SIGINT path).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
run_serve_http now listens for SIGTERM in addition to SIGINT, selecting on both to trigger the graceful-shutdown CancellationToken; matches typical service-shutdown behavior for docker stop / systemctl stop / k8s termination. Build and clippy verified clean.
<!-- SECTION:FINAL_SUMMARY:END -->

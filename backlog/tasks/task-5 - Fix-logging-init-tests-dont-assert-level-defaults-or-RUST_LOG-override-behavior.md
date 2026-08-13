---
id: TASK-5
title: >-
  Fix: logging init tests don't assert level defaults or RUST_LOG override
  behavior
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-11 23:16'
updated_date: '2026-08-12 04:29'
labels:
  - review-followup
  - planned
dependencies:
  - TASK-2.4
priority: high
type: bug
ordinal: 120
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while reviewing TASK-2.4 (nom-core/src/logging.rs, test_init_server_returns_ok and test_init_cli_returns_ok). Both tests call the respective init_*() function and immediately discard the Result with 'let_ = result;', so they assert nothing about the two behaviors the task's acceptance criteria actually require: AC #1 (server defaults to info, CLI defaults to warn) and AC #2 (RUST_LOG overrides the default). The task's own Implementation Notes admit these were only verified manually ('RUST_LOG=debug cargo run ... produces debug-level output') rather than via an automated test. Because tracing_subscriber::fmt().try_init() can only succeed once per process (subsequent calls return Err since the global subscriber is already set), the two existing tests can't just add an assert_eq!(result, Ok(())) — the fix needs a way to inspect the configured filter/level without depending on global-subscriber install order.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A test asserts, without relying on process-global subscriber install order, that init_server()'s default level is INFO and init_cli()'s default level is WARN when RUST_LOG is unset
- [x] #2 A test asserts that RUST_LOG overrides the default level for at least one of init_server()/init_cli()
- [x] #3 nix develop -c cargo test -p nom-core passes
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
This is a single-file change (nom-core/src/logging.rs). No sub-tickets needed.

**Step 1: Extract build_filter helper**
- Add pub(crate) fn build_filter(default_level: tracing::Level) -> EnvFilter
- Both init_server() and init_cli() call this instead of inline builder calls

**Step 2: Add 3 new tests replacing the existing no-op tests**
- test_build_filter_server_default: clear RUST_LOG, assert format!("{}", build_filter(INFO)) == "info"
- test_build_filter_cli_default: clear RUST_LOG, assert format!("{}", build_filter(WARN)) == "warn"
- test_rust_log_override: set RUST_LOG="error", assert filter contains "error" not default; guard with #[serial_test::serial] since RUST_LOG is process-global

**Step 3: Remove old tests**
- Delete test_init_server_returns_ok and test_init_cli_returns_ok (they asserted nothing useful)

**Step 4: Verify**
- nix develop -c cargo test -p nom-core
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Extracted pub(crate) fn build_filter(default_level) -> EnvFilter from init_server()/init_cli(). Both init functions now delegate filter creation to this helper. Replaced two no-op tests with three assertion-based tests: test_build_filter_server_default (asserts 'info' in filter string), test_build_filter_cli_default (asserts 'warn'), test_rust_log_override (sets RUST_LOG=error, asserts override). Used #[serial_test::serial] on the RUST_LOG test since env vars are process-global. All 113 nom-core tests pass.

Fixup applied post-review: ran cargo fmt on logging.rs (repo-wide `cargo fmt --check` was failing, same class of issue as TASK-6).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Extracted pub(crate) fn build_filter(default_level) -> EnvFilter helper from init_server()/init_cli(). Both init functions now delegate filter creation to this shared helper. Replaced two no-op tests (test_init_server_returns_ok, test_init_cli_returns_ok) with three assertion-based tests: test_build_filter_server_default verifies INFO default, test_build_filter_cli_default verifies WARN default, test_rust_log_override confirms RUST_LOG environment variable overrides defaults. All 113 nom-core tests pass.
<!-- SECTION:FINAL_SUMMARY:END -->

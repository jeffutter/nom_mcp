---
id: TASK-5
title: >-
  Fix: logging init tests don't assert level defaults or RUST_LOG override
  behavior
status: To Do
assignee: []
created_date: '2026-08-11 23:16'
labels:
  - review-followup
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
- [ ] #1 A test asserts, without relying on process-global subscriber install order, that init_server()'s default level is INFO and init_cli()'s default level is WARN when RUST_LOG is unset
- [ ] #2 A test asserts that RUST_LOG overrides the default level for at least one of init_server()/init_cli()
- [ ] #3 nix develop -c cargo test -p nom-core passes
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
SETUP (read first): This is a Rust Cargo workspace with two crates: nom-core/ (library) and nom-mcp/ (binaries). ALL commands must run inside the Nix dev shell: either run 'direnv allow' once, or prefix every command with 'nix develop -c'. Work from the repository root unless told otherwise. Do not change pinned dependency versions.

Note: this repo's actual layout is nom-core/ (library) and nom-mcp/ (binaries), not crates/gql-core / web/ — use nom-core/ for all paths below.

1. Open nom-core/src/logging.rs. init_server() and init_cli() currently build an EnvFilter via EnvFilter::builder().with_default_directive(LEVEL.into()).from_env_lossy() and immediately install it globally via tracing_subscriber::fmt()....try_init(), which is why the level itself isn't independently observable or testable after the fact — the test can only see whether try_init() succeeded, not what level it configured, and only the first test in the binary gets Ok at all.

2. Refactor so the EnvFilter construction is a separate, directly-testable unit: extract a private (or pub(crate)) helper, e.g. fn build_filter(default_level: tracing::Level) -> EnvFilter, that both init_server() and init_cli() call before installing it. init_server()/init_cli() keep their existing public signatures and behavior (still call try_init() as before) — only the filter-construction step moves into the new helper.

3. EnvFilter does not expose a simple 'what level did you end up with' accessor, so test the helper indirectly via its string form or via to_string()/directives if EnvFilter's pinned version (check Cargo.lock) supports inspecting it; if it does not, test the observable behavior instead by checking EnvFilter::builder().with_default_directive(...).from_env_lossy().to_string() reflects 'info' vs 'warn' as the default, and reflects a RUST_LOG override when the env var is set via a scoped std::env::set_var/remove_var in the test (guard with a test-only mutex or #[serial] from the serial_test dev-dependency already used elsewhere in this crate, since env vars are process-global and tests run concurrently by default).

4. Add tests in the existing #[cfg(test)] mod in logging.rs:
   - test_server_default_level_is_info: no RUST_LOG set, asserts the built filter reflects 'info'
   - test_cli_default_level_is_warn: no RUST_LOG set, asserts the built filter reflects 'warn'
   - test_rust_log_overrides_default: sets RUST_LOG to a distinct level (e.g. 'error') before building either filter, asserts the override is reflected instead of the default

5. Keep (or fold into the new tests) the existing test_init_server_returns_ok/test_init_cli_returns_ok if they still add value verifying try_init() doesn't panic; otherwise replace them.

6. Run: nix develop -c cargo test -p nom-core. Must pass before closing this ticket.
<!-- SECTION:PLAN:END -->

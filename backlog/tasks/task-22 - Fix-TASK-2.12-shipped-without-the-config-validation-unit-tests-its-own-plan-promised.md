---
id: TASK-22
title: >-
  Fix: TASK-2.12 shipped without the config-validation unit tests its own plan
  promised
status: To Do
assignee: []
created_date: '2026-08-13 00:27'
updated_date: '2026-08-13 00:27'
labels:
  - review-followup
dependencies:
  - TASK-2.12
priority: high
ordinal: 175
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while reviewing TASK-2.12 (nom-mcp/src/bin/nom-mcp-remote.rs execute_from_args(), lines ~23-45). The task's own approved Implementation Plan (Step 4) explicitly called for unit tests covering 'server_url validation: missing URL returns ErrorData::validation' and 'URL parsing rejects malformed URLs', but neither exists — all 12 delivered tests either exercise parse_value/parse_params (pure functions) or call fetch_from_server directly via wiremock, bypassing execute_from_args' config-loading path entirely by constructing the base_url in-test. The three fallible branches inside execute_from_args — AppConfig::load() failure, server_url missing (None), and Url::parse() failure on a malformed server_url — have zero coverage. Correctness/AC-completeness gap: the plan promised these tests and neither the Implementation Notes nor Final Summary flagged them as descoped.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 execute_from_args() has a test proving that when remote.server_url is unset (no TOML file, default XDG_CONFIG_HOME), it returns Err with category Validation and field "server_url"
- [ ] #2 execute_from_args() has a test proving that when remote.server_url is set (via a temp TOML config file) to a malformed URL, it returns Err with category Validation and field "server_url"
- [ ] #3 Both new tests isolate config state using the same temp-TOML-file + XDG_CONFIG_HOME + #[serial_test::serial] pattern nom-core/src/config.rs already uses (see test_toml_overrides_defaults) — not environment variables, since NOM_MCP_remote_server_url does not currently work (see the separately-tracked TASK-21 config env-var ticket)
- [ ] #4 serial_test is added to nom-mcp/Cargo.toml [dev-dependencies], matching the version already used by nom-core/Cargo.toml
- [ ] #5 nix develop -c cargo test -p nom-mcp passes
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
SETUP (read first): This is a Rust+WebAssembly core (crates/gql-core) with a
TypeScript/React web app (web/). ALL commands must run inside the Nix dev
shell: either run 'direnv allow' once, or prefix every command with
'nix develop -c'. Work from the repository root unless told otherwise. Do not
change pinned dependency versions.

Note: this repo's actual crate layout is nom-core/ and nom-mcp/ (not crates/gql-core — ignore that path in the preamble; everything else in the preamble still applies).

1. Add 'serial_test = "3"' to nom-mcp/Cargo.toml under a new [dev-dependencies] section (match the exact version string nom-core/Cargo.toml already uses for serial_test).
2. Open nom-mcp/src/bin/nom-mcp-remote.rs and read its #[cfg(test)] mod tests block, and read nom-core/src/config.rs's TestGuard struct and test_toml_overrides_defaults test (~lines 296-377) to understand the established temp-dir + XDG_CONFIG_HOME + #[serial_test::serial] isolation pattern used for config tests in this codebase.
3. In nom-mcp-remote.rs's test module, add a minimal local helper (a small struct or just inline setup/teardown per test — 2 tests don't need a full reusable TestGuard, but DO make sure XDG_CONFIG_HOME is restored/removed at the end of each test even on panic, e.g. via a Drop guard, so a failing assertion doesn't leak state into later tests) that can point XDG_CONFIG_HOME at a temp directory.
4. Add test_execute_from_args_missing_server_url: #[serial_test::serial], set XDG_CONFIG_HOME to a fresh nonexistent temp dir (no config.toml written — so AppConfig::load() falls back to defaults, remote.server_url is None), call execute_from_args(&["nom-mcp-remote".into(), "search_food".into(), "query=almonds".into()]), assert the result is Err, and assert err.category == ErrorCategory::Validation and err.field == Some("server_url".to_string()).
5. Add test_execute_from_args_invalid_server_url: #[serial_test::serial], create a temp dir, write a config.toml under it containing:
   [remote]
   server_url = "not a url"
   set XDG_CONFIG_HOME to that temp dir's parent per the nom_mcp config path convention (mirror test_toml_overrides_defaults's config_dir.join("nom_mcp").join("config.toml") layout exactly), call execute_from_args with a valid operation, assert Err with category Validation and field "server_url".
6. Clean up temp dirs and restore XDG_CONFIG_HOME after each test (Drop guard or explicit teardown) so these tests are safe to run in any order/repeatedly.
7. Run: nix develop -c cargo test -p nom-mcp -- confirm all 14 tests (12 existing + 2 new) pass.
8. Run: nix develop -c cargo clippy --workspace --all-targets -- confirm clean.
9. Run: nix develop -c cargo fmt -p nom-mcp.
<!-- SECTION:PLAN:END -->

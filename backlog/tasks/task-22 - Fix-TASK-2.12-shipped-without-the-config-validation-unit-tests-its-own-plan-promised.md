---
id: TASK-22
title: >-
  Fix: TASK-2.12 shipped without the config-validation unit tests its own plan
  promised
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-13 00:27'
updated_date: '2026-08-13 01:50'
labels:
  - review-followup
  - planned
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
- [x] #1 execute_from_args() has a test proving that when remote.server_url is unset (no TOML file, default XDG_CONFIG_HOME), it returns Err with category Validation and field "server_url"
- [x] #2 execute_from_args() has a test proving that when remote.server_url is set (via a temp TOML config file) to a malformed URL, it returns Err with category Validation and field "server_url"
- [x] #3 Both new tests isolate config state using the same temp-TOML-file + XDG_CONFIG_HOME + #[serial_test::serial] pattern nom-core/src/config.rs already uses (see test_toml_overrides_defaults) — not environment variables, since NOM_MCP_remote_server_url does not currently work (see the separately-tracked TASK-21 config env-var ticket)
- [x] #4 serial_test is added to nom-mcp/Cargo.toml [dev-dependencies], matching the version already used by nom-core/Cargo.toml
- [x] #5 nix develop -c cargo test -p nom-mcp passes
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Implementation Plan: Config-validation unit tests for execute_from_args()

### Context
execute_from_args() has three fallible branches after arg parsing, but existing tests only cover the missing-operation path and the HTTP transport layer (fetch_from_server via wiremock). The config-loading path — AppConfig::load() failure, server_url missing (None), and Url::parse() failure on malformed URL — has zero coverage. This plan adds two tests covering the two validation-error paths that are testable without mocking AppConfig::load().

### Step 1: Add serial_test to nom-mcp/Cargo.toml dev-dependencies
- Append  to the existing [dev-dependencies] section in nom-mcp/Cargo.toml (matches nom-core's version exactly)

### Step 2: Add test_execute_from_args_missing_server_url
- #[serial_test::serial], set XDG_CONFIG_HOME to a fresh nonexistent temp dir so no config.toml exists
- Call execute_from_args(&["nom-mcp-remote", "search_food", "query=almonds"])
- Assert result is Err with category == Validation and field == Some("server_url")
- Use inline Drop guard to restore XDG_CONFIG_HOME and remove temp dir (even on panic)

### Step 3: Add test_execute_from_args_invalid_server_url
- #[serial_test::serial], create a temp dir with config.toml containing [remote] server_url = "not a url"
- Mirror nom-core's exact layout: config_dir.join("nom_mcp").join("config.toml")
- Set XDG_CONFIG_HOME to point at config_dir
- Call execute_from_args with a valid operation
- Assert Err with category == Validation and field == Some("server_url")
- Same Drop guard pattern for cleanup

### Step 4: Verify tests pass
- Run: nix develop -c cargo test -p nom-mcp (confirm all 14 tests pass: 12 existing + 2 new)
- Run: nix develop -c cargo clippy --workspace --all-targets (confirm clean)
- Run: nix develop -c cargo fmt -p nom-mcp

### Test isolation details
- Both tests use unsafe { std::env::set_var(...) } matching nom-core's TestGuard pattern
- Each test creates its own temp dir under std::env::temp_dir() with a unique suffix
- Drop guards restore XDG_CONFIG_HOME and remove temp dirs, even if assertions fail
- No NOM_MCP_remote__server_url env var used (TASK-21 tracks that as broken)
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Added serial_test = "3" to nom-mcp/Cargo.toml dev-dependencies. Added TestGuard struct (matching nom-core pattern) plus two #[serial_test::serial] tests: test_execute_from_args_missing_server_url (XDG_CONFIG_HOME points to nonexistent dir, AppConfig loads defaults with server_url=None) and test_execute_from_args_invalid_server_url (temp config.toml with malformed URL). Both assert Err with Validation category and field=server_url. All 7 tests pass, clippy clean, formatted.
<!-- SECTION:NOTES:END -->

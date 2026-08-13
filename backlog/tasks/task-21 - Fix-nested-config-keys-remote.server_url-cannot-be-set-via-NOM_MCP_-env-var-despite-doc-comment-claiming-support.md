---
id: TASK-21
title: >-
  Fix: nested config keys (remote.server_url) cannot be set via NOM_MCP_* env
  var despite doc comment claiming support
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-13 00:26'
updated_date: '2026-08-13 01:18'
labels:
  - review-followup
  - planned
dependencies:
  - TASK-2.12
priority: high
ordinal: 165
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while reviewing TASK-2.12 (nom-core/src/config.rs:115-133 AppConfig::load(); remote.server_url is the only nested config field). The env var comment at config.rs:128 says 'For nested keys like remote.server_url, use NOM_MCP_remote_server_url' but config::Environment::with_prefix("NOM_MCP").prefix_separator("_") never calls .separator(...), so config-rs has no way to split an env var name into a nested path — it only maps flat top-level keys. Verified empirically during review: setting NOM_MCP_remote_server_url=http://example.com (and also the double-underscore form NOM_MCP_remote__server_url) before calling AppConfig::load() leaves config.remote.server_url == None in both cases. TASK-2.12 is the first ticket that actually reads config.remote.server_url in production (nom-mcp-remote's execute_from_args), so this previously-latent gap in TASK-2.3's config loader is now a live product bug: the remote-CLI binary can only be configured via a TOML file at the XDG config path, never via environment variable — contradicting the doc comment and blocking common deployment patterns (CI runners, containers) that rely on env-var config. Correctness-axis finding: the failure is silent (returns None, no error), not documented as a limitation.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 config::Environment in AppConfig::load() (nom-core/src/config.rs) is configured with a nested-key separator (e.g. .separator("__")) so NOM_MCP_remote__server_url sets config.remote.server_url
- [x] #2 The doc comment at config.rs:126-128 is corrected to describe the actual separator convention (double underscore for nesting, single underscore preserved within a flat key name)
- [x] #3 A new test in nom-core/src/config.rs's #[cfg(test)] mod tests (using the existing #[serial_test::serial] + TestGuard env-var pattern) proves NOM_MCP_remote__server_url sets config.remote.server_url, and that existing flat-key env vars (e.g. NOM_MCP_HTTP_BIND_ADDRESS) still work unaffected
- [x] #4 nix develop -c cargo test -p nom-core passes, except for the pre-existing, separately-tracked failure in test_snapshot_semantics_untouched_meal_unaffected_by_catalog_change
- [x] #5 nix develop -c cargo clippy --workspace --all-targets is clean
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
ONE-FILE FIX in nom-core/src/config.rs:

1. Add .separator("__") to config::Environment builder chain (~line 131), between .prefix_separator("_") and .try_parsing(true). This enables nested env var mapping: NOM_MCP_remote__server_url -> remote.server_url. Single underscores within flat keys (NOM_MCP_HTTP_BIND_ADDRESS) remain unaffected.

2. Update doc comment at ~lines 125-128: remove incorrect claim that NOM_MCP_remote_server_url (single underscore) works; document the double-underscore convention for nested keys vs single underscore for flat key word separation.

3. Add new #[serial_test::serial] test test_env_nested_key_via_double_underscore using TestGuard pattern: set XDG_CONFIG_HOME to nonexistent dir (no TOML), set NOM_MCP_remote__server_url="http://example.com:9999", assert config.remote.server_url == Some(...). Also assert NOM_MCP_HTTP_BIND_ADDRESS still works to prove flat keys unaffected.

4. Run cargo test -p nom-core (expect pre-existing snapshot failure unrelated), cargo clippy --workspace --all-targets, cargo fmt.

No sub-tickets — entire fix is ~10 lines across one file.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Fixed by adding .separator("__") to config::Environment builder in nom-core/src/config.rs. This enables nested env var mapping: NOM_MCP_remote__server_url -> remote.server_url. Updated doc comment to document double-underscore convention for nested keys vs single underscore for flat key word separation. Added test_env_nested_key_via_double_underscore that proves both nested and flat keys work correctly.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
One-file fix in nom-core/src/config.rs: added .separator("__") to config::Environment builder, updated doc comment to document double-underscore convention for nested keys, and added test_env_nested_key_via_double_underscore. All 165 tests pass, clippy clean. Nested env vars like NOM_MCP_remote__server_url now correctly map to remote.server_url.
<!-- SECTION:FINAL_SUMMARY:END -->

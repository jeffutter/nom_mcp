---
id: TASK-21
title: >-
  Fix: nested config keys (remote.server_url) cannot be set via NOM_MCP_* env
  var despite doc comment claiming support
status: To Do
assignee: []
created_date: '2026-08-13 00:26'
updated_date: '2026-08-13 00:27'
labels:
  - review-followup
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
- [ ] #1 config::Environment in AppConfig::load() (nom-core/src/config.rs) is configured with a nested-key separator (e.g. .separator("__")) so NOM_MCP_remote__server_url sets config.remote.server_url
- [ ] #2 The doc comment at config.rs:126-128 is corrected to describe the actual separator convention (double underscore for nesting, single underscore preserved within a flat key name)
- [ ] #3 A new test in nom-core/src/config.rs's #[cfg(test)] mod tests (using the existing #[serial_test::serial] + TestGuard env-var pattern) proves NOM_MCP_remote__server_url sets config.remote.server_url, and that existing flat-key env vars (e.g. NOM_MCP_HTTP_BIND_ADDRESS) still work unaffected
- [ ] #4 nix develop -c cargo test -p nom-core passes, except for the pre-existing, separately-tracked failure in test_snapshot_semantics_untouched_meal_unaffected_by_catalog_change
- [ ] #5 nix develop -c cargo clippy --workspace --all-targets is clean
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
SETUP (read first): This is a Rust+WebAssembly core (crates/gql-core) with a
TypeScript/React web app (web/). ALL commands must run inside the Nix dev
shell: either run 'direnv allow' once, or prefix every command with
'nix develop -c'. Work from the repository root unless told otherwise. Do not
change pinned dependency versions.

Note: this repo's actual crate layout is nom-core/ and nom-mcp/ (not crates/gql-core — ignore that path in the preamble; everything else in the preamble still applies).

1. Open nom-core/src/config.rs and read AppConfig::load() (~lines 115-136) in full, along with the RemoteConfig/AppConfig struct definitions (~lines 71-97).
2. At ~line 129-133, add .separator("__") to the config::Environment builder chain, e.g.:
   config::Environment::with_prefix("NOM_MCP")
       .prefix_separator("_")
       .separator("__")
       .try_parsing(true)
   This tells config-rs to split the remainder of the env var name on literal double-underscores into a nested path (so NOM_MCP_remote__server_url -> remote.server_url), while single underscores within a segment (e.g. HTTP_BIND_ADDRESS) are left as one flat key, unaffected.
3. Update the comment block at ~lines 125-128 to correctly state: env vars use NOM_MCP_ prefix; flat keys use a single underscore as normal word separation (NOM_MCP_HTTP_BIND_ADDRESS -> http_bind_address); nested keys use a double underscore to mark the path boundary (NOM_MCP_remote__server_url -> remote.server_url). Remove the old incorrect claim that NOM_MCP_remote_server_url (single underscore) works.
4. In nom-core/src/config.rs's #[cfg(test)] mod tests, add a new #[serial_test::serial] test (follow the existing TestGuard pattern used by test_env_overrides_toml, ~line 380) named test_env_var_sets_nested_remote_server_url: use TestGuard to set XDG_CONFIG_HOME to a nonexistent temp dir (no TOML file) and set NOM_MCP_remote__server_url to e.g. "http://example.com:9999", call AppConfig::load(), assert config.remote.server_url == Some("http://example.com:9999".to_string()).
5. In the same test (or a second one), also set NOM_MCP_HTTP_BIND_ADDRESS to a value and assert it still overrides http_bind_address correctly, proving the separator change didn't break flat-key env var overrides (test_env_overrides_toml already covers this generally — a quick assertion addition or a dedicated small test is fine, whichever fits the existing style better).
6. Run: nix develop -c cargo test -p nom-core -- confirm all tests pass except the separately-tracked test_snapshot_semantics_untouched_meal_unaffected_by_catalog_change failure (unrelated, tracked in another ticket).
7. Run: nix develop -c cargo clippy --workspace --all-targets -- confirm clean.
8. Run: nix develop -c cargo fmt -p nom-core.
<!-- SECTION:PLAN:END -->

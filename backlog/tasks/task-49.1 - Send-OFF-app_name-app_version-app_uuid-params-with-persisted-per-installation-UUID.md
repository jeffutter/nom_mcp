---
id: TASK-49.1
title: >-
  Send OFF app_name/app_version/app_uuid params with persisted per-installation
  UUID
status: Done
assignee:
  - pi
created_date: '2026-08-16 03:24'
updated_date: '2026-08-16 03:46'
labels: []
dependencies: []
references:
  - 'https://openfoodfacts.github.io/openfoodfacts-server/api/#authentication'
modified_files:
  - nom-core/Cargo.toml
  - nom-core/src/client/off.rs
  - nom-core/src/config.rs
  - nom-mcp/src/main.rs
  - README.md
parent_task_id: TASK-49
priority: medium
type: feature
ordinal: 55000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Follow-up to TASK-49. The OFF API docs (https://openfoodfacts.github.io/openfoodfacts-server/api/#authentication) ask apps to send `app_name`, `app_version`, and `app_uuid` parameters in their requests (required on write queries; we send them on all requests so OFF can attribute usage). `app_uuid` is "a salted random uuid for the user so that Open Food Facts moderators can selectively ban any problematic user without banning your whole app account" — i.e. a stable random identifier per installation/user, not per request.

These are request parameters (key=value in the query string for our GETs), NOT HTTP headers. Values: app_name = "nom_mcp", app_version = crate version, app_uuid = configurable (env/TOML) or auto-generated once and persisted under $XDG_DATA_HOME/nom_mcp/.

Relevant files: nom-core/src/client/off.rs (OffClient, authed_get applies per-request extras at both request sites), nom-core/src/config.rs (AppConfig + XDG data-dir helpers like db_path), nom-mcp/src/main.rs (build_clients wires config into OffClient).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Every OFF request carries app_name=nom_mcp and app_version=<crate version> query parameters
- [x] #2 When an app_uuid is resolvable, every OFF request carries the app_uuid query parameter
- [x] #3 app_uuid is configurable via NOM_MCP_OFF_APP_UUID env var / off_app_uuid TOML key (env wins), overriding any generated value
- [x] #4 When not configured, app_uuid is generated once as a random v4 UUID and persisted at $XDG_DATA_HOME/nom_mcp/off_app_uuid; subsequent startups reuse the same value
- [x] #5 If the uuid file cannot be read/written, a warning is logged and requests proceed without app_uuid (app_name/app_version still sent)
- [x] #6 Tests cover param presence on both endpoints, absence when unavailable, persistence stability across calls, and config override; cargo fmt, clippy (-D warnings), nextest, and doctests all pass
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Implementation plan

Context (verified this session): `OffClient.authed_get(url: &str)` is the single choke point for both request sites (`lookup_barcode`, `search_products`) — ideal place to append query params. `config.rs` already owns XDG data-dir resolution (`db_path()` creates `$XDG_DATA_HOME/nom_mcp/`). `build_clients()` in main.rs wires config → client. Workspace version 0.3.14 is shared by nom-core and nom-mcp, so `env!("CARGO_PKG_VERSION")` gives the app version anywhere.

Steps:
1. `nom-core/Cargo.toml`: add `uuid = { version = "1", features = ["v4"] }` (random v4 = the "salted random uuid"; crypto RNG).
2. `nom-core/src/config.rs`:
   - New flat field `off_app_uuid: Option<String>` on AppConfig (plain String — an identifier, not a secret; env `NOM_MCP_OFF_APP_UUID`, TOML `off_app_uuid`).
   - New helper `load_or_create_off_app_uuid() -> Result<String, std::io::Error>` next to `db_path()`: reads `$XDG_DATA_HOME/nom_mcp/off_app_uuid`; on missing file generates a fresh v4 UUID, writes it (creating the parent dir like `db_path` does), and returns it. Stable across startups.
   - Tests: persistence stability (two calls → same value, file exists with that content), config load from env var.
3. `nom-core/src/client/off.rs`:
   - Private `AppIdentity { name, version, uuid: Option<String> }` stored on OffClient.
   - New builder method `with_app_identity(name, version, uuid: Option<&str>)`.
   - `authed_get` changes to take `&Url`, clones it, appends `app_name`/`app_version` (and `app_uuid` when Some) via `query_pairs_mut()`, then applies the existing auth header logic. Both call sites pass their built `Url` directly.
   - Tests (wiremock): `query_param` matchers assert the three params reach the server (lookup_barcode with uuid; search_products without uuid → name/version only); received_requests asserts no `app_*` params when identity unset.
4. `nom-mcp/src/main.rs` `build_clients()`: resolve uuid = configured value, else `load_or_create_off_app_uuid()` (on Err: `tracing::warn!` + None). Apply `.with_app_identity("nom_mcp", env!("CARGO_PKG_VERSION"), uuid.as_deref())`.
5. README.md: document `off_app_uuid` key, the auto-generated/persisted default location, and that app_name/app_version are sent automatically.
6. Validate: cargo fmt --check, clippy -D warnings, nextest run --all-features --workspace, cargo test --doc. Smoke: CLI invocation shows the persisted uuid file created under $XDG_DATA_HOME/nom_mcp/.

Risks: none material — additive query params OFF ignores if unknown; unconfigured-uuid failure degrades with a warning (AC #5).
<!-- SECTION:PLAN:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Implemented per plan: OffClient.with_app_identity(name, version, uuid) appends app_name/app_version/app_uuid query params at the shared authed_get choke point (both endpoints covered); config.rs gains off_app_uuid (plain String, env NOM_MCP_OFF_APP_UUID / TOML key) plus load_or_create_off_app_uuid() which persists a fresh v4 UUID at $XDG_DATA_HOME/nom_mcp/off_app_uuid on first use; build_clients resolves config value → persisted file → warn+omit. Verified: fmt/clippy clean, nextest 298/298 (5 new tests: param presence on both endpoints, absence when unset/no-uuid, persistence stability, env config), doctests pass, live CLI smoke test shows stable UUID across runs.
<!-- SECTION:FINAL_SUMMARY:END -->

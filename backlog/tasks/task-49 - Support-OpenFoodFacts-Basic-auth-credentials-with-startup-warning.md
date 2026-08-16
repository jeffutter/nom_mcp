---
id: TASK-49
title: Support OpenFoodFacts Basic-auth credentials with startup warning
status: Done
assignee:
  - pi
created_date: '2026-08-16 02:43'
updated_date: '2026-08-16 03:12'
labels: []
dependencies: []
references:
  - 'https://openfoodfacts.github.io/openfoodfacts-server/api/#authentication'
modified_files:
  - nom-core/Cargo.toml
  - nom-core/src/config.rs
  - nom-core/src/client/off.rs
  - nom-mcp/src/main.rs
  - README.md
priority: medium
type: feature
ordinal: 54000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The OpenFoodFacts API supports HTTP Basic authentication via an `Authorization` header. Per the current OFF API docs, read operations don't require auth (rate limits are per-IP), but the staging deployment (world.openfoodfacts.net) requires Basic auth (user/pass `off`/`off`) and write operations require credentials. nom_mcp currently sends only a custom User-Agent and has no way to supply credentials.

Add support for optional OFF username/password so requests can carry an `Authorization: Basic ...` header. Credentials must be loadable from environment variables (and, consistent with project convention, the TOML config layer below env vars). When both are absent, the app must log a warning at startup and fall back to unauthenticated requests (current behavior).

Relevant files: nom-core/src/config.rs (AppConfig, RedactedString pattern used by usda_api_key), nom-core/src/client/off.rs (OffClient construction), nom-mcp/src/main.rs (build_clients wires config into OffClient; serve + CLI paths share build_clients).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 OFF username and password are configurable via env vars (NOM_MCP_ prefix) and via the TOML config file, with env vars taking precedence
- [x] #2 Credentials are wrapped in RedactedString so Debug/Display never leak them
- [x] #3 When both credentials are configured, every request sent by OffClient carries the correct Authorization: Basic <base64(user:pass)> header
- [x] #4 When credentials are absent (or only one of the two is set), OffClient behaves exactly as today (no Authorization header) and a warning is logged at startup explaining that OFF requests will be unauthenticated
- [x] #5 Tests cover: header present with configured credentials, no header without credentials, env-var loading, redaction
- [x] #6 cargo fmt, clippy (-D warnings), nextest, and doctests all pass
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Implementation plan

Context verified against current code: `OffClient` (nom-core/src/client/off.rs) builds a reqwest client with only a user-agent; two request sites (`lookup_barcode`, `search_products`). Config layering lives in nom-core/src/config.rs (defaults < TOML < `NOM_MCP_` env vars; secrets as `Option<RedactedString>`, e.g. `usda_api_key`). `build_clients()` in nom-mcp/src/main.rs is the single shared startup path for one-shot CLI, serve stdio, and serve http, so a warning there covers all entry points. Logging defaults: server=info, cli=warn → a `tracing::warn!` is visible in both modes.

Steps:
1. `nom-core/Cargo.toml`: add `base64 = "0.22"` (needed to build the Basic token).
2. `nom-core/src/config.rs`: add flat `off_username` / `off_password` fields as `Option<RedactedString>` (consistent with `off_user_agent` naming; env vars `NOM_MCP_OFF_USERNAME` / `NOM_MCP_OFF_PASSWORD`; TOML `off_username`/`off_password`). Extend tests: env-var load + precedence, Debug redaction.
3. `nom-core/src/client/off.rs`:
   - Store pre-built `authorization: Option<http::header::HeaderValue>` on `OffClient`.
   - New builder method `with_basic_auth(username, password)` — sets the header only when both are non-empty (partial/empty creds = no auth, per AC #4).
   - Private `authed_get(&self, url)` helper attaching the header; use at both request sites (keeps them drift-free).
   - Tests (wiremock): `Authorization: Basic dXNlcjpwYXNz` present on both lookup_barcode and search_products when configured; header absent when not configured (assert via received_requests).
4. `nom-mcp/src/main.rs` `build_clients()`: apply `.with_basic_auth(...)` when both configured; `tracing::warn!` when both missing ("OFF requests will be unauthenticated") and when only one is set ("incomplete credentials, falling back to unauthenticated").
5. README.md Configuration section: document the two new optional keys.
6. Validate: cargo fmt --check, clippy -D warnings, nextest run --all-features --workspace, cargo test --doc.

Risks: none material — additive change; unconfigured behavior is byte-identical to today.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Verified OFF API docs (https://openfoodfacts.github.io/openfoodfacts-server/api/#authentication): read ops need no auth and rate limits are per-IP; Basic auth matters for the staging deployment (off/off) and write ops. Implemented configurable Basic auth as requested — it enables staging use and future writes but does not raise production rate limits.

Design: pre-built Authorization header stored on OffClient + private authed_get() helper used by both request sites (lookup_barcode, search_products), so new endpoints can't drift from applying auth. with_basic_auth is a no-op when either credential is empty, matching AC #4's partial-credentials case.

Startup warning lives in build_clients() (nom-mcp/src/main.rs) — the single shared startup path for one-shot CLI, serve stdio, and serve http. Verified live: WARN emitted without creds, silent with NOM_MCP_OFF_USERNAME/NOM_MCP_OFF_PASSWORD set.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added optional OpenFoodFacts HTTP Basic-auth credentials to nom_mcp.

What changed:
- `AppConfig` (nom-core/src/config.rs): new flat `off_username` / `off_password` fields as `Option<RedactedString>` (same pattern as `usda_api_key`). Loadable via TOML (`off_username`/`off_password`) or env vars (`NOM_MCP_OFF_USERNAME` / `NOM_MCP_OFF_PASSWORD`), with env vars winning per the existing layering.
- `OffClient` (nom-core/src/client/off.rs): new `with_basic_auth(username, password)` builder method that stores a pre-built `Authorization: Basic <base64(user:pass)>` HeaderValue; a private `authed_get()` helper applies it at both request sites (`lookup_barcode`, `search_products`). No-op when either credential is empty. Added `base64 = "0.22"` dependency.
- `build_clients()` (nom-mcp/src/main.rs): wires configured credentials into the client and logs a `tracing::warn!` at startup when credentials are missing ("OFF requests will be unauthenticated") or only half-set ("incomplete credentials, falling back to unauthenticated"). This covers all three entry points (one-shot CLI, serve stdio, serve http).
- README.md: documented the two new optional config keys and the startup-warning behavior.

Behavior: unconfigured/partially-configured deployments behave byte-identically to before (no Authorization header); configured ones send the header on every OFF request. Per the current OFF docs this enables the staging deployment (which requires off/off) and any future write operations; production read rate limits remain per-IP regardless of auth.

Tests: 5 new tests — wiremock assertions that `Basic dXNlcjpwYXNz` reaches the server on both endpoints when configured, that no Authorization header is sent without (or with partial) credentials (via received_requests), env-var loading with Debug redaction, and TOML/env precedence. Full suite: cargo fmt --check, clippy -D warnings, nextest (293 passed), doctests — all green. Live smoke test confirmed the startup warning appears without creds and is silent with them.
<!-- SECTION:FINAL_SUMMARY:END -->

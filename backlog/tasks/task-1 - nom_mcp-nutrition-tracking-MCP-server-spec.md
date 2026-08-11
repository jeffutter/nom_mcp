---
id: TASK-1
title: 'nom_mcp: nutrition-tracking MCP server spec'
status: Done
assignee: []
created_date: '2026-08-11 04:39'
updated_date: '2026-08-11 13:19'
labels:
  - 'wayfinder:map'
dependencies: []
documentation:
  - doc-5
ordinal: 1000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Destination

An implementation-ready spec for nom_mcp: a single-user Rust MCP server for logging meals, weight, and nutrition goals, backed by OpenFoodFacts + USDA FDC food data and local-file libSQL/Turso storage, exposed identically over MCP / local CLI / HTTP / remote-CLI via a notectl-style shared Operation abstraction. This map does not build the server — it produces the schema, tool inventory, and integration design a build effort would implement from.

## Notes

- Domain vocabulary: see /CONTEXT.md (Food, Meal, Portion, Weight Entry, Goal).
- Architecture reference: jeffutter/notectl — notectl-core/src/operation.rs (Operation trait), src/main.rs (local binary: serve + local CLI), src/bin/notectl-remote.rs (thin HTTP-client binary).
- Inspiration for tool surface: akutishevsky/nutrition-mcp README (tool list minus exclusions — see Out of scope).
- MCP serving via rmcp (https://crates.io/crates/rmcp).
- Storage: libSQL/Turso rust SDK (https://docs.turso.tech/sdk/rust/quickstart), local-file-only mode, no Turso cloud account.
- Nutrition data: openfoodfacts-rust (barcode/packaged) + USDA FoodData Central API (whole/raw foods, https://fdc.nal.usda.gov/api-guide#bkmk-1).
- Build tooling: nix flake + rust-overlay, mirroring jeffutter/notectl's flake.nix — nixpkgs-unstable, oxalica rust-overlay (`rust-bin.stable.latest.default`, following nixpkgs), crane for the actual build, one crane package per binary (maps directly onto TASK-1.6's main-binary + remote-CLI-bin-target split), mold linker on Linux, split `default`/`ci` devShells, nixpkgs-fmt formatter. Current repo `flake.nix` is still the cookiecutter stub and needs replacing when a build effort starts.
- CI/CD: mirrors jeffutter/notectl's `.github/workflows/{ci,cd,audit}.yml` verbatim in structure — CI (test/rustfmt/clippy/docs via `nix develop .#ci`) on push+PR; CD (cross-built release binaries + `nix build` pushed to the `jeffutter` Cachix cache + GitHub Release) on a semver git tag; daily cargo-audit. See TASK-1.15.
- For grilling tickets, invoke /grilling and /domain-modeling; update CONTEXT.md as terms sharpen.

## Not yet specified

None — every fog item identified during charting has graduated into a ticket and been resolved. The spec is implementation-ready.

## Out of scope

- Alcohol tracking — excluded per user request from the start
- Water tracking — excluded per user request from the start
- Timezone/unit-setting MCP tools (set_timezone, get_timezone, set_weight_unit, get_weight_unit) — excluded per user request from the start
- Auth / multi-user support — single-user, no auth needed
- CSV import/export (start_meal_import, bulk_import_meals, export_meals) — deferred, not needed for v1
- Trends/pattern analytics (get_trends, get_meal_patterns) — deferred to a future effort after v1 ships and real usage data exists
- Account deletion (delete_account) — not applicable to a self-hosted single-user server
- Metrics/tracing-export (OpenTelemetry etc.) — deferred per TASK-1.17, no audience to consume it yet
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- **Research rmcp crate capabilities for a multi-transport Operation pattern** (`TASK-1.1`) — rmcp 3.1.2 (notectl pins 2.2) macro-drives tools+schema (schemars) and swaps stdio/streamable-HTTP transports cleanly, but has no built-in Resources macro or 'MCP-only tool' concept — both must be built in nom_mcp's own core, same as notectl's Operation trait; see doc-1.

- **Research USDA FoodData Central API** (`TASK-1.4`) — free api.data.gov key (1,000 req/hr), search + detail/batch endpoints, nutrients per 100g with household portions alongside; use Foundation + SR Legacy + Survey (FNDDS) data types only, filter out Branded (Open Food Facts covers that); no Rust crate exists, build a bespoke reqwest client. See doc-2.

- **Research openfoodfacts-rust crate** (`TASK-1.3`) — thin/unpublished/stalled wrapper (no crates.io release, no code changes since 2022, no typed models); recommend calling OFF's REST API directly with reqwest instead of depending on this crate. See doc-3.

- **Research Turso/libSQL rust SDK for local-file-only usage** (`TASK-1.2`) — use the `libsql` crate (not the newer beta `turso` crate) with `Builder::new_local(path)`; no shipped migration tooling (BYO raw SQL); WAL is NOT the local default (must set `PRAGMA journal_mode=WAL` + `busy_timeout` explicitly for safe CLI+server concurrent file access) — see doc-4.

- **Research Turso/libSQL rust SDK for local-file-only usage** (`TASK-1.2`) — CORRECTED: use `turso` (pure Rust, pre-1.0), not `libsql` (mature but C-based) — see ticket's updated Final Summary for the full trade-off.

- **Design the core domain schema: Food, Meal, Portion, Weight Entry, Goal** (`TASK-1.5`) — Snapshot-based schema: Portion captures Food's nutrient rate at log time (immune to later catalog refreshes), Meal adjustment is nullable columns on the row, Goal is effective_from-versioned, logged_date is materialized for range queries; also settles turso's multi-process handoff as safe-with-explicit-clean-close (research-backed), keeping the local-CLI direct-DB path rather than folding into TASK-1.6.

- **Design the multi-modal operation/transport architecture** (`TASK-1.6`) — Two-crate workspace (unified nom-core + binary w/ remote bin target); Operation trait gains `surfaces()` so one registry drives CLI/HTTP/MCP dispatch (hand-written MCP list_tools/call_tool, not rmcp's #[tool] macro — ToolBase needs compile-time types, incompatible with a runtime Vec<Arc<dyn Operation>>); MCP-only widget-toggle tools become Operations with surfaces()=MCP-only, weekly-summary Resource stays outside Operation (hand-written ServerHandler glue over a capability-layer function); local-CLI direct-DB is first-class/top-level with a runtime lock-probe safety check, never talks remote (that's the separate thin remote-CLI binary's job).

- **Design how the server determines 'today' without an MCP-exposed timezone tool** (`TASK-1.7`) — Config-value-with-system-local-fallback IANA tz, resolved once at startup; a single Clock in nom-core injected into Operation execution computes 'today' fresh per call, so CLI/HTTP/MCP agree by construction via TASK-1.6's shared registry.

- **Finalize the v1 MCP tool inventory** (`TASK-1.8`) — 18 tools (2 Food, 7 Meal, 6 Weight Entry, 3 Goal): unified search_food that persists results immediately (food_id always usable, no import step); two-step log_meal (food_id required); whole-list-replace portion edits on update_meal; plain-keyword search_meals (no recurring-variation grouping); combined nutrient+weight get_goal_progress; per-serving create_custom_food. Full table in the ticket's Final Summary.

- **Design the nutrition-data resolution workflow across OpenFoodFacts, USDA FDC, and custom foods** (`TASK-1.9`) — search_food auto-routes barcode-shaped queries to OFF, else searches Custom (substring, first) + USDA, merged into a 5-candidate capped list with full nutrient snapshots per candidate; barcode miss falls through to free-text search, dish miss triggers per-ingredient decomposition with LLM-judgment collapse to a whole-dish Custom Food when mostly uncatalogued; Custom Foods have no server-side dedup (reuse via search's substring match); barcode/label photos are transcribed/extracted by the LLM itself, not a tool.

**Design Goals and daily-progress calculation** (`TASK-1.10`) — Goal nutrient targets gain an explicit Direction (target/minimum/maximum, required first-time, carried forward after) resolving the target-or-limit ambiguity; get_goal_progress returns per-nutrient consumed/target/remaining/percent/direction/status (met=exact equality) plus a weight section (latest-vs-target, no percent), all resolved as-of the queried date; no target unset and no goal-ever-set both resolve to null fields, never an error. See decision-2. Extends TASK-1.5's schema and TASK-1.8's set_nutrition_goals signature (noted on that ticket).

- **Design the MCP Resource and MCP-only widget-toggle representation** (`TASK-1.11`) — Weekly Summary Resource (rolling 7-day nutrient-avg-vs-goal + per-day breakdown + weight trend) stays outside Operation entirely per TASK-1.6; Widget Display is a plumbing-only on/off preference (get/set as MCP-only Operations, 18→20 tools) stored in a new DB settings table, kept separate from TASK-1.12's future Config (decision-3).

- **Design config and secrets handling** (`TASK-1.12`) — Defaults < TOML file ($XDG_CONFIG_HOME/nom_mcp/config.toml) < env vars (NOM_MCP_*), no CLI flags; USDA key from env or file (env wins, always redacted, validated lazily per-Operation so missing key doesn't block non-USDA commands); OFF User-Agent ships a working default; HTTP binds 127.0.0.1 by default; remote-CLI shares the same Config via its own [remote] table; timezone key settles TASK-1.7's deferred format.

**Design error-handling conventions across the four surfaces** (`TASK-1.13`) — CLI unifies onto `Result<Value, ErrorData>` too; 5-category taxonomy (NotFound/Validation/Conflict/ExternalApiFailure/StorageFailure) lives in `data.category`, mapped to HTTP 404/400/409/502/500 and CLI exit 3/4/5/6/7; HTTP body and MCP content both carry raw serialized ErrorData so local-CLI and remote-CLI share one render function; lock-probe rejection is an ordinary Conflict, not a new category.

- **Design testing strategy for external API integrations** (`TASK-1.14`) — record-and-replay fixtures via wiremock for OFF/USDA clients (needs a configurable base URL); domain logic unit-tested with no I/O; turso local-file temp-DB per integration test.
- **Design release/distribution process** (`TASK-1.15`) — single workspace semver, no CI/CD; 'nix build' near/at the host, systemd service in serve mode, ad hoc local CLI, remote-CLI thin client elsewhere.
- **Design edit/delete semantics for Meals, Portions, Foods, and Weight Entries** (`TASK-1.16`) — Portion edits recompute from their immutable snapshot, never re-fetch catalog data; Meal delete cascades to its Portions; no delete_food ever; Weight Entry/Goal edits are plain field updates; all deletes are hard deletes.
- **Design observability/logging approach** (`TASK-1.17`) — tracing + tracing-subscriber; server logs info-to-stderr, CLI defaults to warn; no metrics/tracing-export for v1.

- **Design release/distribution process** (`TASK-1.15`) — CORRECTED: mirrors notectl's GitHub Actions pipeline (CI on push/PR via nix devShell; CD on semver tag, cross-built binaries + nix build pushed to the 'jeffutter' Cachix cache + GitHub Release; daily cargo-audit) instead of the original no-CI/CD call — see ticket's updated Final Summary.
<!-- SECTION:NOTES:END -->

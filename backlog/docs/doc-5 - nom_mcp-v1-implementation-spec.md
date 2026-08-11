---
id: doc-5
title: nom_mcp v1 implementation spec
type: specification
created_date: '2026-08-11 13:18'
updated_date: '2026-08-11 13:19'
---
# nom_mcp v1 implementation spec

A single-user Rust MCP server for logging meals, weight, and nutrition goals, backed by OpenFoodFacts + USDA FDC food data and local-file libSQL/Turso storage, exposed identically over MCP, local CLI, HTTP, and a remote-CLI thin client via a notectl-style shared Operation abstraction.

This document consolidates the decisions made while charting [TASK-1](../tasks/task-1%20-%20nom_mcp-nutrition-tracking-MCP-server-spec.md) (wayfinder map). Each section links back to the ticket that resolved it — read the ticket's Final Summary and Implementation Notes for full rationale and rejected alternatives; this document states the settled design only.

Domain vocabulary (Food, Meal, Portion, Weight Entry, Goal, Direction, Weekly Summary, Widget Display) is defined in `/CONTEXT.md` and is not restated here.

## 1. Dependencies and external integrations

- **MCP serving**: `rmcp` (official `modelcontextprotocol/rust-sdk` crate). Pin an exact version at implementation time and re-verify feature flags/builder APIs against docs.rs — the crate's API moves fast (real renames between 2.2 and 3.1.2 in about a month). `rmcp` has no built-in concept of MCP Resources or "MCP-only" tools; both are built in nom_mcp's own core, same as notectl. (`TASK-1.1`, `doc-1`)
- **Storage**: the `turso` crate (pure Rust, pre-1.0) — not `libsql` (mature but C-based) — chosen to avoid a C-toolchain build dependency. `Builder::new_local(path)` for local-file-only mode, no Turso cloud account. No first-party migration tooling in either crate — raw SQL migrations. (`TASK-1.2`, `decision-1`, `doc-4`)
- **OpenFoodFacts (barcode/packaged foods)**: call the OFF REST API directly with `reqwest` and a hand-scoped serde struct. The `openfoodfacts-rust` crate is unmaintained (no crates.io release, no code changes since March 2022) and not worth depending on. Respect OFF's real rate limits (15 req/min/IP reads, 10 req/min/IP search) and set a real User-Agent. (`TASK-1.3`, `doc-3`)
- **USDA FoodData Central (whole/raw foods)**: no usable Rust crate exists — build a bespoke `reqwest` client. Free `api.data.gov` key (1,000 req/hr). Query only `Foundation` + `SR Legacy` + `Survey (FNDDS)` data types; exclude `Branded` (OFF already covers packaged/branded foods). Nutrients are per-100g across all data types, with household/serving portions available alongside. (`TASK-1.4`, `doc-2`)

## 2. Storage schema

Five domain tables, single-user (no `user_id` anywhere):

- **`foods`** — `source` enum (OpenFoodFacts / USDA FDC / Custom) + nullable `external_id`, `unique(source, external_id)`. Full nutrient cache (kcal/protein/carbs/fat/fiber) with no auto-refresh.
- **`meals`** — `logged_at` (UTC) + materialized `logged_date` (for range queries, computed at write time from `logged_at` via the Clock from §4). Optional raw-macro adjustment stored as nullable columns directly on the row (not a separate table or pseudo-Portion row).
- **`portions`** — `meal_id`/`food_id` FKs, dual `quantity_mode` (grams or servings). **Snapshots** the Food's per-100g nutrient rate + `serving_size_g` at log time, so totals are computed on read and are immune to later Food catalog refreshes. This is the schema's central invariant — see §9 for its edit/delete implications.
- **`weight_entries`** — `logged_at`/`logged_date` pair, bare value in the system-wide configured unit (no per-entry unit column).
- **`goals`** — versioned via `effective_from`, so past-day progress judges against the goal active *that day*, not today's. Nutrient targets carry an explicit `direction` column (target/minimum/maximum) — see §6.

Indexes: `logged_date` (meals, weight_entries), `meal_id` (portions), `effective_from` (goals).

**Turso multi-process safety**: no doc/issue explicitly confirms sequential (non-overlapping) multi-process file handoff is safe, but turso's locking (POSIX `fcntl` advisory locks, released on close or process exit via a `Drop` impl — not a stale-lockfile scheme) strongly supports it. Caveat: possible WAL data loss on crash-before-checkpoint. **Hard invariant**: both the local-CLI and server code paths must fully close and checkpoint their connection before handoff to the other. This keeps the local-CLI direct-DB path viable (see §3) rather than forcing it through HTTP. (`TASK-1.5`)

## 3. Multi-surface architecture

Two-crate workspace: a unified `nom-core` library (Operation trait, all five entities' capability logic, storage access, and both external API clients as modules — no per-feature crate split, since entities are relationally coupled, not independent plugins) plus a binary package with two targets: the main binary (`serve` + local CLI) and a thin `nom-mcp-remote` binary (HTTP-only client), mirroring notectl's main+remote split.

**Operation trait** gains one new method: `fn surfaces(&self) -> Surfaces` (which of CLI/HTTP/MCP an operation is exposed on; defaults to all three). One operation registry drives all three transports:
- CLI subcommand registration
- HTTP route registration
- A **hand-written** `list_tools`/`call_tool` on the MCP `ServerHandler` that loops the registry directly — deliberately bypassing rmcp's `#[tool]` macro/`ToolRouter`, whose `ToolBase` trait requires compile-time-associated-function types incompatible with a runtime `Vec<Arc<dyn Operation>>`.

This closes, by construction, the silent CLI/HTTP-vs-MCP drift that notectl actually has today (3 outline operations present in CLI/HTTP but missing from MCP, uncaught).

MCP-only widget-toggle tools are ordinary Operations with `surfaces() = MCP only`, dispatched through the same mechanism as everything else. The weekly-summary MCP Resource is different in kind (no CLI/HTTP shape, not a Tool) and stays **outside** the Operation trait — hand-written `list_resources`/`read_resource` glue on `ServerHandler`, with its data-fetching logic in a capability-layer function like everything else.

**Local-CLI** is not a runtime decision — it always executes Operations in-process against the local DB, first-class and top-level alongside `serve` (matching notectl). Remote access is exclusively the separate thin remote-CLI binary over HTTP; the local binary structurally never talks remote. Given §2's clean-close/checkpoint invariant, local-CLI adds a runtime lock probe (on the same POSIX advisory lock turso already takes) before opening the DB directly, failing fast if the server appears to hold it. (`TASK-1.6`)

## 4. Date/time handling ("today")

No MCP-exposed timezone-setting tools exist (out of scope). Timezone is resolved **once at startup** by `nom-core`: explicit IANA tz name from server config if set (§8), else fallback to host system-local timezone.

A single `Clock` owned by `nom-core` is injected into Operation execution and computes "today" **fresh on every call** (never cached — the server is long-running, and a cached "today" would silently go stale at midnight). Since §3's Operation registry drives CLI/HTTP/MCP dispatch, injecting the Clock there makes all three surfaces (plus local CLI, same binary) agree on "today" by construction. Remote-CLI never computes dates itself — thin HTTP client only.

This is also where `meals.logged_date`/`weight_entries.logged_date` get materialized at write time. If the configured tz later changes, historical `logged_date` values are **not** retroactively recomputed. (`TASK-1.7`)

## 5. v1 MCP tool inventory (20 tools)

**Food (2)**
- `search_food(query)` — barcode-shaped (all-digit) queries route to OpenFoodFacts only; everything else routes to local Custom Foods (case-insensitive substring match, searched first) + USDA FDC, merged into one list capped at 5 combined candidates. Every match is upserted into the local `foods` cache as part of the call — **searching IS resolving**, `food_id` is immediately usable, no separate import step. Each candidate carries `food_id`, `name`, `source`, and its full cached nutrient snapshot (no separate `get_food_details` tool — the list must be self-sufficient for disambiguation).
- `create_custom_food(name, serving_size: {quantity, unit}, nutrients)` — nutrients given **per one serving**, not per-100g, matching how a user actually knows a homemade dish's macros. No server-side dedup; reuse relies entirely on `search_food`'s custom-first substring match.

**Meal (7)**
- `log_meal(portions: [{food_id, quantity, quantity_mode}], adjustment?, logged_at?)` — two-step flow, `food_id` must already exist (from a prior `search_food`/`create_custom_food` call).
- `update_meal(meal_id, portions?, adjustment?, logged_at?)` — partial patch; `portions` when present **replaces the whole array** (no granular add/remove-portion tools).
- `delete_meal(meal_id)` — errors on not-found rather than silent no-op.
- `search_meals(query, date_range?)` — plain keyword search over linked Food names, recency-ordered. Deliberately **not** the inspiration project's recurring-variation grouping, which reads as the pattern-analytics behavior already deferred (Out of scope).
- `get_meals_today()` / `get_meals_by_date(date)` / `get_meals_by_date_range(start, end)`.

**Weight Entry (6)**
- `log_weight(value, logged_at?)`, `update_weight_entry(id, value?, logged_at?)`, `delete_weight_entry(id)` (errors on not-found).
- `get_weight_today()` / `get_weight_by_date(date)` / `get_weight_by_date_range(start, end)`.

**Goal (3)**
- `set_nutrition_goals(<partial subset of calories/protein_g/carbs_g/fat_g/fiber_g/target_weight>, direction?)` — partial patch; creates a new `effective_from = today` versioned row merged over the current goal. Each nutrient's `direction` (target/minimum/maximum) is **required the first time that nutrient is set**, carried forward on later updates that omit it. `target_weight` has no direction. Versioning (`effective_from`) itself stays internal — no caller-facing param.
- `get_nutrition_goals()` — currently active goal only.
- `get_goal_progress(date?)` — see §6 for full response shape. Absorbs what would otherwise be a separate "nutrition summary" tool.

**Widget Display (2, MCP-only)**
- `get_widget_display()` / `set_widget_display(enabled: bool)` — `surfaces() = MCP only`. See §7.

Minor implementation defaults (forced by the above, not separate design forks): a custom food's portions can only use `quantity_mode = "servings"` unless its `serving_size.unit` is grams (no gram equivalence otherwise known). (`TASK-1.8`, updated by `TASK-1.10`)

## 6. Nutrition data resolution workflow

`search_food` auto-detects barcode-shaped queries (routes to OFF only) vs everything else (Custom Foods substring match, then USDA FDC, merged/capped at 5). Full workflow, including LLM-driven fallbacks:

1. **Barcode miss** (no OFF match) → LLM falls through to a free-text `search_food` retry using the product name, rather than going straight to a Custom Food.
2. **Free-text/dish miss** → LLM decomposes the dish into ingredients and resolves each individually via `search_food`, creating a one-off Custom Food for any ingredient found in neither source. LLM judgment (guided by tool-description prose, not a hard numeric threshold) collapses to a single whole-dish Custom Food when most ingredients end up uncatalogued.
3. **Barcode photo** → LLM transcribes the digits itself and calls `search_food(digits)`.
4. **Nutrition-label photo** → LLM extracts the structured nutrients itself and calls `create_custom_food(...)` directly, **bypassing search entirely** — a photographed label implies no catalog entry exists.

Tool descriptions must explicitly instruct "search before creating" since Custom Food dedup is entirely search-driven, not server-enforced. (`TASK-1.9`)

## 7. Goals and daily-progress calculation

Goal nutrient targets (calories, protein, carbs, fat, fiber) carry an explicit **Direction** (target / minimum / maximum) — see `decision-2` for full rationale. `target_weight` has no Direction; its progress reads directly off the comparison to the latest Weight Entry.

`get_goal_progress(date?)` — single date only, no date-range aggregation. Returns:
- **Per nutrient**: `consumed`, `target` (null if unset for that date's active Goal), `remaining = target − consumed` (null if no target), `percent = consumed/target×100` (null if target null or zero), `direction` (echoed), `status ∈ {under, met, over}` (met = exact equality, no tolerance band).
- **Weight**: `latest_weight` (the Weight Entry on/before the queried date, null if none exist yet), `target_weight` (from the Goal active that date, null if unset), `remaining`, same status scheme — **no percent field** for weight (a weight ratio isn't a meaningful progress number without a tracked baseline).

If no Goal has ever been active as of the queried date, `consumed`/`latest_weight` still populate from real logged data, but every target/remaining/percent/status/direction field is null — **never an error**.

Both "active Goal" and "latest weight" resolve **as-of the queried date** (not always-current), consistent with §2's `effective_from` versioning and §4's Clock-driven date semantics. (`TASK-1.10`, `decision-2`)

## 8. MCP Resource and Widget Display

**Weekly Summary Resource** — MCP-only, fixed URI (`nom://weekly-summary`), no params, live-computed on every read (no caching). Rolling **last-7-days** window (not calendar week). Two sections:
- **Nutrients**: per-nutrient daily-average consumed vs daily Goal target, shaped like `get_goal_progress` (consumed/target/remaining/percent/direction/status), plus a per-day array of the week's raw daily totals. No-goal-set resolves to null fields, same convention as `get_goal_progress`.
- **Weight**: start/end/delta computed from Weight Entries inside the window; if none logged this week, start/delta are null but `latest_known_weight` still comes from the most recent entry before the window if one exists. Includes the target-weight comparison, mirroring `get_goal_progress`.

**Widget Display setting** — single global on/off preference (rich MCP-client-rendered widgets vs plain text/JSON). Persisted in a new single-row `settings` table (`widget_display_enabled BOOLEAN`) in the same local libSQL file as domain data — deliberately **separate** from §9's startup Config, since this is runtime-mutable via a tool call while Config is read once at process start (`decision-3`). v1 scope is **plumbing only**: stored and readable/writable, but no tool or Resource output branches on it yet. (`TASK-1.11`, `decision-3`)

## 9. Config and secrets

Precedence: defaults < optional TOML config file < env vars (env wins). No CLI flags for values.

- Config file: `$XDG_CONFIG_HOME/nom_mcp/config.toml` (fallback `~/.config`).
- DB file: `$XDG_DATA_HOME/nom_mcp/nom.db` (fallback `~/.local/share`).
- Env vars: prefixed `NOM_MCP_*`.
- **USDA FDC API key**: from env var or config file (env wins). Plaintext-on-disk risk accepted and documented, no keychain integration. Validated **lazily per-Operation** via §3's registry — a missing key only errors when a USDA-touching Operation actually runs, not at process startup (so weight/meal logging works fine without it configured). Always redacted — its type never plain-`Debug`/`Display`s the value.
- **OpenFoodFacts User-Agent**: ships a working hardcoded default (`nom_mcp/<version>`), overridable.
- **HTTP transport**: binds `127.0.0.1` by default (no-auth is a loopback-trust assumption, not open-network).
- **Remote-CLI**: shares the same Config type/file/env mechanism via its own `[remote]` table (`server_url`) that the main binary ignores.
- **Timezone** (§4): a `timezone` TOML key / `NOM_MCP_TIMEZONE` env var, optional IANA string, same layering as everything else. (`TASK-1.12`)

## 10. Error handling

All four surfaces unify on one error currency: `Result<Value, ErrorData>`. CLI dispatch (`execute_from_args`) is changed to return this too (previously `Result<String, Box<dyn Error>>`), and a shared render function (used by both local-CLI and remote-CLI) turns an `ErrorData` into stderr text + exit code.

Taxonomy lives in `ErrorData.data.category` (the JSON-RPC `code` field stays coarse/spec-standard):

| Category | `code` | HTTP | CLI exit |
|---|---|---|---|
| NotFound | RESOURCE_NOT_FOUND | 404 | 3 |
| Validation | INVALID_PARAMS | 400 | 4 |
| Conflict | INTERNAL_ERROR | 409 | 5 |
| ExternalApiFailure | INTERNAL_ERROR | 502 | 6 |
| StorageFailure | INTERNAL_ERROR | 500 | 7 |
| uncategorized (fallback) | as rmcp sets it | 500 | 7 |

Unclassified/panic path keeps exit code 1. No separate "Internal" category — unclassified/protocol-level errors fall back to StorageFailure-tier treatment.

`data` payload per category: Validation carries `{category, field, reason}`; Conflict/ExternalApiFailure/StorageFailure carry `{category, reason}`; NotFound needs only `category`.

Wire representation is the same `ErrorData` JSON across every non-CLI-native surface: HTTP response body is the raw serialized `ErrorData` (status per the table above) so remote-CLI can deserialize it and feed it through the exact same render function local-CLI uses. MCP returns `CallToolResult{ is_error: true, content: [Text(json(ErrorData))] }`.

The local-CLI's runtime lock-probe (§3) is not a new category — it's an ordinary Conflict with `reason: "local_db_locked"`, rendered with a CLI-specific message ("server is running — stop it or use the remote-CLI instead"). (`TASK-1.13`)

## 11. Testing strategy

- **Unit tests**: pure domain logic (schema validation, goal-progress calc, error-taxonomy mapping) — no I/O.
- **External API integration tests**: record-and-replay via fixture JSON files captured from real OpenFoodFacts/USDA FDC responses, served back through `wiremock`. Requires §1's `reqwest` clients to accept a **configurable base URL** (constructor param) so tests can point at the local wiremock server instead of production endpoints.
- **DB-layer integration tests**: turso's local-file mode with a fresh temp-file DB per test — real schema, no DB mocking.
- **Surface tests**: since §3's Operation-trait unification means Operation logic is tested once, CLI/HTTP/MCP-specific tests stay thin smoke tests for wiring only. (`TASK-1.14`)

## 12. Release, distribution, and CI/CD

Single Cargo-workspace version (semver), bumped manually per release — binaries ship together, no per-binary versioning.

Mirrors `jeffutter/notectl`'s GitHub Actions pipeline (`.github/workflows/{ci,cd,audit}.yml`) directly:

- **CI** — on push to `main` + PRs: four jobs (test, rustfmt, clippy, docs), each run inside `nix develop .#ci -c cargo ...`, using `DeterminateSystems/nix-installer-action` + `magic-nix-cache-action` + `Swatinem/rust-cache`.
- **CD** — triggered on a semver git tag push: cross-compiles release binaries for macOS-aarch64, Linux-x86_64, and Linux-aarch64, strips debug symbols, tars + sha256s them; then uses `cachix/install-nix-action` + `cachix/cachix-action` (cache name `jeffutter`) to run `nix build .#nom-mcp` and `.#nom-mcp-remote` (§3's two crane packages) plus `nix flake check`, pushing those build closures into the `jeffutter` Cachix binary cache. Tarballs + shasums are attached to a GitHub Release via `softprops/action-gh-release`.
- **Security audit** — daily cron plus Cargo.toml/Cargo.lock-touching pushes/PRs, `cargo-audit` via `rustsec/audit-check`.

**Deploy**: the version tag is the CD trigger. On the host machine, deploy is `nix build --accept-flake-config ...`, which pulls the prebuilt closure from the `jeffutter` Cachix cache instead of rebuilding from source. The server binary runs as a systemd (user or NixOS) service in `serve` mode; local CLI is invoked ad hoc on the same machine; remote-CLI is used from elsewhere. No package registry, no automated deploy beyond that, no rollback tooling beyond nix's normal generation rollback — this remains a self-hosted, single-user model, just with CI verification and a warm binary cache. (`TASK-1.15`, corrected)

**Build tooling** (nix flake + rust-overlay, mirroring notectl's `flake.nix`): nixpkgs-unstable, oxalica rust-overlay (`rust-bin.stable.latest.default`, following nixpkgs), crane for the actual build, one crane package per binary, mold linker on Linux, split `default`/`ci` devShells, nixpkgs-fmt formatter. The current repo `flake.nix` is still the cookiecutter stub and needs replacing when a build effort starts.

## 13. Edit/delete semantics

- **Editing a Portion**: recomputes its macros from the nutrient rate captured at creation time (§2's snapshot) — **never** re-fetches current catalog data. There is no "refresh nutrition data" operation in v1.
- **Deleting a Meal**: cascades to delete its Portions (they have no existence independent of their Meal).
- **Foods are never hard-deleted** — no `delete_food` operation exists. Since Portions already snapshot their own data, keeping unused catalog/Custom Foods around is harmless. Hiding/archiving a Custom Food from search is deferred (not needed for v1).
- **Weight Entry / Goal edits**: plain field updates, no cascade concerns — nothing else references them.
- **All deletes** (Meal, Weight Entry) are **hard deletes**, no soft-delete/undo — single-user tool, no audit-trail requirement. (`TASK-1.16`)

## 14. Observability and logging

`tracing` + `tracing-subscriber` across all four surfaces.

- **Server modes** (HTTP/MCP serve): structured logs to stderr at `info` by default, level overridable via `RUST_LOG`/config.
- **Local CLI**: defaults to `warn` to keep command output clean — user-facing errors surface through §10's `ErrorData` rendering, not raw log lines.
- **External API calls** (OFF, USDA FDC): log request outcome (success/failure, status code) at `debug`. API keys are never logged.
- **No metrics or tracing-export** (OpenTelemetry etc.) for v1 — single-user tool, no ops team monitoring it. Deferred to a future effort if the need ever arises. (`TASK-1.17`)

## 15. Out of scope for v1

- Alcohol tracking, water tracking — excluded per user request from the start.
- Timezone/unit-setting MCP tools (`set_timezone`, `get_timezone`, `set_weight_unit`, `get_weight_unit`) — excluded per user request from the start.
- Auth / multi-user support — single-user, no auth needed.
- CSV import/export (`start_meal_import`, `bulk_import_meals`, `export_meals`) — deferred, not needed for v1.
- Trends/pattern analytics (`get_trends`, `get_meal_patterns`) — deferred to a future effort after v1 ships and real usage data exists.
- Account deletion (`delete_account`) — not applicable to a self-hosted single-user server.
- Metrics/tracing-export — deferred per §14, no audience to consume it yet.

## Source map

| Area | Ticket | Related assets |
|---|---|---|
| rmcp capabilities | `TASK-1.1` | `doc-1` |
| Storage SDK choice | `TASK-1.2` | `decision-1`, `doc-4` |
| OpenFoodFacts crate | `TASK-1.3` | `doc-3` |
| USDA FDC API | `TASK-1.4` | `doc-2` |
| Domain schema | `TASK-1.5` | |
| Multi-surface architecture | `TASK-1.6` | |
| "Today"/Clock | `TASK-1.7` | |
| v1 tool inventory | `TASK-1.8` | |
| Food resolution workflow | `TASK-1.9` | |
| Goals & progress | `TASK-1.10` | `decision-2` |
| MCP Resource & Widget Display | `TASK-1.11` | `decision-3` |
| Config & secrets | `TASK-1.12` | |
| Error handling | `TASK-1.13` | |
| Testing strategy | `TASK-1.14` | |
| Release/distribution/CI-CD | `TASK-1.15` | |
| Edit/delete semantics | `TASK-1.16` | |
| Observability/logging | `TASK-1.17` | |

Full map: `TASK-1` — "nom_mcp: nutrition-tracking MCP server spec".

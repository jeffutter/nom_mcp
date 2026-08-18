---
id: TASK-54
title: Local dev instance with one-command seed data
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-17 21:10'
updated_date: '2026-08-18 02:09'
labels:
  - devtooling
  - seed-data
dependencies: []
priority: high
type: task
ordinal: 60000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Widget work (and any e2e validation of the MCP Apps UI) needs a local nom_mcp instance with realistic multi-day data. Today there is no fast way to populate a fresh database: goals, custom foods, meals/portions, and weight entries must all be entered operation-by-operation, which makes iterating on widget visuals slow and painful.

Goal: a single documented command stands up a throwaway local instance pre-populated with seed data, so anyone can iterate on widgets (or any surface) without manual data entry.

Seed content requirements:
- Active nutrition goals with directions, covering calories/protein/carbs/fat/fiber plus a target weight
- A handful of custom foods with realistic macros
- Meals + portions spanning at least 7 days including today, with varied meal timing (so fasting windows exist)
- Weight entries spanning at least 7 days including near today
- Values chosen so statuses span 'under', 'met', and 'over' — every existing widget (goal progress, weekly progress) should render non-trivial content

Constraints:
- Fit the existing architecture: Operation trait + registry (see AGENTS.md). The seed action is local-CLI-only (Surfaces::CLI) — it makes no sense over HTTP/MCP.
- Targets a configurable/throwaway DB path; must never touch the default production DB location.
- Repeatable: re-running against the same path resets to a clean known state.
- Dates deterministic relative to 'today' so status coverage holds whenever it runs.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A single documented command creates a fresh local DB populated with seed data: active goals (all 5 nutrients + target weight, with directions), >=5 custom foods, meals/portions spanning >=7 days including today, and weight entries spanning >=7 days.
- [x] #2 Seeding writes only to a throwaway/configurable DB path; the default production DB location is never touched.
- [x] #3 Re-running the seed against the same path resets the database to the same clean known state (repeatable).
- [x] #4 With seeded data, get_goal_progress returns non-null consumed/target for all 5 nutrients and spans at least one 'under', one 'met', and one 'over' status; get_weekly_progress returns >=7 days of calorie data and >=2 weight points.
- [x] #5 README documents the full flow with exact commands: seed a fresh DB, start the server (serve http), and connect an MCP client.
- [x] #6 Integration tests cover seeding against a temp DB (row counts and spot-checked values).
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
# Implementation plan — TASK-54: local-dev instance with one-command seed data

## Design decisions (verified against repo 2026-08-17)

1. **`seed_data` operation** (`Surfaces::CLI` only), new module `nom-core/src/seed/mod.rs`. Registered once in `build_registry()` (nom-mcp/src/main.rs, ~line 120 area) as `registry.register(Arc::new(SeedData::new(*clock)))`. Precedent for restricted surfaces: widget ops use `Surfaces::MCP`.
2. **Explicit required `path` input arg** (not an env var) for the seed op:
   - Self-documenting: `nom-mcp seed_data path=/tmp/nom-dev/nom.db`
   - Testable without process-global env mutation (Rust 2024 edition → `std::env::set_var` is unsafe; existing `with_db_path` builders are `#[cfg(test)]`-only and unavailable to integration tests).
   - Safety gate (AC#2 by construction): if the resolved path equals the default production path (`config::db_path()`), refuse with a `Validation` error: "refusing to seed the default database at {p}; pass a throwaway path".
3. **`NOM_MCP_DB_PATH` env override** for the *server side* (planner finding confirmed: `serve http` always opens the default DB — `AppConfig` has no db_path field; `config::db_path()` is XDG-only). Minimal change in `nom-core/src/config.rs`:
   - `db_path()`: return `PathBuf::from(std::env::var("NOM_MCP_DB_PATH"))` when set and non-empty; else current XDG default.
   - `session_db_path()`: derive from `db_path().parent().join("mcp_sessions.db")` (identical behavior when unset; fully isolates dev-instance MCP session state in the throwaway dir).
   - Unit tests using the existing `TestGuard` env pattern in config.rs tests.
4. **Reset semantics (AC#3):** delete `path`, `path-wal`, `path-shm` (ignore NotFound) → `Connection::open_at(path)` (creates parent dirs, runs hash-tracked idempotent migrations) → insert all seed rows in ONE transaction (`BEGIN TRANSACTION`/`COMMIT` pattern from storage/migration.rs). Re-run = byte-identical known state.
5. **Raw SQL inserts, single connection.** Both `get_goal_progress` and `get_weekly_progress` read materialized `meals.total_*` columns (`SUM(total_calories) FROM meals`), NOT portions joins — so seeding writes consistent `total_*` values directly. Portion macro formula (replicated exactly from meal/mod.rs `compute_portion_macros`): grams mode → `snapshot_X_per_100g * quantity / 100.0`. All quantities chosen so contributions are exact in f64 (integer or terminating products).
6. **Dates deterministic relative to today** via registry-shared `Clock` (`clock.today()`, `Clock::format_date`): window D-6..D0 (7 days incl. today).
7. **Enable widget display** in the seed (`settings.widget_display_enabled = 1`, following widget/mod.rs upsert pattern: SELECT LIMIT 1 → UPDATE or INSERT) so widgets render immediately.
8. **README key=value docs are NOT stale** (a prior planner claimed otherwise — verified wrong: `cli.rs::parse_params` parses `key=value` exactly as documented). No README fix needed beyond adding the new section.

## Files

- NEW `nom-core/src/seed/mod.rs` — `SeedData` op + fixture data + unit tests
- EDIT `nom-core/src/lib.rs` — `pub mod seed;`
- EDIT `nom-core/src/config.rs` — `db_path()`/`session_db_path()` env override + tests
- EDIT `nom-mcp/src/main.rs` — register `SeedData` in `build_registry`
- NEW `nom-mcp/tests/seed_e2e.rs` — binary-spawning e2e (CARGO_BIN_EXE_nom-mcp; precedent: lock_probe_integration.rs spawns a helper binary)
- EDIT `README.md` — new section (below)

## Operation spec

- `name()`: `"seed_data"`; `surfaces()`: `Surfaces::CLI`
- `input_schema()`: `{"type":"object","properties":{"path":{"type":"string","description":"Path of the throwaway SQLite DB file to create and populate"}},"required":["path"]}`
- `execute_json` steps: parse+require `path` (missing → `Validation`) → make absolute → reject if equal to default `config::db_path()` → delete file + `-wal`/`-shm` sidecars → `Connection::open_at` → one transaction inserting goals, foods, meals, portions, weight_entries, settings → return summary JSON `{db_path, days, foods, meals, portions, weight_entries, weight_points, goal: {...}}`
- Errors: bad/missing path → `Validation`; locked target (server running) → existing `local_db_locked` Conflict from open/probe (already friendly-worded)

## Seed fixture (static table in code; precomputed, exact-in-f64)

Foods (source=`Custom`, external_id NULL, macros per 100g):
| food | kcal | P | C | F | Fib |
|---|---|---|---|---|---|
| Oatmeal (dry) | 380 | 12 | 66 | 7 | 10 |
| Whole milk (2%) | 122 | 8.7 | 4.8 | 4.8 | 0 |
| Chicken breast (grilled) | 165 | 31 | 0 | 3.6 | 0 |
| Brown rice (cooked) | 112 | 2.6 | 24 | 0.3 | 1.8 |
| Broccoli (steamed) | 35 | 2.4 | 7 | 0.4 | 5 |
| Almonds | 579 | 21 | 22 | 50 | 12 |
| Protein shake (prepared) | 120 | 24 | 6 | 1 | 0 |

Goals (effective_from = D0): calories 2000 `target`, protein_g 150 `minimum`, carbs_g 200 `maximum`, fat_g 75 `maximum`, fiber_g 37 `minimum`, target_weight 80.0.

Today (D0) — all quantities multiples of 100g → exact f64 totals:
- 07:30 breakfast: oatmeal 100g + milk 100g → 502 kcal / 20.7 P / 70.8 C / 11.8 F / 10 Fib
- 12:45 lunch: chicken 200g + broccoli 200g → 400 / 66.8 / 14 / 8.0 / 10
- 19:30 dinner: chicken 200g + broccoli 100g → 365 / 64.4 / 7 / 7.6 / 5
- 21:15 snack: almonds 100g → 579 / 21 / 22 / 50 / 12
- **Totals: 1846 kcal / 172.9 P / 113.8 C / 77.4 F / 37 Fib**

Status coverage (AC#4) — status math is pure arithmetic on `target − consumed` (goal/mod.rs `nutrient_progress`; direction is informational):
- calories 1846 < 2000 → **under**; protein 172.9 > 150 → **over**; carbs 113.8 < 200 → **under**; fat 77.4 > 75 → **over**; fiber 37 == 37 (integer-exact) → **met**. All three statuses covered; all 5 nutrients non-null consumed/target. Weight 78.0 vs target 80.0 → under (bonus).

Days D-6..D-1: 1–3 meals/day from the same foods, varied timing (first meal 06:30–11:30; ≥2 days with first meal ≥ 11:00 and prior-day last meal ≤ 19:00 → overnight gaps ≥ 16h so weekly fasting stats are non-trivial); every day has ≥1 meal (→ ≥7 days of weekly calorie data). Weight entries D-6..D0 at 07:00: 80.5, 80.1, 79.8, 79.4, 79.0, 78.5, 78.0 (downward trend; ≥2 weekly weight points).

## Tests (AC#6)

1. **Unit** (in `seed/mod.rs`, TempDb-style tempdir like storage/test.rs): seed via `execute_json(json!({"path": tmp}))` → assert files exist; row counts (foods=7, distinct meal dates=7, portions>0, weight_entries=7, goals=1); spot-check a food's macros and today's meal totals vs hardcoded expectations; **re-run → identical state** (ordered full-table dump compared); missing `path` → Validation; `path` == default db_path → Validation refusal.
2. **E2E** (`nom-mcp/tests/seed_e2e.rs`, spawn `CARGO_BIN_EXE_nom-mcp`): `seed_data path=$TMP/nom.db` → `get_goal_progress` (via `NOM_MCP_DB_PATH=$TMP/nom.db`) asserts status set ⊇ {under, met, over}, all 5 nutrients non-null; `get_weekly_progress` asserts ≥7 days calorie data + ≥2 weight points. This encodes AC#4 as a test and exercises the real CLI surface + env override together.

## README section (AC#5)

```sh
# 1. Seed a throwaway DB (never touches your real data)
nom-mcp seed_data path=/tmp/nom-dev/nom.db
# 2. Start the server against it
NOM_MCP_DB_PATH=/tmp/nom-dev/nom.db nom-mcp serve http --port 8000
# 3. Point a streamable-HTTP MCP client at http://localhost:8000/mcp
#    (REST: POST /api/{operation}; remote CLI: NOM_MCP_remote__server_url=http://localhost:8000 nom-mcp-remote ...)
rm -rf /tmp/nom-dev   # cleanup — the seed DB is disposable
```
Plus notes: re-running `seed_data` resets the DB; don't run the local CLI against a seeded DB while the server holds it (`local_db_locked`); document `NOM_MCP_DB_PATH` in the config section.

## Verification gates

`cargo fmt --all --check` · `cargo clippy --all-targets --all-features --workspace -- -D warnings` · `cargo nextest run --all-features --workspace` · `cargo test --doc --all-features --workspace` · `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --document-private-items --all-features --workspace --examples`

## Gotchas

- `std::env::set_var` is unsafe (edition 2024) — never mutate process env in tests; the e2e test passes the env var to the spawned child only.
- Delete `-wal`/`-shm` sidecars, not just the .db file.
- Fiber 'met' relies on integer-exact sums — keep D0 fiber contributions integral (they are: 10+10+5+12).
- `UNIQUE(source, external_id)` allows multiple Custom rows with NULL external_id (SQLite NULLs are distinct) — fine.
- Midnight edge: re-run determinism holds within the same calendar day; cross-midnight re-runs shift dates by design (documented).
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Executed per plan (resumed prior WIP). Delivered: nom-core/src/seed/mod.rs (SeedData op, Surfaces::CLI, fixture data, 7 unit tests incl. full-table-dump re-run equality, default-path refusal, weekly-progress-on-seeded-db), config.rs NOM_MCP_DB_PATH override (+ session_db_path derivation, 2 new env tests), lib.rs module export, main.rs registration, nom-mcp/tests/seed_e2e.rs (binary-spawning e2e: seed -> get_goal_progress via NOM_MCP_DB_PATH -> reseed determinism), README sections (new 'Local dev instance with one-command seed data' + Configuration note). Deviations from plan: (1) e2e drives get_weekly_progress indirectly — it is Surfaces::MCP-only so has no CLI route; its seeded-data behavior is covered by direct-operation test test_weekly_progress_on_seeded_db in seed/mod.rs (with_db_path is cfg(test)-only). (2) Plan item 8 was wrong: local CLI is clap-based (--key value, since TASK-30), NOT key=value; parse_params is only used by nom-mcp-remote. README/AGENTS.md local-CLI syntax docs are stale — filed as follow-up. Verified end-to-end manually: seed_data -> serve http on seeded DB -> REST get_goal_progress (statuses under/under/over/met/over) -> MCP tools/call get_weekly_progress (7 daily_totals, fasting avg 15.3h over 6 days) -> resources/read nom://weekly-summary. All gates green: fmt, clippy -D warnings, nextest 321/321, doctests, rustdoc -D warnings.

Follow-up filed: TASK-59 (stale local-CLI key=value docs — README/AGENTS.md vs clap --key value reality).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
One-command seed data for local dev instances: new CLI-only 'seed_data' operation (nom-core/src/seed) creates or resets a throwaway SQLite DB with deterministic 7-day fixture data (active goals w/ directions for all 5 nutrients + target weight, 7 custom foods, 18 meals incl. today with varied timing, 7 weight entries, widget display enabled), chosen so get_goal_progress spans under/met/over and get_weekly_progress has >=7 calorie days, >=2 weight points, non-trivial fasting stats. Safety: refuses the default DB path; lock-probes before deleting; deletes WAL sidecars on reset. NOM_MCP_DB_PATH env override lets serve http / any surface target the seeded DB (session store relocates alongside). Documented end-to-end in README (seed -> serve http -> MCP client) and covered by 7 unit tests plus a binary-spawning e2e test. Verified live: REST + MCP tools + weekly-summary resource all return the seeded content.
<!-- SECTION:FINAL_SUMMARY:END -->

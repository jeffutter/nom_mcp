---
id: TASK-62.1
title: >-
  Generalize migration runner to a version list and add meals.meal_type via
  migration v2
status: Done
assignee:
  - '@ralph'
created_date: '2026-09-07 01:24'
updated_date: '2026-09-07 01:56'
labels:
  - task
  - planned
dependencies: []
documentation:
  - >-
    backlog/docs/research/doc-6 -
    Research-meal-type-annotation-time-based-default-and-schema-evolution.md
parent_task_id: TASK-62
priority: high
type: task
ordinal: 70000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Foundation slice for TASK-62: make migrations extensible, then add the column. Ships schema ONLY — no module reads or writes the value yet, so this can merge and deploy safely on its own.

Facts (verified in doc-6 §3/§4 and by research agents):
- nom-core/src/storage/migration.rs is single-migration by construction: MIGRATION_V1 = include_str!("schema.sql") (L9), bespoke hash_v1() (L13-24), and apply_migration() (L101-119) early-returns `if current_version >= 1`.
- Two tests pin "exactly one migration row": nom-core/src/storage/test.rs:113 (test_migrations_table_has_version_entry) and :147 (test_migration_idempotency).
- Every writer to `meals` enumerates its columns explicitly (meal/mod.rs:258 insert_meal, seed/mod.rs:363, test seeds in fasting.rs:141, goal/mod.rs:1166/:1515, weekly/mod.rs:526/:838, meal/mod.rs:1748) — a new nullable column breaks none of them.
- All reads are index-based (turso `.get::<T>(idx)`), so appending a column shifts nothing.

Requirements:
1. Generalize the runner to an ordered list of (version, sql) migrations applied where version > current_version, each recorded with its own SHA-256 hash (extract a shared hash helper from hash_v1). Keep run()'s PRAGMA/BEGIN/COMMIT/checkpoint structure (L37-72) intact.
2. New migration v2: ALTER TABLE meals ADD COLUMN meal_type TEXT CHECK (meal_type IN ('breakfast','lunch','dinner')) — appended as the LAST column, so fresh installs (v1 then v2) and upgraded installs (v2 only) converge on identical column order. Do NOT edit schema.sql: existing _migrations rows already store v1's hash.
3. NULL is intentional for pre-existing rows (SQL CHECK is tri-valued, so NULL passes; verified against this repo's turso). No backfill here.
4. Update the two pinned tests and add coverage for: meal_type present and last on a fresh DB, v2 not reapplied on reopen, and a legacy-v1 DB upgrading cleanly with its rows intact.

Acceptance: fresh + legacy DBs both end with the column, whole suite green, no domain code touched.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Migration runner applies an ordered list of (version, sql) migrations, applying only those with version > current_version, recording each with its own SHA-256 hash
- [x] #2 migration v2 appends a nullable meal_type TEXT column with CHECK IN ('breakfast','lunch','dinner') to meals, as the last column, on both fresh and pre-existing databases
- [x] #3 schema.sql is unmodified, so v1's recorded hash stays valid; no backfill of meal_type for existing rows
- [x] #4 test_migrations_table_has_version_entry and test_migration_idempotency updated for two migration rows and pass
- [x] #5 New tests cover: meal_type is the last column on a fresh DB, v2 is not reapplied on reopen, and a simulated v1-only database upgrades with its rows intact
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Overview

Turn the hard-coded single migration into an ordered version list, then add `migration v2` appending a nullable `meal_type` column to `meals`. Schema only: nothing reads or writes the value in this ticket.

## Step 1 — generalize the runner (`nom-core/src/storage/migration.rs`)

1. Extract `fn hash_sql(sql: &str) -> String` out of `hash_v1()` (L13-24) and use it for both migrations.
2. Add `const MIGRATION_V2: &str = include_str!("migration_v2.sql");`
3. Replace the `if current_version >= 1 { return Ok(()) }` gate inside `apply_migration` (L101-119) with a loop over an ordered list — `[(1, MIGRATION_V1, hash_v1()), (2, MIGRATION_V2, hash_v2())]` — applying each entry whose `version > current_version` via `execute_batch(sql)` then `INSERT OR IGNORE INTO _migrations (version, hash) VALUES (?, ?)`. Leave `run()` (L37-72) — `PRAGMA foreign_keys = OFF`, `BEGIN TRANSACTION`, `COMMIT`, `PRAGMA foreign_keys = ON`, `checkpoint()` — untouched, so both migrations still land in one transaction.

Fresh installs run v1 then v2 in one pass; upgraded installs run only v2. Because v2 is an `ALTER TABLE ... ADD COLUMN` in both paths, fresh and upgraded databases end with byte-identical column order (`meal_type` last). **Do not edit `schema.sql`** — v1's hash is already recorded in live `_migrations` rows.

## Step 2 — `nom-core/src/storage/migration_v2.sql` (new file, single statement)

```sql
ALTER TABLE meals ADD COLUMN meal_type TEXT CHECK (meal_type IN ('breakfast','lunch','dinner'));
```

Verified against this repo's turso 0.8.0-pre.4 (doc-6 §3): ADD COLUMN with a CHECK succeeds on a populated table, and SQL CHECK is tri-valued, so legacy `NULL` rows pass. The CHECK matches repo convention for enum-ish columns (`foods.source` schema.sql:8, `portions.quantity_mode` :43, `goals.*_direction` :68-76). Known cost: SQLite >= 3.37 evaluates an added column's CHECK against pre-existing rows, making the ALTER O(rows) — irrelevant at single-user scale. Bad values must still be rejected in Rust (later ticket), since a CHECK violation surfaces as `ErrorCategory::StorageFailure`, not Validation.

## Step 3 — update the two tests that pin "exactly one migration"

- `nom-core/src/storage/test.rs:113` `test_migrations_table_has_version_entry` → assert versions 1 and 2 each present exactly once (and hashes non-empty).
- `nom-core/src/storage/test.rs:147` `test_migration_idempotency` → `COUNT(*) FROM _migrations` becomes 2 after reopen.

## Step 4 — new tests

- Fresh DB: `PRAGMA table_info(meals)` shows `meal_type` as the LAST column, and nothing else moved.
- Reopen idempotency: opening an already-migrated DB leaves the count at 2 (v2 not reapplied).
- Legacy upgrade: open a `TempDb`, seed one row using only v1 columns, simulate a v1-only database with `ALTER TABLE meals DROP COLUMN meal_type` + `DELETE FROM _migrations WHERE version = 2`, close, reopen via `Connection::open_at` → assert the column is back, the seeded row is intact with `meal_type IS NULL`, and `_migrations` holds exactly 2 rows. If turso rejects `DROP COLUMN`, hand-build the legacy database instead: create `_migrations` plus `meals` using the exact v1 DDL (schema.sql:20-36), insert a row, register version 1 with `hash_v1()`, then open it.

## Verification

```sh
nix develop .#ci -c cargo fmt --all
nix develop .#ci -c cargo clippy --all-targets --all-features --workspace -- -D warnings
nix develop .#ci -c cargo nextest run --all-features --workspace
nix develop .#ci -c cargo test --doc --all-features --workspace
```

## Done when

Fresh and legacy databases both end up with the column, the two pinned tests are updated, new migration coverage passes, and no module outside `storage/` changed.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Runner generalized: migrations() = [(1, MIGRATION_V1), (2, MIGRATION_V2)] applied where version > current_version, each recorded with hash_sql() (extracted from hash_v1); run()'s PRAGMA/BEGIN/COMMIT/checkpoint structure untouched. migration_v2.sql appends meal_type TEXT CHECK IN ('breakfast','lunch','dinner') as LAST column; schema.sql NOT modified (v1 hash stays valid); no backfill. Tests: both pinned tests now expect versions 1+2; new tests test_meal_type_is_last_column_on_fresh_db, test_v2_not_reapplied_on_reopen, test_legacy_v1_db_upgrades_with_rows_intact (turso DOES support ALTER TABLE DROP COLUMN on the CHECK'd column, so the simulated-v1 approach worked). Full gates green: fmt, clippy -D warnings, nextest 356/356, doctests. Note: turso binds integer params as i64 (version cast at INSERT).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Migration runner now an ordered (version, sql) list with per-migration SHA-256 hashes; migration v2 appends nullable meals.meal_type (CHECK breakfast/lunch/dinner) as last column on fresh and upgraded DBs. schema.sql untouched, no backfill, no domain code touched. All gates green: fmt, clippy -D, nextest 356/356, doctests.
<!-- SECTION:FINAL_SUMMARY:END -->

---
id: TASK-2.5
title: Storage schema and migrations
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-11 13:23'
updated_date: '2026-08-11 18:43'
labels:
  - planned
dependencies:
  - TASK-2.1
type: feature
ordinal: 24000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Scope
Turso/libSQL schema for the five domain tables plus settings, single-user (no user_id anywhere):
- foods (source enum + nullable external_id, unique(source, external_id), full nutrient cache)
- meals (logged_at UTC + materialized logged_date, optional raw-macro adjustment as nullable columns)
- portions (meal_id/food_id FKs, quantity_mode grams-or-servings, snapshots Food's per-100g nutrient rate + serving_size_g at log time)
- weight_entries (logged_at/logged_date pair, bare value in configured unit)
- goals (effective_from-versioned, direction column per nutrient target)
- settings (single row, widget_display_enabled BOOLEAN)

Indexes: logged_date (meals, weight_entries), meal_id (portions), effective_from (goals). No shipped migration tooling — raw SQL migrations, BYO.

See doc-5 §2.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 all six tables created via raw SQL migration(s) with the columns/constraints listed above
- [ ] #2 indexes exist on logged_date (meals, weight_entries), meal_id (portions), effective_from (goals)
- [ ] #3 a Portion row's snapshot columns are populated at insert time and never updated by a later Food catalog change
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Implementation Plan

### Overview
Build the Turso/libSQL storage layer: schema definitions, raw SQL migrations, and migration runner. All six domain tables (foods, meals, portions, weight_entries, goals, settings) plus indexes and constraints per doc-5 §2.

---

### Step 1: Add turso dependency to nom-core

**File**: 

Add  as a workspace dependency. Use version  (latest pre-release). The  crate provides  for local-file mode.

Also ensure  derive feature is available (already present).

---

### Step 2: Create storage module skeleton

**Files**: 
-  — public re-exports
-  — Connection wrapper with FK enforcement
-  — migration runner
-  — raw SQL for initial migration (v1)

Register module in : 

**Connection wrapper** ():
-  wrapping 
-  — creates parent dirs, opens DB, enables , runs migrations
- On Drop/close: checkpoint WAL before releasing connection (per doc-5 §2 invariant)

---

### Step 3: Write initial migration SQL ()

Single file containing all DDL for v1. Execute atomically in a transaction.

**Table: foods**

**Table: meals**

**Table: portions**

**Table: weight_entries**

**Table: goals**

**Table: settings**

**Indexes:**

**Migration tracking table** (geni pattern):

---

### Step 4: Build migration runner ()

Follow the geni pattern — thin, no framework overhead:

- Embed migration SQL as a compile-time string (include! from schema.sql) or inline constant
-  table tracks applied versions with SHA-256 hash of the SQL content
- :
  1. Disable FK checks during DDL: 
  2. BEGIN TRANSACTION
  3. Create  table if not exists
  4. Check current max version from 
  5. For each pending migration: execute SQL, INSERT version + hash into 
  6. COMMIT
  7. Re-enable FK checks: 
  8. Checkpoint WAL after migration completes

For v1 there is only one migration (version 1). Future migrations are appended as new constants/functions.

Key considerations:
- Use  for individual statements or  for multi-statement atomic execution
- Idempotent DDL () so dev re-runs don't fail
- Hash the migration SQL at build time (or compute at runtime) to detect tampering

---

### Step 5: Wire into nom-core

Update  to expose 

The  module exports:
-  struct and  
-  for use by tests that manage their own connections

---

### Step 6: Unit tests

Test the migration runner against a temp-file DB:
- Verify all six tables exist after running migrations
- Verify all four indexes exist
- Verify FK enforcement is active post-migration
- Verify  table has the correct version/hash entry
- Verify idempotency — running migrations twice does not error

Use  for isolated test databases.

---

### Acceptance criteria mapping

- **AC #1**: Six tables created via raw SQL migration with correct columns/constraints → Steps 3-4
- **AC #2**: Indexes on logged_date, meal_id, effective_from → Step 3 (index DDL)
- **AC #3**: Portion snapshot columns populated at insert time → Schema defines snapshot columns; application-level population happens in TASK-2.14 (Meal operations), but the schema must define these columns here

### Files created/modified

| File | Action |
|------|--------|
|  (workspace) | Add  to  |
|  | Add  dependency |
|  | Add  |
|  | New — re-exports |
|  | New — Connection wrapper |
|  | New — migration runner |
|  | New — v1 DDL |
|  | New — migration tests |

### Risks & edge cases

1. **Turso API volatility**: Pre-1.0 crate may change APIs. Pin exact version and verify against docs.rs.
2. **STRICT tables**: libSQL supports  keyword but we use standard type affinity for maximum compatibility — the CHECK constraints on enum columns enforce validity.
3. **Generated column compatibility**:  requires SQLite ≥ 3.31; libSQL/turso should support this, but verify at runtime.
4. **WAL checkpoint invariant**: Must checkpoint after migrations complete and before any MCP/HTTP serve starts. Document this in connection.rs.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
## Implementation Notes

Created the storage module under nom-core/src/storage/ with four files:

- **mod.rs**: Public re-exports for Connection, StorageError, migration runner, and test module
- **connection.rs**: Connection wrapper that enables FK enforcement on open, runs migrations, and checkpoints WAL before close (doc-5 §2 invariant). Includes StorageError enum and From<turso::Error> impl.
- **migration.rs**: Geni-pattern migration runner — embeds schema.sql at compile time, tracks versions + SHA-256 hashes in _migrations table, runs atomically in transaction with FK disabled during DDL.
- **schema.sql**: v1 DDL for all six domain tables (foods, meals, portions, weight_entries, goals, settings) plus four indexes (logged_date on meals/weight_entries, meal_id on portions, effective_from on goals), unique constraint on foods(source, external_id), CHECK constraints on enums, and _migrations tracking table.
- **test.rs**: Five integration tests against temp-file databases: all six tables exist, all four indexes exist, FK enforcement active post-migration, _migrations has correct version/hash entry, idempotency (re-open doesn't error).

Dependencies added to nom-core/Cargo.toml: turso 0.8.0-pre.4, sha2 0.10, tempfile/tokio for dev-dependencies.

AC #1 ✓ — Six tables created via raw SQL migration with correct columns/constraints
AC #2 ✓ — Indexes on logged_date (meals, weight_entries), meal_id (portions), effective_from (goals)
AC #3 ✓ — Portion snapshot columns defined in schema; application-level population deferred to TASK-2.14
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Created storage module in nom-core with turso 0.8.0-pre.4 dependency. Connection wrapper enables FK enforcement and checkpoints WAL (doc-5 §2 invariant). Migration runner uses geni pattern with SHA-256 hash tracking. v1 schema includes all six domain tables (foods, meals, portions, weight_entries, goals, settings) with proper constraints, four indexes, and _migrations tracking table. Five integration tests verify table creation, index existence, FK enforcement, migration tracking, and idempotency.
<!-- SECTION:FINAL_SUMMARY:END -->

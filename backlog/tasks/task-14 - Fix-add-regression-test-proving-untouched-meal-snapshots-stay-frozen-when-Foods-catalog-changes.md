---
id: TASK-14
title: >-
  Fix: add regression test proving untouched meal snapshots stay frozen when
  Foods catalog changes
status: To Do
assignee: []
created_date: '2026-08-12 20:21'
labels:
  - review-followup
dependencies:
  - TASK-2.14
priority: high
ordinal: 160
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while reviewing TASK-2.14 (nom-core/src/meal/mod.rs, build_meal_summary portions query around lines 420-429, and the existing test test_snapshot_semantics_editing_uses_own_snapshot). The ticket's core invariant is that a Portion's nutrient snapshot is captured once at insert/replace time and NEVER re-derived from a live join to the foods table -- 'Previously logged meals are NEVER retroactively updated when Foods catalog changes' per the ticket's own Implementation Plan. The existing test only proves the REPLACE case (update_meal capturing a fresh snapshot for a newly-set portion after the catalog changed). It never proves the other half: that an UNTOUCHED, previously-logged meal's totals stay exactly the same after its underlying Food row's macros are edited. The code (build_meal_summary selects p.snapshot_* columns, joins foods only for f.name) is currently correct, but nothing protects this from a future change (e.g. someone 'simplifying' the join to pull live foods.calories_per_100g instead of the stored snapshot) -- that regression would silently corrupt historical nutrition data and no test would catch it.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A new integration test in nom-core/src/meal/mod.rs logs a meal via log_meal, then directly UPDATEs the foods table row for that meal's food_id to different macro values (bypassing any food operation, straight SQL UPDATE against the foods table used by the test's TempDb), without calling update_meal on the meal itself
- [ ] #2 The test re-reads the meal (via search_meals, get_meals_by_date_range, or equivalent) and asserts its totals/portion snapshot values are UNCHANGED from what was computed at log_meal time, not the new foods table values
- [ ] #3 nix develop -c cargo test -p nom-core passes
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
SETUP (read first): This is a Rust+WebAssembly core (nom-core, nom-mcp) with SQLite storage via the turso crate. ALL commands must run inside the Nix dev shell: either run 'direnv allow' once, or prefix every command with 'nix develop -c'. Work from the repository root unless told otherwise. Do not change pinned dependency versions.

1. Open nom-core/src/meal/mod.rs, find the test module and the existing test test_snapshot_semantics_editing_uses_own_snapshot (search by name) -- copy its TempDb setup and seeded-food pattern.
2. Add a new test named test_snapshot_semantics_untouched_meal_unaffected_by_catalog_change. Seed a food row with known macros (e.g. calories_per_100g=200.0), call log_meal to create a meal with one portion referencing that food, and capture the returned totals.
3. Directly execute a raw SQL UPDATE against the foods table for that food_id changing calories_per_100g (and ideally protein/carbs/fat/fiber too) to clearly different values -- use the same Connection/TempDb the test already has open, a plain conn.execute('UPDATE foods SET calories_per_100g = ? WHERE id = ?', (new_value, food_id)) call. Do NOT call update_meal or any meal operation on the already-logged meal.
4. Re-fetch the meal's summary (via search_meals with a matching query, or get_meals_by_date_range covering the logged_date, or any existing read path in this file) and assert its total_calories (and other totals) still equal the ORIGINAL log_meal-time values, not the new foods table values.
5. Run: nix develop -c cargo test -p nom-core -- meal::tests::test_snapshot_semantics_untouched_meal_unaffected_by_catalog_change --nocapture and confirm it passes, then run the full suite: nix develop -c cargo test -p nom-core
<!-- SECTION:PLAN:END -->

---
id: TASK-14
title: >-
  Fix: add regression test proving untouched meal snapshots stay frozen when
  Foods catalog changes
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-12 20:21'
updated_date: '2026-08-12 22:17'
labels:
  - review-followup
  - planned
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
- [x] #1 A new integration test in nom-core/src/meal/mod.rs logs a meal via log_meal, then directly UPDATEs the foods table row for that meal's food_id to different macro values (bypassing any food operation, straight SQL UPDATE against the foods table used by the test's TempDb), without calling update_meal on the meal itself
- [x] #2 The test re-reads the meal (via search_meals, get_meals_by_date_range, or equivalent) and asserts its totals/portion snapshot values are UNCHANGED from what was computed at log_meal time, not the new foods table values
- [x] #3 nix develop -c cargo test -p nom-core passes
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Single new integration test in nom-core/src/meal/mod.rs, added right after the existing test_snapshot_semantics_editing_uses_own_snapshot (line ~2056).

Test name: test_snapshot_semantics_untouched_meal_unaffected_by_catalog_change

Steps:
1. SETUP — TempDb::new(), Connection::open_at(), seed_food("Almonds") with known macros (calories_per_100g=250.0, protein=20, carbs=30, fat=8, fiber=3), drop conn.
2. LOG MEAL — Call log_meal with one portion: {food_id, quantity: 100.0, quantity_mode: "grams"}. Capture meal_id from result. Expected totals at this point: total_calories = 250.0 (250 * 100/100), total_protein_g = 20.0, etc.
3. CAPTURE ORIGINAL TOTALS — Read log_result["totals"]["total_calories"] etc., or re-fetch immediately via get_meals_by_date_range covering today's date to capture original totals before mutation.
4. MUTATE CATALOG — Open a new Connection, execute raw SQL: UPDATE foods SET calories_per_100g = 999.0, protein_g_per_100g = 99.0, carbs_g_per_100g = 99.0, fat_g_per_100g = 99.0, fiber_g_per_100g = 99.0 WHERE id = ? (food_id). Drop conn. Do NOT call update_meal or any meal operation.
5. RE-FETCH — Use GetMealsByDateRange covering the logged date (or SearchMeals with food name query) which exercises build_meal_summary internally. Assert returned meal's totals.total_calories still equals 250.0 (original value), not 999.0 (mutated catalog value). Same for protein, carbs, fat, fiber totals.
6. ASSERT — Use abs() < 0.01 tolerance for float comparisons. Message should clearly state that untouched meal stayed frozen despite catalog change.

Then run: nix develop -c cargo test -p nom-core -- meal::tests::test_snapshot_semantics_untouched_meal_unaffected_by_catalog_change --nocapture followed by full suite: nix develop -c cargo test -p nom-core
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Added test_snapshot_semantics_untouched_meal_unaffected_by_catalog_change in nom-core/src/meal/mod.rs (line ~2039). Test: seeds Almonds food, logs a 100g meal capturing original totals (250 cal), directly SQL-UPDATEs foods table to 999 cal/100g, re-fetches via GetMealsByDateRange, asserts all totals unchanged. Validates snapshot freeze invariant that build_meal_summary uses stored portion snapshots, not live food macros.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added regression test test_snapshot_semantics_untouched_meal_unaffected_by_catalog_change that proves meal snapshots stay frozen when the foods catalog is mutated. Test logs a meal, directly UPDATEs the foods table to different macro values, re-fetches via GetMealsByDateRange, and asserts totals remain unchanged. All 154 nom-core tests pass.
<!-- SECTION:FINAL_SUMMARY:END -->

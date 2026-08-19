---
id: TASK-58.1
title: >-
  Extend log_meal response with per-portion macro breakdown and post-insertion
  daily totals
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-19 01:50'
updated_date: '2026-08-19 03:38'
labels:
  - task
dependencies: []
parent_task_id: TASK-58
priority: high
ordinal: 67000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Extends log_meal's response additively (all existing fields unchanged) with two new top-level keys.

(1) 'portions': per-portion breakdown using the existing PortionSummary shape (id, food_id, food_name, quantity_mode, quantity, calories, protein_g, carbs_g, fat_g, fiber_g). Implementation: after COMMIT, read back via the existing build_meal_summary(&conn, meal_id) and take .portions — no signature change to the shared resolve_portions() helper (which currently discards food names), and guarantees the response matches stored state. Adjustment is meal-level and correctly stays out of portion macros.

(2) 'daily_totals': post-insertion per-nutrient progress scoped to logged_date, computed AFTER commit on the same open connection so it reflects the new meal. Shape: 5 x goal::NutrientProgress under keys calories/protein_g/carbs_g/fat_g/fiber_g (consumed/target/remaining/percent/direction/status; Option fields omitted when null). Weight is excluded — logging a meal cannot change weight progress. Requires a new pub(crate) helper in nom-core/src/goal/mod.rs (e.g. struct DailyNutrientProgress + async fn daily_nutrient_progress(conn, date)) built on the existing private fetch_active_goal + fetch_consumed_totals and pub(crate) nutrient_progress; extract GetGoalProgress's inline direction-parse closure into a shared function to avoid duplication. Do NOT refactor GetGoalProgress itself (it still needs the goal row for target_weight) — keep the diff focused. No-goal case must match get_goal_progress exactly: consumed populated, target/remaining/percent/status null. Post-commit read failures propagate as storage_failure (consistent with the codebase's no-partial-success semantics; the meal is persisted either way).

<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Multi-portion log_meal response includes a 'portions' array with correct food names + macros for grams and servings modes; all pre-existing fields (meal_id/logged_at/logged_date/totals) unchanged.
- [x] #2 'daily_totals' sums pre-seeded same-date meals + the new meal including any adjustment; with an active goal, target/percent/status are correct per nutrient; without a goal, consumed-only with null target-derived fields.
- [x] #3 Tests in the meal module cover: multi-portion response shape, daily_totals with and without an active goal, adjustment flowing into daily_totals but not into portion macros.
- [x] #4 cargo nextest run --all-features --workspace + cargo test --doc pass; fmt/clippy clean.
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- `resolve_portions()` now retains the food name returned by `lookup_food()` (previously discarded as `_name`) so the response can carry a per-portion breakdown; quantity mode (grams/servings) and any adjustment are applied per portion.
- New `goal::daily_nutrient_progress()` helper shares the row-mapping logic with get_goal_progress, so `daily_totals` uses the identical per-nutrient progress shape (consumed/target/percent/status; target-derived fields null when no active goal).
- Post-commit read-back: daily totals are computed after the meal insert, so they include the newly logged meal.
- 4 new tests in the meal module: multi-portion response shape, daily_totals with and without an active goal, adjustment flowing into daily_totals but not into portion macros. Full CI gate green (fmt, clippy -D warnings, nextest, doctests); payload shape additionally verified end-to-end on the TASK-54 seeded instance.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Shipped TASK-58.1: log_meal's response is extended additively with `portions` (per-portion food name + calories/protein/carbs/fat/fiber) and `daily_totals` (post-insertion daily totals in the same per-nutrient progress shape as get_goal_progress). All pre-existing response fields unchanged, so all four surfaces (CLI/HTTP/MCP/remote) pick up the fields automatically through the registry. All 4 ACs verified; CI green.
<!-- SECTION:FINAL_SUMMARY:END -->

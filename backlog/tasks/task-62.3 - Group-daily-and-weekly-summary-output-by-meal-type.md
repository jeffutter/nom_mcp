---
id: TASK-62.3
title: Group daily and weekly summary output by meal type
status: Done
assignee:
  - '@ralph'
created_date: '2026-09-07 01:28'
updated_date: '2026-09-07 02:57'
labels:
  - task
  - planned
dependencies:
  - TASK-62.2
parent_task_id: TASK-62
priority: medium
type: task
ordinal: 72000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Summary slice of TASK-62 (AC #7): daily and weekly summary outputs group entries by meal type so a reader can tell breakfast from lunch from dinner. Depends on TASK-62.2 for stored values and the MealType type.

Scope: one new shared aggregate over meals grouped by (logged_date, meal_type), consumed by both the Weekly Summary (nested per-day breakdown in nutrients.daily_totals, which flows to the nom://weekly-summary resource and get_weekly_progress automatically) and get_goal_progress (per-meal-type section for the requested date). Existing day-level fields and widget bindings stay exactly as they are — this is additive JSON shape.

Explicitly avoid duplicating meal aggregation SQL: derive the existing per-day totals in weekly from the same grouped rows rather than leaving a second SUM query beside it (TASK-31 exists precisely because weight-summary mapping got duplicated once).

Out of scope: widgets (TASK-62.5), docs (TASK-62.4), changing goal's day-level NutrientProgress or fetch_consumed_totals.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 One shared aggregate groups meals by (logged_date, meal_type) and is consumed by both weekly and goal modules — no duplicated meal SUM SQL
- [x] #2 Weekly Summary daily_totals gain a by_meal_type breakdown while existing day-level fields, ordering and days_with_data semantics stay byte-compatible for widgets
- [x] #3 get_goal_progress gains a per-meal-type section for the requested date without changing its whole-day NutrientProgress values
- [x] #4 Both the nom://weekly-summary resource JSON and get_weekly_progress tool output expose the new breakdown
- [x] #5 Legacy rows with NULL meal_type appear in a null-labelled bucket and still count toward day totals
- [x] #6 Tests cover multi-type/multi-day splits, empty results ([] not null), legacy null bucket, and unchanged day totals
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Overview

Add a per-meal-type breakdown to daily and weekly summaries with ONE new query shared by both consumers. Day-level output stays byte-compatible so widgets keep binding.

## Step 1 — shared aggregate (`nom-core/src/meal_type.rs`)

```rust
pub struct DateMealTypeTotals {
    pub date: String,
    pub meal_type: Option<String>, // None = legacy row logged before the field existed
    pub calories: f64, pub protein_g: f64, pub carbs_g: f64, pub fat_g: f64, pub fiber_g: f64,
}

pub async fn fetch_totals_by_date_and_meal_type(
    conn: &Connection, start_date: &str, end_date: &str,
) -> Result<Vec<DateMealTypeTotals>, ErrorData>
```

SQL:

```sql
SELECT logged_date, meal_type,
       COALESCE(SUM(total_calories), 0.0), COALESCE(SUM(total_protein_g), 0.0),
       COALESCE(SUM(total_carbs_g), 0.0),  COALESCE(SUM(total_fat_g), 0.0),
       COALESCE(SUM(total_fiber_g), 0.0)
FROM meals
WHERE logged_date BETWEEN ? AND ?
GROUP BY logged_date, meal_type
ORDER BY logged_date, meal_type
```

Read rows index-based (turso has no name-keyed access); `NULL` meal_type via the `turso::Value::Null` pattern at meal/mod.rs:231-241. Mirror the error handling style of `weekly::fetch_daily_totals` (weekly/mod.rs:381-433) — propagate the storage error, never silently drop rows (see TASK-27).

## Step 2 — Weekly Summary (`nom-core/src/weekly/mod.rs`)

Replace `fetch_daily_totals()` (L381-433, currently `GROUP BY logged_date` only) with a fold over the shared rows: per `(date, meal_type)` rows accumulate into `DailyTotals`, summing to the existing five per-day fields in Rust. Constraints:

- Keep `DailyTotals`' existing fields, names, ordering by `logged_date`, and semantics identical — `weekly_progress_widget.html` binds `daily_totals[].date/calories/...` (asset L261) and `test_get_weekly_progress_matches_fetch_weekly_summary` (L804) compares tool output against the struct.
- `days_with_data` keeps counting days whose total calories > 0 (L128) and averages stay `sum / 7.0` (L121-127).
- Add exactly one new field: `by_meal_type: Vec<MealTypeTotals>` on `DailyTotals` (L56-64), where `MealTypeTotals { meal_type: Option<String>, calories, protein_g, carbs_g, fat_g, fiber_g }` serializes breakfast/lunch/dinner in that order with any legacy `null` bucket last. Use a plain `Vec` (not `Option<Vec<_>>`) so empty days emit `[]`, not `null`.

Both consumers serialize `WeeklySummary` verbatim — the resource path (`mcp_handler.rs:410` `serde_json::to_string(&summary)`, branch at :469-484) and `get_weekly_progress` (`weekly/mod.rs:284`) — so the new field appears in both with no further change.

## Step 3 — daily progress (`nom-core/src/goal/mod.rs`)

Leave `fetch_consumed_totals()` (L208-244) and `NutrientProgress` untouched; the rings still show whole-day totals. Add `meals_by_type: Vec<MealTypeTotals>` to `GoalProgress` (L72-91), populated by calling the shared helper with `start_date == end_date == date` near the existing body (L762-813). Two queries per call is acceptable and worth noting in the doc comment; do not refactor `fetch_consumed_totals` in this ticket. `goal_progress_widget.html` ignores unknown fields (it branches on `data.variant`, asset L402), so no widget work is required here.

## Step 4 — tests (`#[cfg(test)] mod tests` in each module, `TempDb` + local `seed_meal_at` helpers at weekly/mod.rs:838 and goal/mod.rs:1515 — extend those helpers to take a meal type)

- weekly: seed three meal types across two days → assert (a) `daily_totals` day sums equal today's expected values, (b) `by_meal_type` splits match per type, (c) ordering is by date then breakfast/lunch/dinner. Empty DB → `by_meal_type` is `[]`. Keep `test_daily_totals_ordering` (L716) green; update `test_get_weekly_progress_matches_fetch_weekly_summary` (L804) if it pins literals.
- legacy rows: seed a meal inserted WITHOUT `meal_type` → its macros land in the day total AND in a trailing entry whose `meal_type` serializes as `null` (nothing silently vanishes from the aggregate).
- goal: `get_goal_progress` on a date with breakfast + dinner → two entries, day-level `NutrientProgress.consumed` unchanged.
- resource: assert the `nom://weekly-summary` JSON contains `nutrients.daily_totals[0].by_meal_type` (extend the existing `mcp_handler` resource tests, cf. TASK-33's coverage).

## Verification

```sh
nix develop .#ci -c cargo fmt --all
nix develop .#ci -c cargo clippy --all-targets --all-features --workspace -- -D warnings
nix develop .#ci -c cargo nextest run --all-features --workspace
nix develop .#ci -c cargo test --doc --all-features --workspace
```

## Done when

Both summaries label/group by meal type (AC #7), legacy NULL rows are visible rather than dropped, day-level output is unchanged for existing consumers, and CI is green.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented as planned. nom-core/src/meal_type.rs gains fetch_days_by_meal_type() -> Vec<DayByMealType>: one GROUP BY logged_date, meal_type query, private TotalsRow wire shape, fold in Rust that sums whole-day totals and orders buckets breakfast/lunch/dinner with the legacy NULL bucket last (SQLite sorts NULL first, so ordering happens in Rust via bucket_rank). weekly::fetch_daily_totals (the second SUM query) is deleted; DailyTotals is built From<DayByMealType> and gains only by_meal_type: Vec<MealTypeTotals>. goal::GetGoalProgress gains meals_by_type, populated by the same helper with start==end; fetch_consumed_totals and NutrientProgress untouched. Tests: 4 pure fold/serde tests + 2 DB-backed helper tests in meal_type.rs; 3 weekly tests (multi-type/multi-day split, legacy null bucket, get_weekly_progress exposure); 2 goal tests (split incl. neighbour-day isolation and unchanged consumed, empty == []); 1 mcp_handler resource test asserting nutrients.daily_totals[0].by_meal_type in the nom://weekly-summary JSON.

Fixup applied post-review: fetch_days_by_meal_type's SQL ordered only by logged_date, not (logged_date, meal_type). fold_rows() requires all rows for a date to be contiguous, but ORDER BY logged_date alone leaves tie order among same-date meal_type rows unspecified by SQL semantics — it only worked because SQLite's current GROUP BY implementation happens to emit rows pre-sorted by the full grouping key. Added meal_type as a secondary ORDER BY key to make the contiguity guarantee explicit rather than relying on undocumented engine behavior. All 379 tests + fmt + clippy -D warnings still pass.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
One shared (logged_date, meal_type) aggregate now feeds both summaries. meal_type::fetch_days_by_meal_type is its only SQL; weekly's duplicate per-day SUM query is gone. Weekly Summary daily_totals entries gained by_meal_type and get_goal_progress gained meals_by_type; day-level fields, ordering, days_with_data and ring values are untouched. Legacy NULL rows land in a trailing null-labelled bucket and still count toward day totals. 11 new tests (3 pure fold/serde + 2 DB-backed in meal_type.rs, 3 weekly, 2 goal, 1 MCP resource). Gate green: fmt, clippy -D warnings, 379 nextest tests, doctests. Verified live on a seeded temp DB: nom://weekly-summary resource JSON and get_weekly_progress tool output are byte-identical and both carry the breakdown.
<!-- SECTION:FINAL_SUMMARY:END -->

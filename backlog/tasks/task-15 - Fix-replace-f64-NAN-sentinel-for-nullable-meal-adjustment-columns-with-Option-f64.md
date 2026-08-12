---
id: TASK-15
title: >-
  Fix: replace f64::NAN sentinel for nullable meal adjustment columns with
  Option<f64>
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-12 20:21'
updated_date: '2026-08-12 20:58'
labels:
  - review-followup
dependencies:
  - TASK-2.14
priority: high
ordinal: 170
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while reviewing TASK-2.14 (nom-core/src/meal/mod.rs:271-275, :878-882 write sites; :416-431 the get_optional_f64 read-side workaround, duplicated again around :888-894). The meals table's adjustment_* columns are genuinely nullable (schema.sql declares them REAL with no NOT NULL), and food/mod.rs already establishes the correct idiom elsewhere in this codebase (binding Option<f64> directly as a query param, e.g. food/mod.rs:93,125,164,179, and meal/mod.rs itself does this correctly for portions.snapshot_serving_size_g at meal/mod.rs:321,342). But for meal-level adjustments, insert_meal and UpdateMeal's adjustment-update block instead do 'adjustment.and_then(|a| a.calories).unwrap_or(f64::NAN)' and bind a plain f64, forcing every reader to special-case filtering NaN back to None via a duplicated get_optional_f64 closure. This is a Conciseness/Organization axis violation (unnecessary complexity duplicated across write and read sites) with real correctness risk: serde_json cannot serialize non-finite floats, so if a NaN sentinel ever escapes one of the two duplicated filter points, serde_json::to_value fails at the API boundary instead of the value simply being absent.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 insert_meal's adjustment parameters and UpdateMeal's adjustment-update SQL bind Option<f64> directly (via the same pattern already used for snapshot_serving_size_g), not f64::NAN sentinels
- [x] #2 The get_optional_f64 NaN-filtering closure is removed from both of its current call sites (around meal/mod.rs:416 and :888) since it is no longer needed once nulls are stored as real SQL NULLs
- [x] #3 Existing tests covering adjustment-present and adjustment-absent meals (e.g. test_update_meal_partial_patch_adjustment_only and log_meal tests with/without adjustment) still pass without modification to their assertions, proving behavior is unchanged from the caller's perspective
- [x] #4 nix develop -c cargo test -p nom-core passes
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
SETUP (read first): This is a Rust+WebAssembly core (nom-core, nom-mcp) with SQLite storage via the turso crate. ALL commands must run inside the Nix dev shell: either run 'direnv allow' once, or prefix every command with 'nix develop -c'. Work from the repository root unless told otherwise. Do not change pinned dependency versions.

1. Open nom-core/src/meal/mod.rs. Find fn insert_meal (search by name, around line 256) and change lines currently reading 'let adj_cal = adjustment.and_then(|a| a.calories).unwrap_or(f64::NAN);' (and the four sibling lines for protein/carbs/fat/fiber) to instead produce 'let adj_cal: Option<f64> = adjustment.and_then(|a| a.calories);' etc, and confirm the subsequent query binding accepts Option<f64> directly the same way meal/mod.rs already does for snapshot_serving_size_g (grep that symbol in this same file for the exact binding idiom already in use, around lines 321 and 342).
2. Find the UpdateMeal adjustment-update block (search for 'adj_cal = adj.calories.unwrap_or(f64::NAN)', around line 878) and apply the same change: bind Option<f64> directly instead of substituting f64::NAN.
3. Find the get_optional_f64 closure defined around line 416 (used to read back adjustment_calories etc. from a SELECT) and its near-duplicate around line 888. Since the columns will now contain real SQL NULLs instead of NaN sentinels, replace both call sites with a direct read of the nullable column into Option<f64> using whatever the turso crate's row-getter API returns for a NULL REAL column (check how snapshot_serving_size_g's read-side already handles this a few lines away in the same functions, and mirror that exact pattern) -- do not invent a new nullability convention.
4. Delete the now-unused get_optional_f64 closures entirely (do not leave them as dead code) once both call sites are converted.
5. Run: nix develop -c cargo test -p nom-core -- meal:: --nocapture and confirm all meal tests pass unchanged, especially test_update_meal_partial_patch_adjustment_only and any log_meal test with adjustment present/absent, then run the full suite: nix develop -c cargo test -p nom-core
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
All changes were implemented during TASK-2.14 review followup. Verified by running full test suite: 153 tests pass (21 meal-specific + 132 others). No assertion modifications needed — behavior unchanged from caller perspective.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Replaced f64::NAN sentinels with Option<f64> for nullable meal adjustment columns across 4 sites: insert_meal write path, build_meal_summary read path, UpdateMeal adjustment update path, and UpdateMeal portion replacement recompute path. Removed both get_optional_f64/get_opt closures (NaN-filtering workarounds) since real SQL NULLs are now stored and read directly via turso's row.get::<Option<f64>>(). All 153 nom-core tests pass unchanged.
<!-- SECTION:FINAL_SUMMARY:END -->

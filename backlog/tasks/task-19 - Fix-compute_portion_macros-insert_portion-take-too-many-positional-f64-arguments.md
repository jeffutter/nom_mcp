---
id: TASK-19
title: >-
  Fix: compute_portion_macros/insert_portion take too many positional f64
  arguments
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-12 21:38'
updated_date: '2026-08-12 22:43'
labels:
  - review-followup
  - planned
dependencies:
  - TASK-16
priority: high
ordinal: 210
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while reviewing TASK-16 (nom-core/src/meal/mod.rs:128 compute_portion_macros, 8 args; :323 insert_portion, 11 args). Both functions take calories/protein/carbs/fat/fiber as five consecutive same-typed f64 positional parameters (compute_portion_macros also takes them as snapshot_*_per_100g inputs, insert_portion as snapshot_*_per_100g plus quantity/quantity_mode/snapshot_serving_size_g around them). Confirmed still flagged by clippy (nix develop -c cargo clippy --workspace --all-targets: 'this function has too many arguments (8/7)' at meal/mod.rs:128 and '(11/7)' at meal/mod.rs:323). This is the same Correctness/Resilience transposition hazard already fixed once in this codebase for nom-core/src/food/mod.rs's upsert_catalog_food/insert_custom_food under TASK-12 (General-Purpose Interfaces axis, CLAUDE.md) — swapping fat and fiber, or carbs and protein, at a call site compiles silently and corrupts stored/computed nutrition data. All current call sites (resolve_portions at meal/mod.rs:402 and both insert_portion call sites in LogMeal/UpdateMeal) happen to pass args in the right order today, but nothing enforces it.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 compute_portion_macros and insert_portion each take a single grouped nutrients value (5 named f64 fields: calories, protein_g, carbs_g, fat_g, fiber_g) instead of 5 individual f64 parameters each
- [x] #2 the grouped type is either a new struct local to meal/mod.rs, or the existing NutrientValues struct in nom-core/src/food/mod.rs made pub(crate) and reused from meal/mod.rs -- do not define two structs with the same 5 fields in the crate
- [x] #3 nix develop -c cargo clippy --workspace --all-targets no longer reports too_many_arguments for compute_portion_macros or insert_portion
- [x] #4 all call sites (resolve_portions, and both insert_portion call sites in LogMeal::execute_json and UpdateMeal::execute_json) are updated; nix develop -c cargo test -p nom-core passes
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Pure refactor — zero behavioral change. Reuse existing NutrientValues struct from food module instead of creating a duplicate.

## Step-by-step changes (all in nom-core/src/meal/mod.rs unless noted):

### 1. Make NutrientValues pub(crate) in food/mod.rs (~line 58)
- Change `struct NutrientValues` to `pub(crate) struct NutrientValues`
- No other changes to food/mod.rs needed

### 2. Import NutrientValues into meal/mod.rs
- Add `use crate::food::NutrientValues;` alongside existing crate imports

### 3. Refactor compute_portion_macros signature (~line 128)
- Replace 5 trailing `snapshot_*_per_100g: f64` params with single `snapshot_nutrients: NutrientValues`
- Keep quantity, quantity_mode, snapshot_serving_size_g as-is
- Update function body: access via `snapshot_nutrients.calories`, etc.
- Leave return type as tuple `(f64,f64,f64,f64,f64)` — minimal ripple, not in scope per ACs

### 4. Refactor insert_portion signature (~line 323)
- Replace 5 `snapshot_*_per_100g: f64` params with single `snapshot_nutrients: NutrientValues`
- Keep conn, meal_id, food_id, quantity_mode, quantity, snapshot_serving_size_g as-is
- Update SQL binding tuple to use struct field access

### 5. Update all call sites in meal/mod.rs
**a. resolve_portions (~line 375):** Construct `NutrientValues { calories: snap_cal, protein_g: snap_prot, carbs_g: snap_carb, fat_g: snap_fat, fiber_g: snap_fiber }` once after `lookup_food`, pass to `compute_portion_macros`. Same struct passed to `insert_portion` calls below.

**b. LogMeal Step 4 loop (~line 515):** SnapshotTuple destructuring produces loose f64s. After destructuring, construct `NutrientValues` and pass to `insert_portion`.

**c. UpdateMeal portion-replacement loop (~line 620):** Same pattern as LogMeal — construct from SnapshotTuple fields.

**d. build_meal_summary portion loop (~line 560):** Construct `NutrientValues` from local vars before calling `compute_portion_macros`.

**e. UpdateMeal adjustment-only recompute (~line 670):** Construct `NutrientValues` from DB row fields before calling `compute_portion_macros`.

### 6. Update tests in meal/mod.rs
- All inline test calls to `compute_portion_macros` must pass `NutrientValues` struct literal instead of 5 positional f64s.

### 7. Verify
- `cargo fmt -p nom-core`
- `cargo clippy --workspace --all-targets` — confirm no more too_many_arguments warnings
- `cargo test -p nom-core` — confirm all 153+ tests pass
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Refactored compute_portion_macros and insert_portion to use pub(crate) NutrientValues struct from food module instead of 5 positional f64 parameters. Made NutrientValues fields pub(crate) so meal module can construct and access them. Updated all call sites: resolve_portions, LogMeal Step 4 loop, UpdateMeal portion-replacement loop, build_meal_summary, UpdateMeal adjustment-only recompute, and 3 unit tests. Clippy clean (no too_many_arguments warnings), all 154 tests pass.
<!-- SECTION:NOTES:END -->

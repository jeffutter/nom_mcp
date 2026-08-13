---
id: TASK-20
title: >-
  Fix: compute_portion_macros/compute_totals return a bare positional f64 tuple,
  reintroducing the transposition hazard TASK-19 fixed on the input side
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-12 22:47'
updated_date: '2026-08-12 23:00'
labels:
  - review-followup
  - planned
dependencies:
  - TASK-19
priority: high
ordinal: 220
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while reviewing TASK-16/TASK-19 (nom-core/src/meal/mod.rs:129-134 compute_portion_macros returns (f64,f64,f64,f64,f64); :155-156 compute_totals takes &[(f64,f64,f64,f64,f64)]). TASK-19 grouped the calories/protein/carbs/fat/fiber *input* params into the NutrientValues struct specifically to close a transposition hazard (swapping fat and fiber, or carbs and protein, compiles silently and corrupts nutrition data), but explicitly left the *return* type as a bare 5-tuple ('minimal ripple, not in scope per ACs'). The same hazard now lives on the output side: three non-test call sites destructure this tuple positionally by pattern -- compute_totals's 'for (cal, prot, carb, fat, fiber) in portions' loop (meal/mod.rs:167), build_meal_summary's 'let (cal, prot, carb, fat, fiber) = compute_portion_macros(...)' (meal/mod.rs:582), and UpdateMeal's adjustment-only recompute pushing directly into all_macros (meal/mod.rs:1063) which compute_totals later destructures. A reordered destructure or a reordered tuple construction at any of these sites would compile and silently swap nutrition fields, identical in kind to the bug class TASK-12 and TASK-19 were filed to close. This is a Correctness/Resilience finding (CLAUDE.md 'General-Purpose Interfaces' / 'Define Errors Out of Existence' -- make the illegal transposition state unrepresentable rather than trusting positional discipline).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 compute_portion_macros returns a named struct (reuse the existing NutrientValues struct from nom-core/src/food/mod.rs, matching the pattern TASK-19 already established for its input parameter) instead of a bare (f64,f64,f64,f64,f64) tuple
- [x] #2 compute_totals's portions parameter is &[NutrientValues] (or equivalent named type) instead of &[(f64,f64,f64,f64,f64)], and its accumulation loop accesses named fields instead of positional tuple destructuring
- [x] #3 the three call sites (compute_totals's loop at meal/mod.rs:167, build_meal_summary at meal/mod.rs:582, and UpdateMeal's adjustment-only recompute around meal/mod.rs:1063) are updated accordingly; no positional 5-f64-tuple destructuring of macro values remains in non-test code
- [x] #4 nix develop -c cargo clippy --workspace --all-targets is clean
- [x] #5 nix develop -c cargo test -p nom-core passes
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Pure refactor — zero behavioral change. All changes in nom-core/src/meal/mod.rs. Reuse existing pub(crate) NutrientValues from food module (already imported at meal/mod.rs:21).

## Step-by-step changes (all in nom-core/src/meal/mod.rs):

### 1. compute_portion_macros return type (~line 129-151)
- Change return type from `(f64, f64, f64, f64, f64)` to `NutrientValues`
- Replace tuple literal construction with struct: `NutrientValues { calories: snapshot_nutrients.calories * factor, protein_g: ..., carbs_g: ..., fat_g: ..., fiber_g: ... }`

### 2. compute_totals signature (~line 155-178)
- Change `portions: &[(f64, f64, f64, f64, f64)]` to `portions: &[NutrientValues]`
- Replace positional destructure loop `for (cal, prot, carb, fat, fiber) in portions` with `for n in portions` and accumulate via named fields (`n.calories`, `n.protein_g`, etc.)

### 3. Remove MacroTuple type alias (~line 356-357)
- Delete the `type MacroTuple = (f64, f64, f64, f64, f64);` line and its doc comment
- This alias provided zero value since it was just a name around the same bare tuple

### 4. resolve_portions (~lines 371-420)
- Change return type from `Result<(Vec<MacroTuple>, Vec<SnapshotTuple>), ErrorData>` to `Result<(Vec<NutrientValues>, Vec<SnapshotTuple>), ErrorData>`
- Change `let mut all_macros: Vec<MacroTuple>` to `let mut all_macros: Vec<NutrientValues>`
- The `compute_portion_macros()` call already assigns to `macros` variable; no destructure change needed since it now returns NutrientValues directly

### 5. build_meal_summary (~line 582)
- Replace `let (cal, prot, carb, fat, fiber) = compute_portion_macros(...)` with `let macros = compute_portion_macros(...)`
- Use `macros.calories`, `macros.protein_g`, `macros.carbs_g`, `macros.fat_g`, `macros.fiber_g` when constructing PortionSummary

### 6. UpdateMeal adjustment-only recompute (~line 1005)
- Change `let mut all_macros: Vec<(f64, f64, f64, f64, f64)>` to `let mut all_macros: Vec<NutrientValues>`
- The `all_macros.push(compute_portion_macros(...))` at ~line 1063 needs no code change beyond the type flowing through

### 7. Unit tests (~lines 1528-1573)
- `test_compute_portion_macros_grams_mode`: replace `let (cal, prot, carb, fat, fiber) = compute_portion_macros(...)` with `let result = compute_portion_macros(...)` and access `result.calories`, `result.protein_g`, etc.
- `test_compute_portion_macros_servings_mode`: same pattern
- `test_compute_portion_macros_servings_no_serving_size`: same pattern (only checks calories)
- `test_compute_totals_basic`: change test data from tuple literals `(100.0, 10.0, 15.0, 5.0, 2.0)` to `NutrientValues { calories: 100.0, protein_g: 10.0, ... }`
- `test_compute_totals_with_adjustment`: same pattern for the single portion

### 8. Verification steps
- `nix develop -c cargo fmt -p nom-core`
- `nix develop -c cargo clippy --workspace --all-targets` — confirm clean
- `nix develop -c cargo test -p nom-core` — confirm all 154 tests pass
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Pure refactor in nom-core/src/meal/mod.rs. Changed compute_portion_macros return type from (f64,f64,f64,f64,f64) to NutrientValues struct; updated compute_totals signature, resolve_portions, build_meal_summary, UpdateMeal adjustment-only recompute, and 5 unit tests. Removed MacroTuple type alias. Zero behavioral change — all 154 tests pass, clippy clean.
<!-- SECTION:NOTES:END -->

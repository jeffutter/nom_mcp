---
id: TASK-19
title: >-
  Fix: compute_portion_macros/insert_portion take too many positional f64
  arguments
status: To Do
assignee: []
created_date: '2026-08-12 21:38'
labels:
  - review-followup
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
- [ ] #1 compute_portion_macros and insert_portion each take a single grouped nutrients value (5 named f64 fields: calories, protein_g, carbs_g, fat_g, fiber_g) instead of 5 individual f64 parameters each
- [ ] #2 the grouped type is either a new struct local to meal/mod.rs, or the existing NutrientValues struct in nom-core/src/food/mod.rs made pub(crate) and reused from meal/mod.rs -- do not define two structs with the same 5 fields in the crate
- [ ] #3 nix develop -c cargo clippy --workspace --all-targets no longer reports too_many_arguments for compute_portion_macros or insert_portion
- [ ] #4 all call sites (resolve_portions, and both insert_portion call sites in LogMeal::execute_json and UpdateMeal::execute_json) are updated; nix develop -c cargo test -p nom-core passes
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
SETUP (read first): This is a Rust+WebAssembly core (nom-core, nom-mcp). ALL commands must run inside the Nix dev shell: either run 'direnv allow' once, or prefix every command with 'nix develop -c'. Work from the repository root unless told otherwise. Do not change pinned dependency versions.

1. Read nom-core/src/meal/mod.rs in full, especially compute_portion_macros (~line 128), insert_portion (~line 323), resolve_portions (~line 375), and the MacroTuple/SnapshotTuple type aliases just above resolve_portions (~lines 363-367).
2. Decide struct reuse vs new struct: nom-core/src/food/mod.rs already defines a private 'struct NutrientValues { calories, protein_g, carbs_g, fat_g, fiber_g }' (all f64, derives Debug/Clone/Copy) at food/mod.rs:58. Prefer making that struct 'pub(crate)' and importing it into meal/mod.rs (e.g. 'use crate::food::NutrientValues;') over duplicating an identical struct -- same shape, same crate. If food/mod.rs's module structure makes that awkward, a new equivalent struct in meal/mod.rs is acceptable, but note the duplication tradeoff in the commit message.
3. Change compute_portion_macros's signature: replace the 5 trailing snapshot_*_per_100g: f64 params with a single 'snapshot: NutrientValues' param (keep quantity, quantity_mode, snapshot_serving_size_g as-is). Update the function body to read snapshot.calories, snapshot.protein_g, etc. Its return type '(f64,f64,f64,f64,f64)' can stay as-is (that's the MacroTuple output, not in scope for this ticket) or also be converted to NutrientValues if trivial -- prefer converting it too for consistency, updating the MacroTuple alias and its two call sites (compute_totals's portions: &[MacroTuple] param, and resolve_portions's all_macros accumulation) accordingly. If converting the return type turns out to ripple beyond meal/mod.rs, leave the return type as the tuple and only change the input params.
4. Change insert_portion's signature: replace the 5 snapshot_*_per_100g: f64 params with a single 'snapshot: NutrientValues' param, keeping conn, meal_id, food_id, quantity_mode, quantity, and snapshot_serving_size_g as individual params. Update the SQL binding tuple in the function body to use snapshot.calories etc.
5. Update resolve_portions (~line 375) to construct a NutrientValues from lookup_food's return values once and pass it to compute_portion_macros, instead of passing 5 loose f64s.
6. Update both insert_portion call sites (LogMeal::execute_json's Step 4 loop, and UpdateMeal::execute_json's portion-replacement loop) to pass a NutrientValues instead of 5 loose *.
7. Update the SnapshotTuple type alias and its two destructuring for-loops if the snapshot tuple's 5 trailing f64 fields are also grouped -- this is optional cleanup, not required by the ACs, do it only if it stays a small, mechanical change.
8. Run: nix develop -c cargo fmt -p nom-core
9. Run: nix develop -c cargo clippy --workspace --all-targets -- confirm no too_many_arguments warning remains for these two functions.
10. Run: nix develop -c cargo test -p nom-core -- confirm all tests still pass (153 at time of writing).
<!-- SECTION:PLAN:END -->

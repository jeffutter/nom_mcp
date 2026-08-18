---
id: TASK-61
title: 'Fix: seed_data reimplements meal::compute_portion_macros instead of reusing it'
status: To Do
assignee: []
created_date: '2026-08-18 03:30'
labels:
  - review-followup
dependencies:
  - TASK-54
priority: high
ordinal: 100
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while reviewing TASK-54 (nom-core/src/seed/mod.rs:113-122, portion_macros()). SeedData reimplements meal::compute_portion_macros's exact grams-mode formula (snapshot_X_per_100g * quantity / 100.0) using a separate tuple-based representation, instead of calling the real function every log_meal-driven read actually uses. The seed module's own comment at :111-112 explicitly acknowledges the duplication ('identical formula to meal::compute_portion_macros'). This is the 'shared knowledge, extract it' case from the Rust best-practices handbook (Ch.1 §1.8) and CLAUDE.md's 'information leakage' red flag: the definition of how a portion's macros are computed is one business decision that must change everywhere at once. If meal::compute_portion_macros's formula ever changes (new nutrient, rounding, unit handling, servings-mode edge case), seed/mod.rs's fixture data will silently diverge from what real log_meal-produced data looks like -- nothing exercises both code paths together, and seed's own unit tests pin hardcoded expected totals so they would keep passing even after the drift. Root cause: meal::compute_portion_macros (nom-core/src/meal/mod.rs:129) is a private fn, so the sibling seed module could not call it and duplicated it instead. Axis: Organized (information leakage / duplicated algorithm across modules).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 nom-core/src/meal/mod.rs's compute_portion_macros is pub(crate) (visible to nom-core/src/seed/mod.rs), with no behavior change to its existing callers in meal/mod.rs
- [ ] #2 nom-core/src/seed/mod.rs no longer contains a separate reimplementation of the per-100g macro formula; every planned portion's macros are computed by calling the shared compute_portion_macros via a NutrientValues snapshot
- [ ] #3 All existing seed unit tests (row counts, spot-checked macro/totals values such as Almonds 579.0/21.0/22.0/50.0/12.0, day-0 totals, re-run equality) still pass with their expected literals unmodified -- proving the refactor is behavior-preserving, not just compiling
- [ ] #4 nix develop -c cargo nextest run --all-features --workspace passes
- [ ] #5 nix develop -c cargo clippy --all-targets --all-features --workspace -- -D warnings is clean
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
SETUP (read first): nom_mcp is a Rust Cargo workspace (nom-core = domain logic, nom-mcp = thin CLI/server binaries). ALL commands must run inside the Nix dev shell: prefix every command with 'nix develop -c' (or run 'direnv allow' once). Work from the repository root unless told otherwise. Do not change pinned dependency versions.

1. In nom-core/src/meal/mod.rs, change 'fn compute_portion_macros(...)' (~line 129) to 'pub(crate) fn compute_portion_macros(...)'. Leave its signature, body, and all existing call sites within meal/mod.rs untouched.
2. In nom-core/src/seed/mod.rs, delete the portion_macros() helper (~lines 111-122). Import crate::food::NutrientValues (already pub(crate)) and reference meal::compute_portion_macros (crate::meal::compute_portion_macros, adjusting the module path if compute_portion_macros needs re-exporting -- check nom-core/src/meal/mod.rs's existing pub(crate) items for the established path convention first).
3. Update build_plan (~line 146) so each portion looks up its SEED_FOODS entry, builds a NutrientValues { calories, protein_g, carbs_g, fat_g, fiber_g } snapshot from it, and calls compute_portion_macros(quantity, "grams", None, snapshot) to get the portion's contributed NutrientValues -- summed into the meal's NutrientValues totals instead of the current manual tuple arithmetic. Change PlannedPortion.snapshot and PlannedMeal.totals from the raw (f64,f64,f64,f64,f64) tuples to NutrientValues.
4. Update the INSERT statements in execute_json (~lines 356-399) to read the named NutrientValues fields (.calories, .protein_g, .carbs_g, .fat_g, .fiber_g) instead of tuple indices (.0, .1, .2, .3, .4).
5. Run: nix develop -c cargo test -p nom-core seed:: -- and confirm every existing assertion (spot-checked macro values, day-0 totals, re-run-equality dump) still passes with NO changes to the expected literals -- this proves the refactor is behavior-preserving.
6. Run the full gate: nix develop -c cargo fmt --all --check; nix develop -c cargo clippy --all-targets --all-features --workspace -- -D warnings; nix develop -c cargo nextest run --all-features --workspace; nix develop -c cargo test --doc --all-features --workspace.
<!-- SECTION:PLAN:END -->

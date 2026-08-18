---
id: TASK-61
title: 'Fix: seed_data reimplements meal::compute_portion_macros instead of reusing it'
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-18 03:30'
updated_date: '2026-08-18 04:07'
labels:
  - review-followup
  - planned
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
- [x] #1 nom-core/src/meal/mod.rs's compute_portion_macros is pub(crate) (visible to nom-core/src/seed/mod.rs), with no behavior change to its existing callers in meal/mod.rs
- [x] #2 nom-core/src/seed/mod.rs no longer contains a separate reimplementation of the per-100g macro formula; every planned portion's macros are computed by calling the shared compute_portion_macros via a NutrientValues snapshot
- [x] #3 All existing seed unit tests (row counts, spot-checked macro/totals values such as Almonds 579.0/21.0/22.0/50.0/12.0, day-0 totals, re-run equality) still pass with their expected literals unmodified -- proving the refactor is behavior-preserving, not just compiling
- [x] #4 nix develop -c cargo nextest run --all-features --workspace passes
- [x] #5 nix develop -c cargo clippy --all-targets --all-features --workspace -- -D warnings is clean
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
SETUP (read first): nom_mcp is a Rust Cargo workspace (nom-core = domain logic, nom-mcp = thin CLI/server binaries). ALL commands must run inside the Nix dev shell: prefix every command with 'nix develop -c' (or run 'direnv allow' once). Work from the repository root unless told otherwise. Do not change pinned dependency versions.

GOAL: seed_data (nom-core/src/seed/mod.rs) must stop reimplementing the per-portion macro formula that meal::compute_portion_macros owns. Make the shared function callable from the seed module, route all planned-portion math through it, and replace the raw (f64 x5) tuple representation with the existing pub(crate) NutrientValues struct. Behavior-preserving: seeded DB rows and test expectations stay byte-identical.

VERIFIED CONTEXT (planning-time source check, 2026-08-18):
- nom-core/src/meal/mod.rs:129 — private fn compute_portion_macros(quantity: f64, quantity_mode: &str, snapshot_serving_size_g: Option<f64>, snapshot_nutrients: NutrientValues) -> NutrientValues; called at :397, :576, :1040 plus tests at :1498/:1515/:1532. No other pub(crate) items exist in this file.
- nom-core/src/lib.rs declares 'pub mod meal;' and 'pub mod food;', so sibling modules can reference crate::meal::compute_portion_macros directly once it is pub(crate) — no re-export needed.
- nom-core/src/food/mod.rs:58 — pub(crate) struct NutrientValues { calories, protein_g, carbs_g, fat_g, fiber_g } (fields pub(crate), derives Debug/Clone/Copy). Import convention: 'use crate::food::NutrientValues;' (same as meal/mod.rs:19).
- nom-core/src/seed/mod.rs:111–122 — portion_macros() duplicates the grams-mode formula over a raw 5-tuple; its doc comment admits the duplication.
- seed/mod.rs:124–138 — PlannedPortion.snapshot and PlannedMeal.totals are (f64,f64,f64,f64,f64) tuples.
- build_plan (~:146–185) does manual tuple accumulation; execute_json INSERTs read tuple indices (.0–.4) for meals (~:356–372) and portions (~:374–399).
- Tests: total_at() helper (:542–550) indexes totals by usize (used by test_build_plan_dates_and_totals_are_deterministic and test_seed_fixture_status_coverage); breakfast.totals.0..4 accessed directly in test_build_plan_dates_and_totals_are_deterministic. Hardcoded expected literals (e.g. today's totals 1846.0/172.9/113.8/77.4/37.0) must NOT change.

STEPS:
1. meal/mod.rs: change 'fn compute_portion_macros(' (~line 129) to 'pub(crate) fn compute_portion_macros('. Nothing else in meal/mod.rs changes.
2. seed/mod.rs: add 'use crate::food::NutrientValues;' and 'use crate::meal::compute_portion_macros;' (matching the file's existing 'use crate::...' style). Delete portion_macros() and its doc comment (~:111–122).
3. Change PlannedPortion.snapshot and PlannedMeal.totals to NutrientValues (adjust field doc comments). In build_plan: destructure SEED_FOODS as today, build let snapshot_100g = NutrientValues { calories: *kcal, protein_g: *p, carbs_g: *c, fat_g: *f, fiber_g: *fib }; then call let snapshot = compute_portion_macros(*grams, "grams", None, snapshot_100g); (grams mode only — serving size stays None), and accumulate each of the 5 named fields into the meal totals (initialized as all-zero NutrientValues) instead of the 5-line tuple arithmetic.
4. Update the two INSERT blocks in execute_json to read named fields: meal.totals.calories/.protein_g/.carbs_g/.fat_g/.fiber_g and portion.snapshot.calories/.protein_g/.carbs_g/.fat_g/.fiber_g instead of .0–.4.
5. Update test code touching the tuples (mechanical only — NO expected literal changes): delete the total_at() helper and replace its usages with named-field sums (e.g. todays.iter().map(|m| m.totals.calories).sum::<f64>() per nutrient); change breakfast.totals.0..4 to .calories/.protein_g/.carbs_g/.fat_g/.fiber_g.
6. Verify behavior preservation: nix develop -c cargo test -p nom-core seed:: -- (every assertion passes with unchanged literals) and nix develop -c cargo test -p nom-core meal:: (compute_portion_macros gained visibility; its own unit tests must pass unchanged).
7. Full gate: nix develop -c cargo fmt --all --check; nix develop -c cargo clippy --all-targets --all-features --workspace -- -D warnings; nix develop -c cargo nextest run --all-features --workspace; nix develop -c cargo test --doc --all-features --workspace.

NOTES/RISKS:
- One atomic unit — land steps 1–5 together (visibility + callsite migration belong in the same change; splitting leaves dead code or a half-migrated state).
- No new dependencies, no schema changes, no public API surface change (both touched items remain pub(crate)).
- Float math is bit-identical (same multiply/divide sequence as before), so hardcoded totals keep passing — that is the proof the refactor is behavior-preserving.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented per plan (one atomic change): (1) meal/mod.rs:129 compute_portion_macros is now pub(crate); no other meal/mod.rs change, its 23 unit tests pass unchanged. (2) seed/mod.rs: deleted portion_macros() + its self-admitting doc comment; PlannedPortion.snapshot and PlannedMeal.totals are now NutrientValues; build_plan builds a snapshot_100g NutrientValues from each SEED_FOODS row and calls compute_portion_macros(*grams, "grams", None, snapshot_100g), accumulating named fields into an all-zero NutrientValues totals. (3) Both INSERT blocks read named fields (.calories/.protein_g/.carbs_g/.fat_g/.fiber_g). (4) Tests: removed total_at() helper; test_build_plan_dates_and_totals_are_deterministic uses per-nutrient iterator sums and breakfast spot-check reads named fields; test_seed_fixture_status_coverage uses a today_sum(fn(&PlannedMeal)->f64) helper with .map(get) (clippy redundant_closure fix). No expected literal changed anywhere (verified via diff grep). Gates: fmt check clean, clippy -D warnings clean, nextest 329/329 (incl. seed e2e), doctests ok, rustdoc -D warnings clean. Note: pre-existing dirty file .pi/extensions/ralph/index.ts (orchestrator timeout tuning) was left unstaged.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
seed_data no longer reimplements meal::compute_portion_macros's grams-mode formula: the function is now pub(crate), seed/mod.rs routes every planned portion through it via NutrientValues snapshots (replacing the raw 5-tuple representation and its doc-admitted duplicate), and both INSERT blocks plus all tests read named fields. Behavior-preserving — zero expected literals changed, seed/meal unit suites + full gate (fmt, clippy -D warnings, nextest 329/329, doctests, rustdoc) green.
<!-- SECTION:FINAL_SUMMARY:END -->

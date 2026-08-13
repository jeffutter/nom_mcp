---
id: TASK-13
title: 'Fix: add regression test for update_meal empty-portions clear-all behavior'
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-12 20:21'
updated_date: '2026-08-13 01:07'
labels:
  - review-followup
dependencies:
  - TASK-2.14
priority: high
ordinal: 150
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while reviewing TASK-2.14 (nom-core/src/meal/mod.rs, UpdateMeal::execute_json, portions-replacement branch guarded by 'if !new_portions.is_empty()'). The ticket's own Implementation Plan explicitly calls out 'Empty portions array -> clears all portions (explicit decision)', and the code path for it exists, but no test exercises passing portions: [] to update_meal. This is a Correctness axis gap: the exact boundary condition most likely to regress silently (e.g. a future refactor of the delete-then-insert logic that accidentally skips the delete when the array is empty) has zero test coverage protecting it.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A new integration test in nom-core/src/meal/mod.rs calls update_meal with portions: [] on a meal that has existing portions
- [x] #2 The test asserts the meal's portions are actually deleted (e.g. a follow-up query or the returned summary shows zero portions) and that materialized totals are recomputed to reflect the empty portion set (adjustment-only totals, or zero if no adjustment)
- [x] #3 nix develop -c cargo test -p nom-core passes
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
SETUP (read first): This is a Rust+WebAssembly core (crates/gql-core) with a TypeScript/React web app (web/). Wait -- this repo is nom-core/nom-mcp, not gql-core; ALL commands must still run inside the Nix dev shell: either run 'direnv allow' once, or prefix every command with 'nix develop -c'. Work from the repository root unless told otherwise. Do not change pinned dependency versions.

1. Open nom-core/src/meal/mod.rs and locate the existing test module near the bottom of the file, and the existing test test_update_meal_full_portion_replacement (search by name; line numbers have shifted after a post-review rustfmt pass) -- use it as your template for DB setup (TempDb, seeded food row(s), LogMeal to create an initial meal with at least one portion).
2. Add a new test named test_update_meal_empty_portions_clears_all. In it: seed a food, log_meal to create a meal with 1-2 portions, then call UpdateMeal::execute_json with a request JSON where portions is an explicit empty array [] (not omitted -- omitted means untouched, per UpdateMeal's partial-patch semantics; this test must send the field present-but-empty).
3. After the update call, verify zero portions remain for that meal_id -- either by querying the portions table directly (SELECT COUNT(*) FROM portions WHERE meal_id = ?), or by calling a get-by-id/get_meals_by_date_range path and checking the returned portions array is empty.
4. Also assert the meal's materialized totals reflect zero portions after the clear (i.e. total_calories etc. equal whatever the adjustment alone contributes, or 0.0 if no adjustment was set) -- this is the part that would silently break if totals recomputation were skipped when portions is empty.
5. Run: nix develop -c cargo test -p nom-core -- meal::tests::test_update_meal_empty_portions_clears_all --nocapture and confirm it passes, then run the full suite: nix develop -c cargo test -p nom-core
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Fixup applied post-review: cargo fmt --all wrapped the long assert_eq! line at nom-core/src/meal/mod.rs (test_update_meal_empty_portions_clears_all) that failed 'cargo fmt --all -- --check'; no behavior change.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added regression test test_update_meal_empty_portions_clears_all in nom-core/src/meal/mod.rs. The test logs a meal with 2 portions (750 cal total), then calls update_meal with portions: [] and verifies: (1) returned portions array is empty, (2) materialized totals are all zeros, (3) DB-level portion count for that meal_id is 0. All 164 nom-core tests pass.
<!-- SECTION:FINAL_SUMMARY:END -->

---
id: TASK-32
title: >-
  Fix: deduplicate per-nutrient boilerplate in goal::mod.rs
  (SetNutritionGoals/GetGoalProgress)
status: To Do
assignee: []
created_date: '2026-08-13 07:09'
labels:
  - review-followup
dependencies:
  - TASK-2.16
priority: high
ordinal: 100
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while reviewing TASK-2.16 (nom-core/src/goal/mod.rs). SetNutritionGoals::execute_json repeats the same three-step pattern (validate direction string, resolve-and-carry-forward direction, merge value) five times, once per nutrient (calories/protein_g/carbs_g/fat_g/fiber_g) — the validation loop (~lines 407-424), the five near-identical validate_and_resolve_direction calls (~lines 460-489), and the five merge(...) calls (~lines 492-499) are all copy-pasted with only the field names changed. GetGoalProgress::execute_json repeats the same pattern for extracting goal_<nutrient>/goal_<nutrient>_dir from the ActiveGoal struct and calling nutrient_progress (~lines 1000-1030). This is the 'repetition -> missing abstraction' red flag from CLAUDE.md's design philosophy: adding a 6th nutrient in the future requires touching 4+ separate copy-pasted blocks instead of one array entry, and the duplication makes it easy for the blocks to silently drift out of sync (a missed direction validation or a mismatched field extraction) with no compiler check. Consolidate around a small NUTRIENTS table/loop (name, get/set accessors on the request and ActiveGoal, or index-based field access) so each nutrient is defined once. Violates the Concise axis.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 SetNutritionGoals::execute_json no longer has 5 separate copy-pasted validate_and_resolve_direction call sites with only the field name changed — direction validation and carry-forward resolution is driven by iterating a single NUTRIENTS list/array
- [ ] #2 GetGoalProgress::execute_json no longer has 5 separate copy-pasted goal_<nutrient>/goal_<nutrient>_dir extraction + nutrient_progress call blocks — driven by the same NUTRIENTS list/array
- [ ] #3 All 15 existing tests in nom-core/src/goal/mod.rs still pass unmodified (behavior must not change, only structure)
- [ ] #4 nix develop -c cargo test -p nom-core passes
- [ ] #5 nix develop -c cargo clippy -p nom-core --all-targets -- -D warnings is clean
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
SETUP (read first): This is a Rust+WebAssembly core (crates/gql-core) with a TypeScript/React web app (web/). ALL commands must run inside the Nix dev shell: either run 'direnv allow' once, or prefix every command with 'nix develop -c'. Work from the repository root unless told otherwise. Do not change pinned dependency versions.

1. Open nom-core/src/goal/mod.rs. Read the full file first — SetNutritionGoals::execute_json and GetGoalProgress::execute_json, plus the ActiveGoal struct, nutrient_progress() helper, and the request/response structs.

2. Design a single source of truth for the 5 nutrients (calories, protein_g, carbs_g, fat_g, fiber_g). A plain const array of field names is not enough here because the code needs typed access into SetNutritionGoalsRequest, ActiveGoal, and the response JSON — prefer a small helper that takes the request/prior/req-dir-fields as explicit tuples and iterates, e.g.:
   let nutrients: [(&str, Option<f64>, Option<&String>, Option<f64>, Option<&String>); 5] = [
       ("calories", req.calories, req.calories_direction.as_ref(), prior.as_ref().and_then(|g| g.calories), prior.as_ref().and_then(|g| g.calories_direction.as_ref())),
       ... one entry per nutrient ...
   ];
   then loop once, calling validate_and_resolve_direction and merge inside the loop, collecting results into a Vec or a small struct-of-arrays that the final json!() response construction reads from by name.
   Adjust the exact shape as needed — the goal is one loop body executed 5 times, not 5 duplicated call sites, while keeping the final JSON response with the same explicit field names it has today (goal_id, effective_from, calories, calories_direction, ...).

3. Apply the same consolidation to GetGoalProgress::execute_json: build a NUTRIENTS-like loop that reads consumed/target/direction from the (cal, prot, carbs, fat, fiber) tuple and the ActiveGoal, calls nutrient_progress(), and assembles the GoalProgress struct's 5 NutrientProgress fields without 5 separate copy-pasted extraction blocks. Since GoalProgress's fields (calories, protein_g, ...) are named struct fields (not a map), the loop's output still needs to land in those 5 named fields at the end — either loop into a temporary array/Vec keyed by index and destructure into the named fields, or keep GoalProgress construction explicit but drive the *computation* of each NutrientProgress through the shared loop rather than 5 hand-written call sites.

4. Do not change ActiveGoal, NutrientProgress, WeightProgress, GoalProgress, Direction, or ProgressStatus's public shape, and do not change the JSON wire format (field names, null behavior) — this is a structural refactor only, not a behavior change.

5. Run: nix develop -c cargo test -p nom-core -- goal:: to confirm all 15 existing goal tests still pass with zero test-code changes.

6. Run: nix develop -c cargo test -p nom-core (full suite) and nix develop -c cargo clippy -p nom-core --all-targets -- -D warnings and nix develop -c cargo fmt --all --check. All three must be clean.
<!-- SECTION:PLAN:END -->

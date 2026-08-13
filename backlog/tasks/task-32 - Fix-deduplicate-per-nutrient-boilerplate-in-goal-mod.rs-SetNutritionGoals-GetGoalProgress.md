---
id: TASK-32
title: >-
  Fix: deduplicate per-nutrient boilerplate in goal::mod.rs
  (SetNutritionGoals/GetGoalProgress)
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-13 07:09'
updated_date: '2026-08-13 11:58'
labels:
  - review-followup
  - planned
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
- [x] #1 SetNutritionGoals::execute_json no longer has 5 separate copy-pasted validate_and_resolve_direction call sites with only the field name changed — direction validation and carry-forward resolution is driven by iterating a single NUTRIENTS list/array
- [x] #2 GetGoalProgress::execute_json no longer has 5 separate copy-pasted goal_<nutrient>/goal_<nutrient>_dir extraction + nutrient_progress call blocks — driven by the same NUTRIENTS list/array
- [x] #3 All 15 existing tests in nom-core/src/goal/mod.rs still pass unmodified (behavior must not change, only structure)
- [x] #4 nix develop -c cargo test -p nom-core passes
- [x] #5 nix develop -c cargo clippy -p nom-core --all-targets -- -D warnings is clean
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
SETUP: Rust workspace; run all commands via `nix develop -c <cmd>` from repo root. Single-file change: nom-core/src/goal/mod.rs. No sub-tickets — this is a tightly-coupled refactor across two functions in one file that must ship together (does not meet the bar for independent sub-tickets).

Verified current structure (read in full): SetNutritionGoalsRequest has 5 nutrient (value, direction) field pairs (calories/protein_g/carbs_g/fat_g/fiber_g) + target_weight (no direction). ActiveGoal mirrors the same 5 pairs + target_weight. `validate_and_resolve_direction` closure (line ~424) and `merge` closure (line ~460) already exist and are reused per-nutrient — only the *call sites* are duplicated, so those closures stay as-is. `nutrient_progress()` helper already exists and is reused per-nutrient in GetGoalProgress — only the *extraction+call* is duplicated.

Confirmed via test review (all 15 tests in `mod tests`): no test depends on the exact relative order of the two current validation passes (the up-front `valid_directions` format-check loop vs. the later required-direction check inside `validate_and_resolve_direction`) when multiple fields are simultaneously invalid — every direction-error test triggers exactly one invalid field. This means the format-check loop and the resolve/merge loop MAY be folded into a single per-nutrient loop without changing any observable behavior other error ordering (which nothing asserts on).

1. SetNutritionGoals::execute_json — replace the three duplicated blocks (format-check loop ~L392-406, five validate_and_resolve_direction calls ~L456-485, five merge calls ~L487-494) with one loop:

   ```rust
   const VALID_DIRECTIONS: [&str; 3] = ["target", "minimum", "maximum"];

   let nutrients: [(&str, Option<f64>, Option<&String>, Option<f64>, Option<&String>); 5] = [
       ("calories", req.calories, req.calories_direction.as_ref(),
        prior.as_ref().and_then(|g| g.calories), prior.as_ref().and_then(|g| g.calories_direction.as_ref())),
       ("protein_g", req.protein_g, req.protein_g_direction.as_ref(),
        prior.as_ref().and_then(|g| g.protein_g), prior.as_ref().and_then(|g| g.protein_g_direction.as_ref())),
       ("carbs_g", req.carbs_g, req.carbs_g_direction.as_ref(),
        prior.as_ref().and_then(|g| g.carbs_g), prior.as_ref().and_then(|g| g.carbs_g_direction.as_ref())),
       ("fat_g", req.fat_g, req.fat_g_direction.as_ref(),
        prior.as_ref().and_then(|g| g.fat_g), prior.as_ref().and_then(|g| g.fat_g_direction.as_ref())),
       ("fiber_g", req.fiber_g, req.fiber_g_direction.as_ref(),
        prior.as_ref().and_then(|g| g.fiber_g), prior.as_ref().and_then(|g| g.fiber_g_direction.as_ref())),
   ];

   let mut merged: [Option<f64>; 5] = [None; 5];
   let mut dirs: [Option<String>; 5] = [None, None, None, None, None];
   for (i, (name, value, provided_dir, prior_value, prior_dir)) in nutrients.into_iter().enumerate() {
       if let Some(d) = provided_dir {
           if !VALID_DIRECTIONS.contains(&d.as_str()) {
               return Err(ErrorData::validation(
                   format!("{name}_direction"),
                   format!("must be one of 'target', 'minimum', 'maximum', got '{d}'"),
               ));
           }
       }
       dirs[i] = validate_and_resolve_direction(name, value.is_some(), provided_dir, prior_dir)?;
       merged[i] = merge(value, prior_value);
   }
   let [merged_calories, merged_protein_g, merged_carbs_g, merged_fat_g, merged_fiber_g] = merged;
   let [cal_dir, prot_dir, carbs_dir, fat_dir, fiber_dir] = dirs;
   ```
   `merged_target_weight` keeps its existing standalone `merge(...)` call (no direction, not part of the loop). The `validate_and_resolve_direction` and `merge` closures are unchanged; only their call sites collapse into the loop. The final `json!({...})` response construction and the INSERT `params` tuple stay exactly as they are today (still reference the same named `merged_*`/`*_dir` locals produced by the destructure), so the wire format is untouched.

2. GetGoalProgress::execute_json — replace the five duplicated extraction+nutrient_progress blocks (~L681-704) with:

   ```rust
   let fields: [(Option<f64>, Option<&String>, f64); 5] = [
       (goal.as_ref().and_then(|g| g.calories), goal.as_ref().and_then(|g| g.calories_direction.as_ref()), cal),
       (goal.as_ref().and_then(|g| g.protein_g), goal.as_ref().and_then(|g| g.protein_g_direction.as_ref()), prot),
       (goal.as_ref().and_then(|g| g.carbs_g), goal.as_ref().and_then(|g| g.carbs_g_direction.as_ref()), carbs),
       (goal.as_ref().and_then(|g| g.fat_g), goal.as_ref().and_then(|g| g.fat_g_direction.as_ref()), fat),
       (goal.as_ref().and_then(|g| g.fiber_g), goal.as_ref().and_then(|g| g.fiber_g_direction.as_ref()), fiber),
   ];
   let mut progress: [Option<NutrientProgress>; 5] = Default::default();
   for (i, (target, dir_str, consumed)) in fields.into_iter().enumerate() {
       progress[i] = Some(nutrient_progress(consumed, target, parse_direction(dir_str)));
   }
   let [calories_progress, protein_g_progress, carbs_g_progress, fat_g_progress, fiber_g_progress] =
       progress.map(Option::unwrap);
   ```
   (`NutrientProgress` needs `Default` derived, or use `[const { None }; 5]` / a `Vec` + `.try_into().unwrap()` instead if adding `Default` to a public struct feels like scope creep — pick whichever keeps `NutrientProgress`'s public shape otherwise unchanged.) `parse_direction` closure and `nutrient_progress()` function stay as-is. The final `GoalProgress { ... }` construction and `weight_progress` call are untouched, preserving the JSON field names/nulls exactly.

3. Do not touch: ActiveGoal, NutrientProgress, WeightProgress, GoalProgress, Direction, ProgressStatus public shapes (beyond an internal Default derive if used for tmp arrays), fetch_active_goal, fetch_consumed_totals, fetch_latest_weight, nutrient_progress(), weight_progress(), or any SQL/wire format. This is structural only.

4. Run `nix develop -c cargo test -p nom-core -- goal::` — all existing goal tests must pass with zero test-code edits.

5. Run `nix develop -c cargo test -p nom-core` (full suite), `nix develop -c cargo clippy -p nom-core --all-targets -- -D warnings`, and `nix develop -c cargo fmt --all --check`. All three must be clean before calling this done.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Refactored nom-core/src/goal/mod.rs to eliminate the per-nutrient copy-paste in SetNutritionGoals::execute_json and GetGoalProgress::execute_json.

SetNutritionGoals::execute_json: removed the standalone up-front direction-format-check loop and folded format validation into the existing validate_and_resolve_direction closure (now checks VALID_DIRECTIONS itself before its existing carry-forward/required logic). Replaced the five near-identical validate_and_resolve_direction call sites and five merge(...) call sites with a single 'nutrients' array of (name, value, provided_dir, prior_value, prior_dir) tuples, iterated once to populate merged_values/resolved_dirs arrays, then destructured back into the existing named locals (merged_calories, cal_dir, etc.) so the INSERT params tuple and JSON response construction are unchanged byte-for-byte in shape/order. target_weight (no direction) keeps its standalone merge(...) call as planned.

GetGoalProgress::execute_json: replaced the five duplicated goal_<nutrient>/goal_<nutrient>_dir extraction + nutrient_progress(...) call blocks with a single 'nutrients' array of (target, dir_str, consumed) tuples, mapped through nutrient_progress+parse_direction via an iterator, then unpacked via progress.next().unwrap() x5 into the existing named locals. Chose the iterator/.next() approach over an array+Default/try_into to avoid touching NutrientProgress's public derive list, per the ticket's own guidance to prefer whichever keeps its public shape unchanged.

No test code was modified. Verified: nix develop -c cargo test -p nom-core -- goal:: (15/15 pass), nix develop -c cargo test -p nom-core (full suite, 221 + 1 integration test pass), nix develop -c cargo clippy -p nom-core --all-targets -- -D warnings (clean), nix develop -c cargo fmt --all --check (clean).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Deduplicated the per-nutrient boilerplate in nom-core/src/goal/mod.rs by driving both SetNutritionGoals::execute_json's direction validation/carry-forward/merge and GetGoalProgress::execute_json's target/direction extraction + nutrient_progress calls from single per-function nutrient arrays instead of five copy-pasted call sites each. Behavior is unchanged: all 15 existing goal tests pass unmodified, full nom-core suite (221 tests) passes, clippy -D warnings is clean, and cargo fmt is clean.
<!-- SECTION:FINAL_SUMMARY:END -->

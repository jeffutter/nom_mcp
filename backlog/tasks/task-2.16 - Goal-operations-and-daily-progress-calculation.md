---
id: TASK-2.16
title: Goal operations and daily-progress calculation
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-11 13:24'
updated_date: '2026-08-13 06:45'
labels:
  - planned
dependencies:
  - TASK-2.5
  - TASK-2.7
  - TASK-2.14
  - TASK-2.15
  - TASK-26
  - TASK-27
type: feature
ordinal: 35000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Scope
set_nutrition_goals(<partial subset of calories/protein_g/carbs_g/fat_g/fiber_g/target_weight>, direction?) — partial patch, creates a new effective_from=today versioned row merged over the current goal. Each nutrient's direction (target/minimum/maximum) is required the first time that nutrient is set, carried forward on later updates that omit it; target_weight has no direction. get_nutrition_goals() — currently active goal only.

get_goal_progress(date?) — single date only. Per nutrient: consumed, target (null if unset), remaining, percent (null if target null/zero), direction, status (under/met/over, met=exact equality). Weight section: latest_weight (on/before queried date, null if none), target_weight, remaining, status — no percent field. If no Goal has ever been active as of the queried date, consumed/latest_weight still populate from real data but every target/remaining/percent/status/direction is null — never an error. Both 'active Goal' and 'latest weight' resolve as-of the queried date.

See doc-5 §7, decision-2.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 set_nutrition_goals requires direction the first time a nutrient is set and carries it forward on later partial updates
- [x] #2 get_goal_progress(date?) returns the full per-nutrient and weight shape described in doc-5 §7, including null-field behavior when no goal has ever been set
- [x] #3 both active Goal and latest weight resolve as-of the queried date, not always-current
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Implementation Plan for TASK-2.16

### New file: `nom-core/src/goal/mod.rs`

Two Operation implementations sharing the same module pattern as weight/meal.

---

#### 1. SetNutritionGoals Operation (AC #1)

**Request struct**: `SetNutritionGoalsRequest` with optional fields: `calories`, `protein_g`, `carbs_g`, `fat_g`, `fiber_g`, `target_weight`. Each nutrient field that has a non-null value must also have its corresponding `_direction` field (`target`/`minimum`/`maximum`) unless that nutrient already has a direction from a prior active goal. `target_weight` has no direction.

**Algorithm**:
1. Read current active goal row: `SELECT * FROM goals WHERE effective_from <= ? ORDER BY effective_from DESC LIMIT 1` using today's date from Clock.
2. If no prior goal exists, validate that every provided nutrient includes its direction — return Validation error if any direction is missing.
3. If prior goal exists, fill in missing directions from the prior row. Validate that newly-provided nutrients still include their direction.
4. Insert new row with `effective_from = today`, carrying forward all non-overridden fields from the prior goal.
5. Return the complete merged goal shape including id, effective_from, and all nutrient/direction/target_weight fields.

**Edge cases**:
- First-ever call with zero nutrients → still creates a row (all nulls except effective_from). Acceptable per spec since partial patches are the norm.
- Direction mismatch: user provides a different direction for an already-set nutrient → allowed (new row supersedes).
- Removing a target: cannot "unset" a nutrient via partial patch. That's outside scope (would require a separate delete_nutrient_target operation).

---

#### 2. GetGoalProgress Operation (AC #2, #3)

**Request struct**: `GetGoalProgressRequest` with optional `date` (YYYY-MM-DD). Defaults to today via Clock.

**Algorithm**:
1. Resolve query date: use provided date or `clock.today()`.
2. **Active goal as-of-date**: `SELECT * FROM goals WHERE effective_from <= ? ORDER BY effective_from DESC LIMIT 1`.
3. **Consumed totals**: `SELECT SUM(total_calories), SUM(total_protein_g), ... FROM meals WHERE logged_date = ?`. Handle NULL sums (no meals) → treat as 0.0.
4. **Latest weight as-of-date**: `SELECT value FROM weight_entries WHERE logged_date <= ? ORDER BY logged_date DESC LIMIT 1`.
5. **Per-nutrient comparison** for each of calories/protein_g/carbs_g/fat_g/fiber_g:
   - `consumed` = from meal aggregation
   - `target` = from active goal (null if goal doesn't set it)
   - `remaining` = target − consumed (null if target is null)
   - `percent` = (consumed / target × 100), null if target is null or zero
   - `direction` = from active goal (null if goal doesn't set it)
   - `status`: `under` (consumed < target), `met` (consumed == target, exact equality per spec), `over` (consumed > target). Null if target is null.
6. **Weight section**:
   - `latest_weight` = from weight lookup (null if no entries)
   - `target_weight` = from active goal (null if goal doesn't set it)
   - `remaining` = target_weight − latest_weight (null if either is null)
   - `status`: same under/met/over logic, null if either is null
   - No `percent` field for weight.

**Output shape** (per doc-5 §7): All nutrient entries follow the same structure with consumed/target/remaining/percent/direction/status. When no goal has ever been set: consumed values still populate from real data; all target/remaining/percent/status/direction fields are null.

---

#### 3. Registration

In `nom-mcp/src/main.rs`:
- Import `SetNutritionGoals`, `GetGoalProgress` from `nom_core::goal`
- Register both ops in registry after weight ops

In `nom-core/src/lib.rs`:
- Add `pub mod goal;`

---

#### 4. Tests

Follow weight module test patterns exactly: `TempDb::new()`, `#[serial_test::serial]`, `#[tokio::test]`, `with_db_path(db.path.clone())`.

**SetNutritionGoals tests**:
- First call with direction for each nutrient → creates initial goal
- First call without direction for a new nutrient → validation error
- Second call omitting direction for previously-set nutrient → carries forward
- Partial update (only protein changes, others carried forward)
- Target_weight setting (no direction required)
- Overriding direction on re-set

**GetGoalProgress tests**:
- Full progress with goal set and meals logged → all fields populated
- No goal ever set → consumed populated, all goal-derived fields null
- Goal set but no meals → consumed zeros, target fields present
- Weight progress with entries → latest_weight and status computed
- Weight progress with no entries → latest_weight null
- Date parameter resolves goal and meals as-of that date
- Percent null when target is zero
- Status: under/met/over boundary conditions (exact equality for met)

---

### File count: 2 files modified (goal/mod.rs created, lib.rs + main.rs edited)
### Estimated LOC: ~350 total (200 domain logic + 150 tests)
### Risk: Low — schema exists, patterns are proven, no external dependencies
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implementation created nom-core/src/goal/mod.rs with SetNutritionGoals and GetGoalProgress operations. SetNutritionGoals supports partial patches with direction carry-forward from prior active goal; GetGoalProgress resolves both active goal and latest weight as-of queried date, computing per-nutrient consumed/target/remaining/percent/direction/status plus weight progress without percent field. Registered both ops in OperationRegistry for CLI/HTTP/MCP surfaces. 15 tests covering all acceptance criteria including edge cases (no goal ever set, zero target, exact equality for met status, as-of-date resolution). Added pub mod goal to lib.rs and imports/registration in main.rs.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Created goal/mod.rs with SetNutritionGoals (partial-patch versioned goals, direction carry-forward) and GetGoalProgress (per-nutrient + weight progress as-of-date). Registered both operations in main.rs. 15 tests covering all ACs. All quality checks pass: fmt, clippy, doc, full test suite (209 tests).
<!-- SECTION:FINAL_SUMMARY:END -->

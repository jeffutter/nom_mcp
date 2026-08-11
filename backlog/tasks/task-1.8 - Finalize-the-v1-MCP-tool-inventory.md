---
id: TASK-1.8
title: Finalize the v1 MCP tool inventory
status: Done
assignee:
  - Jeffery Utter
created_date: '2026-08-11 04:40'
updated_date: '2026-08-11 12:14'
labels:
  - 'wayfinder:grilling'
dependencies:
  - TASK-1.5
  - TASK-1.6
parent_task_id: TASK-1
ordinal: 9000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Question

Produce the exact v1 tool list (names, inputs, outputs, description text) derived from the confirmed scope: meal logging/editing/deleting/searching/date-range queries, weight logging/editing/deleting/date-range queries, goal setting/getting/progress, barcode and USDA food lookup, custom food creation. Cross-reference the inspiration project's tool table (akutishevsky/nutrition-mcp README) as a starting shape, adjusted for the Food/Meal/Portion domain model and the confirmed exclusions (no alcohol/water/timezone/units/CSV-import/trends/account-deletion). Each tool needs enough detail (params, return shape) that a build effort can implement it without re-deriving intent.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Alternatives weighed: food lookup as separate lookup_barcode/search_usda_foods/search_custom_foods tools (rejected — user picked unified search_food once "ranked" was clarified as routing-by-input-shape + grouping-by-source, not a real cross-source relevance merge). Meal logging as one-step resolve-or-create-inline (rejected — two-step with food_id required keeps log_meal a pure write and matches Portion's snapshot-at-log-time design from TASK-1.5). Date-scoped queries as one consolidated get_meals/get_weight_entries tool with optional params (rejected — separate per-scope tools mirror the inspiration project and give a calling model a trivial fixed schema per call). update_meal portion edits as granular add_portion/remove_portion/update_portion tools or incremental add/remove arrays (rejected both — whole-list replace keeps the tool count down). search_meals as recurring-variation grouping like the inspiration project (rejected — reads as the pattern-analytics behavior the map already deferred to a post-v1 effort). get_goal_progress split from weight-vs-target (rejected — CONTEXT.md explicitly folds target weight into Goal). create_custom_food on a per-100g basis or caller's-choice basis flag (rejected — per-serving matches how a user actually knows a homemade dish's macros). Goal effective_from exposed to the caller (rejected — kept internal-only, set_nutrition_goals always takes effect today). log_meal/log_weight always "now" with backdating via update (rejected — both take an optional logged_at).

Minor implementation defaults settled without a separate grilling round (small enough to be forced by earlier answers, not fresh forks): delete_meal/delete_weight_entry error on a not-found id rather than silent no-op; search_meals matches only against linked Food names since Meal has no free-text notes field in the domain model; a custom food's portions can only use quantity_mode="servings" unless its serving_size.unit is grams (no gram equivalence otherwise known).

UPDATE from TASK-1.10: set_nutrition_goals' per-nutrient params gain a required Direction (target/minimum/maximum) the first time each nutrient is set (no default), carried forward on later updates that omit it. get_goal_progress's response shape (per-nutrient consumed/target/remaining/percent/direction/status, plus a weight section) is fully specified there — see decision-2. Tool count/names/other signatures here are unchanged.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Full v1 tool inventory: 18 tools.

**Food (2):** `search_food(query)` — barcode routes to OpenFoodFacts only, free text routes to USDA FDC + local custom foods (custom-first, USDA in USDA's order); every match is upserted into the local `foods` cache as part of the call so `food_id` is immediately usable — searching IS resolving, no separate import step. `create_custom_food(name, serving_size:{quantity,unit}, nutrients)` — nutrients given per one serving (not per-100g), matching how a user actually knows a homemade dish.

**Meal (7):** `log_meal(portions:[{food_id,quantity,quantity_mode}], adjustment?, logged_at?)` — two-step flow, food_id must already exist. `update_meal(meal_id, portions?, adjustment?, logged_at?)` — partial patch; `portions` when present replaces the whole array (no granular add/remove-portion tools). `delete_meal(meal_id)` — errors on not-found rather than silent no-op. `search_meals(query, date_range?)` — plain keyword search over linked Food names, recency-ordered; deliberately not the inspiration project's recurring-variation grouping, which reads as the deferred pattern-analytics behavior. `get_meals_today/by_date/by_date_range` — mirror the inspiration project's per-scope tool split.

**Weight Entry (6):** `log_weight(value, logged_at?)`, `update_weight_entry(id, value?, logged_at?)`, `delete_weight_entry(id)`, `get_weight_today/by_date/by_date_range` — same per-scope query split and optional-logged_at backdating as Meal.

**Goal (3):** `set_nutrition_goals(<partial subset of calories/protein_g/carbs_g/fat_g/fiber_g/target_weight>)` — partial patch, creates a new `effective_from=today` versioned row merged over the current goal; versioning itself stays internal (no caller-facing effective_from param). `get_nutrition_goals()` — currently active goal only. `get_goal_progress(date?)` — nutrient intake vs. targets AND latest-weight-vs-target-weight in one response, since CONTEXT.md treats target weight as part of Goal, not a separate concept; this also absorbs the inspiration project's separate get_nutrition_summary — no redundant tool needed.

Excluded from this ticket (belong to TASK-1.11): MCP-only widget-toggle tools, the weekly-summary Resource.
<!-- SECTION:FINAL_SUMMARY:END -->

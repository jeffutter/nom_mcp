---
id: TASK-1.10
title: Design Goals and daily-progress calculation
status: Done
assignee:
  - Jeffery Utter
created_date: '2026-08-11 04:40'
updated_date: '2026-08-11 12:14'
labels:
  - 'wayfinder:grilling'
dependencies:
  - TASK-1.5
  - TASK-1.7
parent_task_id: TASK-1
ordinal: 11000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Question

Design how nutrition Goals (daily calorie/macro/fiber targets, optional target weight) are set, stored, and evaluated against logged Meals and Weight Entries to compute progress for a given day or date range. Cover: what 'progress' returns (intake vs target/limit per nutrient), how a target weight relates to Weight Entry trend, and how this ties into the date semantics decided in the 'today' ticket.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Extends TASK-1.5's goals schema (a direction column per nutrient) and TASK-1.8's set_nutrition_goals signature (direction required on first-time nutrient set) — see decision-2 for full rationale and rejected alternatives (uniform target framing; fixed per-nutrient semantics). CONTEXT.md's Goal entry updated with a new Direction term.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Goal nutrient targets (calories, protein, carbs, fat, fiber) carry an explicit Direction (target/minimum/maximum), required the first time a nutrient is set via set_nutrition_goals and carried forward on later partial updates that omit it — see decision-2. target_weight has no Direction; its progress is read directly off comparing to the latest Weight Entry.

get_goal_progress(date?) — single date only, no date-range aggregation (already locked by TASK-1.8; that tool signature stands) — returns per nutrient: consumed, target (null if unset for that date's active Goal), remaining = target − consumed (null if no target), percent = consumed/target×100 (null if target null or zero), direction (echoed), and status ∈ {under, met, over} (met = exact equality, no tolerance band). Weight section: latest_weight (the Weight Entry on/before the queried date, null if none exist yet), target_weight (from the Goal active that date, null if unset), remaining, and the same status scheme — no percent field for weight, since a weight ratio isn't a meaningful progress number without a tracked baseline.

If no Goal has ever been active as of the queried date, consumed/latest_weight still populate from real logged data, but every target/remaining/percent/status/direction is null — no error.

Historical "active Goal" and "latest weight" both resolve as-of the queried date (not always-current), consistent with TASK-1.5's effective_from versioning and TASK-1.7's Clock-driven date semantics.
<!-- SECTION:FINAL_SUMMARY:END -->

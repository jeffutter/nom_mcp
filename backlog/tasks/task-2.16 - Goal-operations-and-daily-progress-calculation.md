---
id: TASK-2.16
title: Goal operations and daily-progress calculation
status: To Do
assignee: []
created_date: '2026-08-11 13:24'
labels: []
dependencies:
  - TASK-2.5
  - TASK-2.7
  - TASK-2.14
  - TASK-2.15
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
- [ ] #1 set_nutrition_goals requires direction the first time a nutrient is set and carries it forward on later partial updates
- [ ] #2 get_goal_progress(date?) returns the full per-nutrient and weight shape described in doc-5 §7, including null-field behavior when no goal has ever been set
- [ ] #3 both active Goal and latest weight resolve as-of the queried date, not always-current
<!-- AC:END -->

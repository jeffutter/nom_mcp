---
id: TASK-62.5
title: 'Optional: show meal type in the food-added and weekly-progress widgets'
status: Backlog
assignee: []
created_date: '2026-09-07 01:30'
labels: []
dependencies: []
parent_task_id: TASK-62
priority: low
type: enhancement
ordinal: 74000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Optional UI polish for TASK-62 — deliberately NOT a dependency of TASK-62, so the parent can close without it. Show the stored meal type in the MCP widgets once the data is flowing (TASK-62.2 for log_meal's result, TASK-62.3 for the weekly breakdown).

Candidates, smallest first:
1. `nom-core/assets/food_added_widget.html` — a meal-type label in the meal header above the portion list (`portionsHtml` at ~L284-299; the log_meal tool result is parsed by `extractLogMeal` at ~L336, so `meal_type` is already available in the payload). Must render gracefully when the field is null.
2. `nom-core/assets/weekly_progress_widget.html` — a per-meal-type line inside each day row, fed by `nutrients.daily_totals[].by_meal_type` (~L261 where daily_totals is indexed by date). Skip days whose breakdown is empty; handle the legacy null bucket.

Constraints: widgets are static HTML shells served verbatim from mcp_handler.rs resource branches, so no server change is needed. Visual verification must respect the repo's image limits — at most 4 screenshots per prompt (e.g. light+dark of one variant per pass), split larger comparisons across separate runs, and prefer analyzing them in subagents.

Needs its own /backlog-planner pass if picked up: layout, dark-mode colors, and whether the compact variants have room are undecided.
<!-- SECTION:DESCRIPTION:END -->

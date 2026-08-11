---
id: TASK-2.17
title: MCP Resource (weekly-summary) and Widget Display tools
status: To Do
assignee: []
created_date: '2026-08-11 13:24'
labels: []
dependencies:
  - TASK-2.7
  - TASK-2.14
  - TASK-2.15
  - TASK-2.16
parent_task_id: TASK-2
type: feature
ordinal: 36000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Scope
Weekly Summary Resource — MCP-only, fixed URI nom://weekly-summary, no params, live-computed on every read (no caching). Stays outside the Operation trait entirely — hand-written list_resources/read_resource on ServerHandler, data-fetching in a capability-layer function. Rolling last-7-days window (not calendar week). Nutrients section shaped like get_goal_progress (per-nutrient daily-average consumed/target/remaining/percent/direction/status) plus a per-day array of raw daily totals. Weight section: start/end/delta from Weight Entries in the window (null start/delta if none logged, but latest_known_weight still comes from the most recent entry before the window), plus target-weight comparison.

Widget Display: get_widget_display()/set_widget_display(enabled: bool), ordinary Operations with surfaces()=MCP only, backed by the settings table (widget_display_enabled BOOLEAN). v1 is plumbing only — no tool or Resource output branches on it yet.

See doc-5 §8, decision-3.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 nom://weekly-summary resource returns nutrients (daily-average vs target, per-day breakdown) and weight (start/end/delta, latest_known_weight, target comparison) for a rolling 7-day window
- [ ] #2 get_widget_display/set_widget_display are Operations with surfaces()=MCP only, persisted in the settings table
- [ ] #3 no other Operation or Resource output currently branches on widget_display_enabled
<!-- AC:END -->

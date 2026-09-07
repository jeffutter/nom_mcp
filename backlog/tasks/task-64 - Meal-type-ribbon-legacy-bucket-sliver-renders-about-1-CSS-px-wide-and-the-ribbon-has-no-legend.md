---
id: TASK-64
title: >-
  Meal-type ribbon: legacy-bucket sliver renders about 1 CSS px wide and the
  ribbon has no legend
status: Dev Ready
assignee: []
created_date: '2026-09-07 14:05'
labels:
  - review-followup
dependencies: []
priority: medium
ordinal: 74000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found during TASK-63's throwaway visual pass over nom-core/assets/weekly_progress_widget.html (two independent fresh-subagent screenshot passes agreed). Two related legibility gaps in the per-day meal-type composition ribbon, both pre-existing and untouched by TASK-63: (1) mealRibbon() clamps a small segment to minSliver = 1.5 user units, which at the shipped 320px-wide layout renders the legacy/unrecognized meal_type bucket as roughly 1 CSS px of solid colour (~2-3 device px at 3x) - present in the data but effectively unreadable as a band, worst in dark mode where --meal-none #71717a also sits near the 3:1 floor against --bg #171717; (2) the ribbon carries no legend, so its four hues are decodable only via each rect's <title> tooltip, which is invisible on touch and print. Suggested directions to evaluate (not decided): raise the minimum segment width or collapse sub-threshold buckets into an explicit remainder, and/or add a compact static legend row.
<!-- SECTION:DESCRIPTION:END -->

---
id: TASK-56
title: 'Compact weekly-progress widget: shrink charts & tighten spacing'
status: To Do
assignee: []
created_date: '2026-08-17 21:12'
labels:
  - widgets
dependencies:
  - TASK-54
references:
  - CONTEXT.md
priority: medium
type: enhancement
ordinal: 62000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The weekly-progress widget (nom-core/assets/weekly_progress_widget.html, served at ui://nom-mcp/weekly-progress) stacks a 140px calorie bar chart and a 100px weight line chart under a title/subtitle and section dividers — roughly 350px tall. It should be more vertically compact since LLM text accompanies it.

Approved approach (user decision): shrink & tighten — keep both charts (calorie bars with dashed goal line; weight line with dashed target line) but cut chart heights by ~40% (~90px and ~70px), and tighten body padding, typography, and section spacing. Target overall height ~200px at 320px width. No structural change: same data, same sections, same interactions (per-bar <title> tooltips, day-of-week labels, weight summary row with Start/End/delta + status chip).

Same constraints as the goal-progress redesign: self-contained HTML, light-dark() + host CSS variables, size-changed reporting, XSS-safe escaping, widget_display_enabled + ui_blocked_clients gating unchanged, existing mcp_handler tests pass.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Rendered height reduced >=35% vs the current widget at equivalent width, while retaining both charts, the goal/target reference lines, and the weight summary row.
- [ ] #2 Per-bar hover tooltips (date + calories) and day-of-week labels remain.
- [ ] #3 Light/dark + host-variable theming, self-containment, and size reporting all intact.
- [ ] #4 Existing mcp_handler resource/gating tests pass unmodified or extended.
- [ ] #5 Manual visual verification on a seeded local instance (TASK-54) in both light and dark mode.
<!-- AC:END -->

---
id: TASK-57
title: 'Weight trend widget: sparkline + week-over-week delta'
status: To Do
assignee: []
created_date: '2026-08-17 21:12'
labels:
  - widgets
dependencies:
  - TASK-54
references:
  - docs/widget-design-reference-goals-cards.png
  - CONTEXT.md
priority: medium
type: feature
ordinal: 63000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The user wants a weight-trend widget: a compact sparkline of recent weight entries plus a week-over-week delta, so trends are visible at a glance next to the LLM's text.

Shape: a new static self-contained asset (alongside nom-core/assets/*.html) served at a new ui:// resource, with a _meta.ui pointer on the relevant weight operation(s) — which exact operations get the pointer (e.g., get_weight_by_date_range, log_weight_entry) is decided at planning time. Gating identical to the existing widget tools (widget_display_enabled + ui_blocked_clients).

Visual (approved design language): ~80-100px tall at 320px wide; minimal axis-free sparkline of the last ~14-30 entries, signed week-over-week delta colored by direction relative to the target (progressing toward target = green, away = red; neutral when no target), optional dashed target line. Light/dark via the existing host-context mechanism; self-contained; size-changed reporting.

Data: weight entries are already stored and range-queryable (get_weight_by_date_range); no schema changes expected.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 At least one weight operation exposes a _meta.ui pointer to the new ui:// weight-trend resource when widget display is enabled, with the same blocklist suppression as the other widget tools.
- [ ] #2 The widget renders a sparkline of the most recent weight entries plus a signed week-over-week delta, colored by direction relative to the target (neutral when no target is set).
- [ ] #3 Sparse data degrades gracefully: 0 or 1 entries shows a short placeholder instead of a broken/empty chart.
- [ ] #4 Self-contained (zero external requests), light/dark themed, size reporting intact.
- [ ] #5 Tests cover resource serving, gating, and the delta computation; manual verification on a seeded local instance (TASK-54).
<!-- AC:END -->

---
id: TASK-58
title: 'Food-added widget on log_meal: food macros + updated daily totals'
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
priority: high
type: feature
ordinal: 64000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
When the user logs food, the LLM replies with text; the user wants a compact widget attached to that moment showing (a) the macros of the just-logged food/meal and (b) their updated daily totals after the addition.

Current gap: log_meal returns {meal_id, logged_at, logged_date, totals:{total_calories,total_protein_g,total_carbs_g,total_fat_g,total_fiber_g}} — no per-food breakdown and no daily context. This task extends the response additively (existing fields unchanged) with:
- Per-portion breakdown: food name + calories/protein/carbs/fat/fiber for each portion logged (respecting quantity mode and any adjustment)
- Post-insertion daily totals with per-nutrient progress (consumed/target/percent/status) — i.e., the same shape get_goal_progress returns for the logged_date

New widget: a static self-contained asset (alongside nom-core/assets/*.html) served at a new ui:// resource, with a _meta.ui pointer on the log_meal tool declaration gated exactly like the existing widget tools (widget_display_enabled flag + ui_blocked_clients suppression).

Visual (approved design language): compact panel, target <= ~150px tall at 320px width — logged food's macros as a compact list/chips on the left, updated daily totals as rings with percent-of-goal inside on the right (colors: under=blue, met=green, over=red). Reference for card/ring styling: docs/widget-design-reference-goals-cards.png.

Scope: log_meal only. update_meal is out of scope (note as follow-up if desired).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 log_meal's response gains a per-portion macro breakdown (food name + calories/protein/carbs/fat/fiber) and post-insertion daily totals with per-nutrient progress (consumed/target/percent/status); all existing response fields are unchanged.
- [ ] #2 A new ui:// resource serves the self-contained widget rendering the logged food's macros next to updated daily-total rings; it appears in resources/list and is discoverable via the tool's _meta.ui.
- [ ] #3 _meta.ui is suppressed when widget display is disabled or the requesting client is blocklisted — identical gating to the existing widget tools.
- [ ] #4 Layout is compact (<= ~150px tall at 320px width), rings carry percent-of-goal, and light/dark theming works via the existing host-context mechanism.
- [ ] #5 Tests cover: multi-portion meal response shape, adjustment handling, resource serving, and gating (enabled/disabled/blocked-client).
- [ ] #6 Manual verification on a seeded local instance (TASK-54): logging a meal shows the widget alongside the text result.
<!-- AC:END -->

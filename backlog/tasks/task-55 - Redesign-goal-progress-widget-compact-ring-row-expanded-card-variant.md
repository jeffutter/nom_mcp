---
id: TASK-55
title: 'Redesign goal-progress widget: compact ring row + expanded card variant'
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
type: enhancement
ordinal: 61000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The goal-progress widget (nom-core/assets/goal_progress_widget.html, served at ui://nom-mcp/goal-progress) is a vertical stack: title, date subtitle, five nutrient rows (label + 'consumed / target' + 6px bar), and a Weight section — roughly 300px tall. Since the LLM usually accompanies the widget with explanatory text, it needs to be significantly more vertically compact.

Approved design (user decision, 2026-08):
- DEFAULT: a single row of small rings — one per nutrient (calories, protein, carbs, fat, fiber), percent-of-goal centered inside each ring, nutrient label below, and weight (latest -> target + status) plus fasting hours in a slim footer line. Target ~100-120px tall at 320px wide.
- EXPANDED: a 2x2 card grid in the style of the reference screenshot docs/widget-design-reference-goals-cards.png (label + 'N under/over' delta on the left, progress ring on the right). Shown when the user explicitly asks for a fuller daily summary; the mechanism (optional argument on get_goal_progress vs a sibling operation) is decided at planning time, but both variants must be reachable from the LLM via the tool's schema/description without breaking existing clients.

Design language (applies to all widget work this round):
- Status colors: under = blue accent, met = green, over = red (existing --under/--met/--over tokens)
- Percent-of-goal inside rings; exact numbers in labels/footer
- Self-contained HTML only (host CSP blocks external requests); light-dark() + host-provided CSS variables; size-changed reporting preserved

Data source is unchanged: get_goal_progress already returns per-nutrient {consumed, target, remaining, percent, status, direction}, weight {latest_weight, target_weight, status}, and fasting_hours.

Must preserve: XSS-safe escaping of all interpolated values, widget_display_enabled gating on _meta.ui, ui_blocked_clients suppression, and existing mcp_handler.rs test coverage (extend as needed).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Default get_goal_progress rendering is a single row of rings <= ~120px tall at 320px width: percent-of-goal inside each ring, nutrient label below, all 5 nutrients present.
- [ ] #2 Nutrients without a target degrade gracefully (neutral ring or no fill, consumed value still shown).
- [ ] #3 Weight (latest vs target + status) and fasting hours render in a compact footer line, hidden gracefully when absent.
- [ ] #4 An expanded 2x2 card variant matching the reference style is available when the user explicitly asks for a fuller daily summary, selectable through the tool's input schema/description without breaking existing clients.
- [ ] #5 Ring colors follow status (under/met/over) and adapt to light/dark themes via the existing host-context mechanism.
- [ ] #6 Widget stays self-contained (zero external requests), keeps size-changed reporting, and existing mcp_handler gating/blocklist/XSS tests still pass.
- [ ] #7 Manual visual verification on a seeded local instance (TASK-54) in both light and dark mode.
<!-- AC:END -->

---
id: TASK-1.11
title: Design the MCP Resource and MCP-only widget-toggle representation
status: Done
assignee:
  - Jeffery Utter
created_date: '2026-08-11 04:40'
updated_date: '2026-08-11 12:46'
labels:
  - 'wayfinder:grilling'
dependencies:
  - TASK-1.6
  - TASK-1.8
parent_task_id: TASK-1
ordinal: 12000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Question

Design the nutrition weekly-summary MCP Resource (what it aggregates, refresh/caching behavior) and the MCP-only widget-display toggle (a setting with no CLI/HTTP equivalent, per the confirmed destination). Cover: how these fit the shared Operation abstraction decided in the transport-architecture ticket without forcing every other Operation to carry MCP-only concerns, and where the widget-toggle setting is persisted.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Resolved via /grilling — all recommended options accepted across three rounds (widget-toggle meaning, resource content/window/caching, per-day vs aggregate + weekly-target expression + weight-gap handling + settings-table shape), then confirmed as a full-design summary before recording.

Domain-modeling pass added Weekly Summary and Widget Display to CONTEXT.md, and recorded decision-3 (DB settings table vs startup Config boundary) since that split is hard-to-reverse-ish, non-obvious to a future reader, and the result of a real trade-off against folding it into TASK-1.12's Config.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
**Weekly Summary Resource** — MCP-only, fixed URI (`nom://weekly-summary`), no params, live-computed on every read (no caching). Stays outside the Operation trait entirely per TASK-1.6's split (hand-written `list_resources`/`read_resource` on ServerHandler; data-fetching lives in a capability-layer function, same as every other piece of domain logic).

Rolling last-7-days window (not calendar week). Two sections:
- **Nutrients**: per-nutrient daily-average consumed vs daily Goal target, shaped like `get_goal_progress` (consumed/target/remaining/percent/direction/status), plus a per-day array of the week's raw daily totals. No-goal-set resolves to null fields, same convention as `get_goal_progress`.
- **Weight**: start/end/delta computed from Weight Entries inside the window; if none logged this week, start/delta are null but `latest_known_weight` still comes from the most recent entry before the window if one exists. Includes the target-weight comparison, mirroring `get_goal_progress`.

**Widget Display setting** — a single global on/off preference (rich MCP-client-rendered widgets vs plain text/JSON), MCP-only with no CLI/HTTP equivalent. `get_widget_display()` / `set_widget_display(enabled: bool)` are ordinary Operations with `surfaces() = MCP only`, dispatched through TASK-1.6's shared registry like everything else — no special-casing needed in the abstraction itself. Adds 2 tools to TASK-1.8's inventory (18 → 20 total).

Persisted in a new single-row `settings` table (`widget_display_enabled BOOLEAN`) in the same local libSQL file as domain data — deliberately separate from TASK-1.12's future startup Config, since this is runtime-mutable via a tool call while Config is read once at process start. Recorded as decision-3.

v1 scope is plumbing only: the setting is stored and readable/writable, but no tool or Resource output branches on it yet — no widget-rendering capability exists to gate. This keeps the toggle from forcing MCP-only concerns onto any other Operation's response shape.

CONTEXT.md gained two terms: Weekly Summary, Widget Display.
<!-- SECTION:FINAL_SUMMARY:END -->

---
id: decision-3
title: 'nom_mcp: widget-toggle settings live in a DB table, not the startup Config'
date: '2026-08-11 12:44'
status: accepted
---
## Context

The Widget Display preference (TASK-1.11) is the first runtime-mutable, MCP-only user setting. TASK-1.12 (not yet designed) will own startup Config: API keys and the timezone fallback, read once at process start. Both are "settings" in a loose sense, and a reader could reasonably expect one unified mechanism.

## Decision

Widget Display persists in its own single-row typed table (`settings`, `widget_display_enabled BOOLEAN`) inside the same local libSQL file as domain data, updated in place by `set_widget_display`. It does not live in whatever Config format TASK-1.12 lands on.

## Consequences

- Any future MCP-only preference that a tool call needs to change at runtime joins this table (one migration per column), not Config.
- TASK-1.12 owns only process-start, effectively-static values (credentials, timezone fallback) — it never needs write-at-runtime support.
- Two places now hold "settings"-flavored data; a reader unaware of this split might wonder why. This decision is that explanation.


---
id: TASK-54
title: Local dev instance with one-command seed data
status: To Do
assignee: []
created_date: '2026-08-17 21:10'
labels:
  - devtooling
  - seed-data
dependencies: []
priority: high
type: task
ordinal: 60000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Widget work (and any e2e validation of the MCP Apps UI) needs a local nom_mcp instance with realistic multi-day data. Today there is no fast way to populate a fresh database: goals, custom foods, meals/portions, and weight entries must all be entered operation-by-operation, which makes iterating on widget visuals slow and painful.

Goal: a single documented command stands up a throwaway local instance pre-populated with seed data, so anyone can iterate on widgets (or any surface) without manual data entry.

Seed content requirements:
- Active nutrition goals with directions, covering calories/protein/carbs/fat/fiber plus a target weight
- A handful of custom foods with realistic macros
- Meals + portions spanning at least 7 days including today, with varied meal timing (so fasting windows exist)
- Weight entries spanning at least 7 days including near today
- Values chosen so statuses span 'under', 'met', and 'over' — every existing widget (goal progress, weekly progress) should render non-trivial content

Constraints:
- Fit the existing architecture: Operation trait + registry (see AGENTS.md). The seed action is local-CLI-only (Surfaces::CLI) — it makes no sense over HTTP/MCP.
- Targets a configurable/throwaway DB path; must never touch the default production DB location.
- Repeatable: re-running against the same path resets to a clean known state.
- Dates deterministic relative to 'today' so status coverage holds whenever it runs.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A single documented command creates a fresh local DB populated with seed data: active goals (all 5 nutrients + target weight, with directions), >=5 custom foods, meals/portions spanning >=7 days including today, and weight entries spanning >=7 days.
- [ ] #2 Seeding writes only to a throwaway/configurable DB path; the default production DB location is never touched.
- [ ] #3 Re-running the seed against the same path resets the database to the same clean known state (repeatable).
- [ ] #4 With seeded data, get_goal_progress returns non-null consumed/target for all 5 nutrients and spans at least one 'under', one 'met', and one 'over' status; get_weekly_progress returns >=7 days of calorie data and >=2 weight points.
- [ ] #5 README documents the full flow with exact commands: seed a fresh DB, start the server (serve http), and connect an MCP client.
- [ ] #6 Integration tests cover seeding against a temp DB (row counts and spot-checked values).
<!-- AC:END -->

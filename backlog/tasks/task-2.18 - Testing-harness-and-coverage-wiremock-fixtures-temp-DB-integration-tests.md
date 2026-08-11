---
id: TASK-2.18
title: 'Testing harness and coverage (wiremock fixtures, temp-DB integration tests)'
status: To Do
assignee: []
created_date: '2026-08-11 13:24'
labels: []
dependencies:
  - TASK-2.8
  - TASK-2.9
  - TASK-2.13
  - TASK-2.14
  - TASK-2.15
  - TASK-2.16
  - TASK-2.17
parent_task_id: TASK-2
type: feature
ordinal: 37000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Scope
Unit tests for pure domain logic (schema validation, goal-progress calc, error-taxonomy mapping) with no I/O. External API integration tests via record-and-replay: fixture JSON files captured from real OpenFoodFacts/USDA FDC responses, served back through wiremock against the OFF/USDA clients' configurable base URLs. DB-layer integration tests use turso's local-file mode with a fresh temp-file DB per test — real schema, no DB mocking. Since Operation logic is unified through the registry, CLI/HTTP/MCP-specific tests stay thin smoke tests for wiring only, not full logic re-tests per surface.

See doc-5 §11.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 wiremock-backed integration tests exist for both the OFF and USDA FDC clients using captured fixture JSON
- [ ] #2 each domain entity (Food, Meal, Portion, Weight Entry, Goal) has integration tests against a fresh temp-file turso DB
- [ ] #3 goal-progress calculation and error-taxonomy mapping have dedicated unit tests with no I/O
- [ ] #4 CLI/HTTP/MCP surface tests are thin wiring smoke tests, not full re-tests of Operation logic already covered above
<!-- AC:END -->

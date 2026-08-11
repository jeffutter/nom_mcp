---
id: TASK-1.14
title: Design testing strategy for external API integrations
status: Done
assignee:
  - '@Jeffery Utter'
created_date: '2026-08-11 13:10'
updated_date: '2026-08-11 13:11'
labels:
  - 'wayfinder:grilling'
dependencies: []
parent_task_id: TASK-1
ordinal: 15000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Question

What unit/integration testing approach and fixture strategy covers nom_mcp, particularly the bespoke reqwest clients for OpenFoodFacts and USDA FDC (TASK-1.3, TASK-1.4 found no usable crates)? Does the answer require any architectural hooks (e.g. configurable base URLs) in TASK-1.6's design?
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Rationale: bespoke reqwest clients (TASK-1.3, TASK-1.4) have no existing mock tooling to lean on, so record-and-replay fixtures are the standard, lowest-effort way to avoid live-network flakiness in tests. Configurable base URL is the one architectural hook this adds to TASK-1.6 — a small, low-risk change (constructor param instead of a baked-in constant).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Unit tests cover pure domain logic (schema validation, goal-progress calc, error-taxonomy mapping) with no I/O. External API integration tests use record-and-replay: fixture JSON files captured from real OpenFoodFacts/USDA FDC responses, served back via wiremock so tests are fast, deterministic, and need no live network or API key in CI. This requires TASK-1.6's reqwest clients to accept a configurable base URL (constructor param, not just an env-derived constant) so tests can point at the local wiremock server. DB-layer integration tests use turso's local-file mode with a fresh temp-file DB per test — real schema, no DB mocking. Operation-trait unification (TASK-1.6) means Operation logic is tested once, surface-specific (CLI/HTTP/MCP) tests stay thin smoke tests for wiring only.
<!-- SECTION:FINAL_SUMMARY:END -->

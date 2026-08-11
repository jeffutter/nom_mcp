---
id: TASK-1.17
title: Design observability/logging approach
status: Done
assignee:
  - '@Jeffery Utter'
created_date: '2026-08-11 13:11'
updated_date: '2026-08-11 13:11'
labels:
  - 'wayfinder:grilling'
dependencies: []
parent_task_id: TASK-1
ordinal: 18000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Question

What logging conventions apply across the four surfaces (MCP, local CLI, HTTP, remote CLI), and is any metrics/tracing-export needed for v1?
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Rationale: tracing is the de facto standard for this ecosystem and likely already used by notectl (the architecture reference); reusing it avoids introducing a second logging convention. Metrics/tracing-export is scoped out for the same reason CI/CD was (TASK-1.15) and multi-user/auth was (map Out of scope) — no audience exists yet to consume it.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Use the 'tracing' crate with 'tracing-subscriber' for structured logging across all four surfaces. Server modes (HTTP/MCP serve) log to stderr at 'info' by default, level overridable via RUST_LOG/config. Local CLI defaults to 'warn' to keep command output clean — user-facing errors surface through TASK-1.13's ErrorData rendering, not raw log lines. External API calls (OpenFoodFacts, USDA FDC) log request outcome (success/failure, status code) at 'debug', and API keys are never logged. No metrics or tracing-export (OpenTelemetry, etc.) for v1 — single-user tool with no ops team monitoring it; deferred to a future effort if the need ever arises.
<!-- SECTION:FINAL_SUMMARY:END -->

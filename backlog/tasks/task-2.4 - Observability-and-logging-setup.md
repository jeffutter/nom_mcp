---
id: TASK-2.4
title: Observability and logging setup
status: To Do
assignee: []
created_date: '2026-08-11 13:23'
labels: []
dependencies:
  - TASK-2.1
parent_task_id: TASK-2
type: chore
ordinal: 23000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Scope
tracing + tracing-subscriber wired across all four surfaces. Server modes (HTTP/MCP serve) log to stderr at info by default, overridable via RUST_LOG/config. Local CLI defaults to warn. External API calls log outcome (success/failure, status code) at debug; API keys never logged. No metrics/tracing-export in v1.

See doc-5 §14.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 tracing-subscriber initialized in the main binary with server default info, CLI default warn
- [ ] #2 RUST_LOG env var overrides the default level
- [ ] #3 grep confirms no API key value is ever passed to a tracing macro
<!-- AC:END -->

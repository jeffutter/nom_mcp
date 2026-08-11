---
id: TASK-2.7
title: Operation trait and multi-surface registry
status: To Do
assignee: []
created_date: '2026-08-11 13:23'
labels: []
dependencies:
  - TASK-2.2
  - TASK-2.5
parent_task_id: TASK-2
type: feature
ordinal: 26000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Scope
Operation trait gains fn surfaces(&self) -> Surfaces (which of CLI/HTTP/MCP; defaults to all three). One registry drives CLI subcommand registration, HTTP route registration, and a hand-written list_tools/call_tool on the MCP ServerHandler that loops the registry directly — deliberately not rmcp's #[tool] macro/ToolRouter, since ToolBase requires compile-time-associated-function types incompatible with a runtime Vec<Arc<dyn Operation>>. This closes, by construction, the CLI/HTTP-vs-MCP drift notectl has today.

See doc-5 §3.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Operation trait defines surfaces() defaulting to all three transports
- [ ] #2 a single registry instance drives CLI subcommand list, HTTP route list, and MCP list_tools output — adding one Operation appears on all three surfaces it declares
- [ ] #3 MCP call_tool dispatches through the same registry via execute_json, no rmcp #[tool] macro used for domain operations
<!-- AC:END -->

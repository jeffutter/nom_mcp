---
id: TASK-2.2
title: Error-handling taxonomy and shared render function
status: To Do
assignee: []
created_date: '2026-08-11 13:23'
labels: []
dependencies:
  - TASK-2.1
type: feature
ordinal: 21000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Scope
ErrorData.data.category taxonomy (NotFound, Validation, Conflict, ExternalApiFailure, StorageFailure) mapped to HTTP status / CLI exit code / MCP CallToolResult. execute_from_args (CLI) changes to return Result<Value, ErrorData>, unifying with execute_json (HTTP/MCP). A shared render function (used by both local-CLI and remote-CLI) turns ErrorData into stderr text + exit code.

See doc-5 §10 for the full category/status/exit-code table and per-category data payload shape.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 ErrorData.data.category enum with the 5 categories implemented
- [ ] #2 HTTP status and CLI exit code mapping matches doc-5 §10's table, unit-tested
- [ ] #3 execute_from_args returns Result<Value, ErrorData>; shared render function used by local-CLI
- [ ] #4 MCP error path returns CallToolResult{is_error: true, content: [Text(json(ErrorData))]}
<!-- AC:END -->

---
id: TASK-2.2
title: Error-handling taxonomy and shared render function
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-11 13:23'
updated_date: '2026-08-11 17:08'
labels:
  - planned
dependencies:
  - TASK-2.2.1
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

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Orchestration Plan for TASK-2.2: Error-handling taxonomy and shared render function

This feature establishes the unified error currency (ErrorData) used across all four surfaces. It breaks into two sequential sub-tickets:

**Phase 1 — TASK-2.2.1: Core types and mappings (foundational)**
- Define ErrorCategory enum with 5 variants + #[non_exhaustive]
- Define ErrorData struct with category, field?, reason? fields
- Derive Serialize/Deserialize/thiserror::Error on both types
- Implement http_status() and exit_code() mapping methods on ErrorCategory
- Add dependencies: thiserror, serde, http crate to nom-core/Cargo.toml
- Write comprehensive unit tests for all mappings and serialization round-trips
- This is purely library code in nom-core/src/error.rs — no I/O, fully testable

**Phase 2 — TASK-2.2.2: Surface integration (depends on 2.2.1)**
- Implement render_error(&ErrorData) -> (String, i32) in nom-core/src/error/render.rs
- Wire local-CLI main to use Result<Value, ErrorData> and render_error on failure
- Wire remote-CLI to deserialize ErrorData from HTTP responses and reuse render_error
- Add MCP handler stub that returns CallToolResult::error(json(ErrorData))
- Add HTTP handler stub that serializes ErrorData as JSON body with correct status
- Handle lock-probe as Conflict with reason "local_db_locked" and CLI-specific message

**Integration verification:**
- Build workspace succeeds with new error module
- Unit tests pass for category mappings and serialization
- Local-CLI prints rendered error to stderr and exits with correct code
- Remote-CLI produces identical output for the same ErrorData (shared function proof)
- MCP and HTTP error paths return structured error data

Execution order: TASK-2.2.1 first (pure types), then TASK-2.2.2 (surface wiring). Both are leaf tasks ready for execution.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- Implemented ErrorCategory enum with 5 variants + #[non_exhaustive]
- ErrorData struct with category, field?, reason? fields; derives Serialize/Deserialize/thiserror::Error
- http_status() and exit_code() mapping methods match doc-5 §10 table exactly
- render_error() shared function in nom-core used by both local-CLI and remote-CLI
- Lock-probe renders as Conflict with CLI-specific message
- execute_from_args returns Result<Value, ErrorData>
- Remote-CLI deserializes ErrorData from HTTP responses through same render path
- MCP/HTTP error handling stubs documented for future tasks
- 15 unit tests: mapping verification, serialization shapes per category, round-trip, render output
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Implemented unified error taxonomy (ErrorCategory enum + ErrorData struct) in nom-core/src/error.rs with 5 categories mapped to HTTP status codes and CLI exit codes per doc-5 §10. Created shared render_error() function used by both local-CLI and remote-CLI. Wrote execute_from_args() returning Result<Value, ErrorData>. Added MCP/HTTP error handling stubs for future tasks. 15 unit tests cover all mappings, serialization shapes, round-trips, and render output.
<!-- SECTION:FINAL_SUMMARY:END -->

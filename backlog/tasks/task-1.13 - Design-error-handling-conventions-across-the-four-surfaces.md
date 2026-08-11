---
id: TASK-1.13
title: Design error-handling conventions across the four surfaces
status: Done
assignee:
  - Jeffery Utter
created_date: '2026-08-11 05:32'
updated_date: '2026-08-11 13:04'
labels:
  - 'wayfinder:grilling'
dependencies:
  - TASK-1.6
parent_task_id: TASK-1
ordinal: 14000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Question

Given TASK-1.6's resolution, `Operation::execute_json` returns `Result<Value, ErrorData>` and is the common currency for both HTTP and MCP dispatch, but CLI dispatch (`execute_from_args`) returns a separate `Result<String, Box<dyn Error>>`. Pin down a consistent error-handling convention across all four surfaces (MCP, local CLI, HTTP, remote-CLI): what error taxonomy exists (e.g. not-found, validation, external-API-failure, storage-failure), how each maps to HTTP status codes, MCP `CallToolResult` error content, and CLI exit codes/messages, and whether `execute_from_args`'s CLI-only error type should be unified with `ErrorData` or intentionally kept separate. Also cover how the local-CLI's new runtime lock-probe safety check (from TASK-1.6) surfaces its error consistently with this taxonomy.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Alternatives weighed: kept CLI's Box<dyn Error> separate with a translation boundary (rejected — reintroduces a drift point the same way notectl's un-enforced MCP/CLI/HTTP split did, which TASK-1.6 already closed structurally). 4-category taxonomy without a distinct Conflict bucket (rejected — the lock-probe rejection and any future concurrent-write case need a category whose remedy is 'retry or use a different path', not 'storage is broken'). Overloading ErrorData.code with custom per-category integers (rejected — code keeps its JSON-RPC-spec meaning; category lives in the named, self-documenting data.category field instead of a numeric range the caller has to memorize). Single generic CLI exit code (rejected — category-specific codes are cheap given the taxonomy already exists, and let scripts branch without parsing stderr). HTTP error body wrapped in an idiomatic {"error": {...}} envelope (rejected — raw ErrorData JSON lets remote-CLI reuse local-CLI's render function byte-for-byte instead of unwrapping first). Grounded in rmcp 2.2.0's actual ErrorData/CallToolResult shapes (registry+https://github.com/rust-lang/crates.io-index, rmcp-2.2.0/src/model.rs) rather than assumption — ErrorCode is a JSON-RPC-flavored newtype over i32 with only ~7 built-in constants, and CallToolResult.content is Vec<ContentBlock>, not a raw JSON slot, which is why the structured data field needed a home in a serialized text block rather than a dedicated struct field.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
CLI dispatch (`execute_from_args`) is changed to return `Result<Value, ErrorData>`, unifying all four surfaces on one error currency. A shared render function (used by both local-CLI and remote-CLI) turns an `ErrorData` into stderr text + exit code.

Taxonomy lives in `ErrorData.data.category` (not the JSON-RPC `code` field, which stays coarse/spec-standard): NotFound, Validation, Conflict, ExternalApiFailure, StorageFailure. No separate "Internal" category — unclassified/protocol-level errors (rmcp's own rejections, HTTP router 404s that never reach `Operation::execute_json`) fall back to StorageFailure-tier treatment for status/exit-code purposes.

| Category | `code` | HTTP | CLI exit |
|---|---|---|---|
| NotFound | RESOURCE_NOT_FOUND | 404 | 3 |
| Validation | INVALID_PARAMS | 400 | 4 |
| Conflict | INTERNAL_ERROR | 409 | 5 |
| ExternalApiFailure | INTERNAL_ERROR | 502 | 6 |
| StorageFailure | INTERNAL_ERROR | 500 | 7 |
| uncategorized (fallback) | as rmcp sets it | 500 | 7 |

Unclassified/panic path keeps exit code 1.

`data` payload per category: Validation carries `{category, field, reason}` (structured field-level detail); Conflict/ExternalApiFailure/StorageFailure carry `{category, reason}`; NotFound needs only `category`.

Wire representation is the same `ErrorData` JSON across every non-CLI-native surface: HTTP response body is the raw serialized `ErrorData` (status set per the table) so the remote-CLI binary can deserialize it and feed it through the exact same render function local-CLI uses — local-CLI and remote-CLI produce identical error output. MCP returns `CallToolResult{ is_error: true, content: [Text(json(ErrorData))] }`. The CLI render function prints `message` then unpacks `data`'s extra fields as detail lines before exiting with the category's code.

The local-CLI's runtime lock-probe (TASK-1.6) is not a new category — it's an ordinary Conflict with `reason: "local_db_locked"`, rendered with a CLI-specific message ("server is running — stop it or use the remote-CLI instead").
<!-- SECTION:FINAL_SUMMARY:END -->

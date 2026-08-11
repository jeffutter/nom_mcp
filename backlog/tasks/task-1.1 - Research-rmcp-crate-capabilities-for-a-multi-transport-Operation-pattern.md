---
id: TASK-1.1
title: Research rmcp crate capabilities for a multi-transport Operation pattern
status: Done
assignee:
  - '@Jeffery Utter'
created_date: '2026-08-11 04:39'
updated_date: '2026-08-11 04:45'
labels:
  - 'wayfinder:research'
dependencies: []
documentation:
  - doc-1
parent_task_id: TASK-1
ordinal: 2000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Question

What does the `rmcp` crate (https://crates.io/crates/rmcp) actually provide, and how well does it fit a notectl-style shared Operation trait (notectl-core/src/operation.rs) that drives MCP tools, an HTTP REST surface, and CLI parsing from one definition?

Cover: current version/feature flags (notectl pins `rmcp = { version = "2.2", features = ["server", "transport-io", "transport-streamable-http-server"] }`), how tools and resources are defined (schemars-based schema derivation?), transport support (stdio vs streamable-http-server), whether rmcp has any concept of MCP-only extras (e.g. a resource, or metadata that has no HTTP/CLI equivalent) relevant to the weekly-summary MCP Resource and MCP-only widget-toggle tools this project wants, and any gotchas notectl's own code works around.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
rmcp (official modelcontextprotocol/rust-sdk crate, now at 3.1.2 vs notectl's pinned 2.2) fits the tool+transport half of a shared Operation pattern well: schemars-derived request structs feed both rmcp's #[tool]/#[tool_router] macro-generated MCP dispatch and a generic input_schema() used for HTTP/CLI, and stdio/streamable-HTTP are thin swappable transports over one ServerHandler (streamable-HTTP needs an explicit LocalSessionManager, gated in current rmcp behind a transport-streamable-http-server-session feature notectl doesn't declare). rmcp has no macro or built-in concept for either MCP Resources (hand-written list_resources/read_resource on ServerHandler) or 'MCP-only' tools — both are notectl-core inventions: Resources need bespoke glue, and an MCP-only tool is simply a #[tool] that is never wired into the shared CLI/HTTP operation-registration list. Given rmcp's fast-moving API (real renames between 2.2 and 3.1.2 in about a month), nom_mcp should pin an exact version and re-verify feature flags/builder APIs against docs.rs at implementation time.
<!-- SECTION:FINAL_SUMMARY:END -->

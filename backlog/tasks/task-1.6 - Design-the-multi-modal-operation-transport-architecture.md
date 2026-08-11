---
id: TASK-1.6
title: Design the multi-modal operation/transport architecture
status: Done
assignee:
  - Jeffery Utter
created_date: '2026-08-11 04:39'
updated_date: '2026-08-11 05:31'
labels:
  - 'wayfinder:grilling'
dependencies:
  - TASK-1.1
parent_task_id: TASK-1
ordinal: 7000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Question

Pin down the workspace and binary layout for exposing one operation set across MCP / local CLI / HTTP / remote-CLI, following notectl's pattern (notectl-core/src/operation.rs's Operation trait; src/main.rs as the binary that runs `serve` (stdio MCP + HTTP MCP + REST) and local CLI commands against a local DB; src/bin/notectl-remote.rs as a thin binary that only does HTTP calls). Cover: crate/workspace structure for this project, how the Operation trait needs to change (if at all) for this domain, how the MCP-only weekly-summary Resource and MCP-only widget-toggle tools fit an abstraction built around Operations that are supposed to work everywhere, and how the local binary decides at runtime whether a CLI invocation should hit the local Turso file directly vs (never, per the confirmed destination) a remote server.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Grounded in two Explore-agent reads of /home/jeffutter/src/notectl: (1) notectl-core's Operation trait shape, workspace layout, main.rs/notectl-remote.rs wiring, confirmed MCP tools never go through Operation::execute_json — they're hand-written #[tool] methods calling the same capability methods, and notectl has a real unenforced gap (3 outline ops present in CLI/HTTP but absent from MCP). No MCP Resources exist in notectl at all. (2) notectl's one precedent for non-macro MCP dispatch — the hand-rolled AsyncTool<TaskSearchService>/ToolBase impl for search/build_search_index (mcp.rs:200-309) — which revealed ToolBase's name()/description()/input_schema() are compile-time associated functions on a distinct Rust type per tool, not instance methods, so it cannot be driven by a runtime Vec<Arc<dyn Operation>> the way CLI/HTTP registration is; the actual mechanism has to be a hand-written list_tools/call_tool that loops the registry directly, not a generalization of AsyncTool.

Alternatives weighed: workspace — mirror notectl's per-feature crate split (rejected, entities are relationally coupled not independent plugins) or split by integration boundary into separate nom-openfoodfacts/nom-usda crates (rejected in favor of keeping them as modules in nom-core, no reuse case outside this project). Operation/MCP wiring — mirror notectl exactly with no parity enforcement (rejected, inherits notectl's proven drift bug) or mirror-plus-a-parity-test (rejected once the surfaces() unification was on the table, since a single registry makes drift structurally impossible rather than just tested-for) or hybrid generic-for-shared/macro-for-MCP-only (rejected, user chose full unification). Local-CLI posture — deliberate escape hatch via flag/namespace (rejected, user chose first-class/notectl-style) or a separate minimal binary (rejected, same reason). Local-CLI safety — documentation-only (rejected, user chose runtime lock probe given the path is now prominent rather than hidden). Resource logic placement — inline in the handler (rejected, user chose capability-layer function for consistency).

Follow-on: this resolution sharpens the map's "Error-handling conventions across the four surfaces" fog item into a ticketable question, since execute_json's Result<Value, ErrorData> is now confirmed as the common currency for HTTP+MCP while CLI's execute_from_args uses a separate Result<String, Box<dyn Error>> — graduating as a new ticket.
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @Jeffery Utter
created: 2026-08-11 04:56
---
Depends on how TASK-1.5 resolves the turso multi-process question. User wants to keep a local-CLI-hits-DB-directly mode mainly for debugging/testing (low frequency, not the primary workflow) — factor that into whether it's worth keeping as a first-class mode or should just be a debug-only escape hatch layered differently than the main server+remote-CLI flow.
---
<!-- COMMENTS:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Two-crate workspace: unified `nom-core` library (Operation trait, all five entities' capability logic, storage access, and both external API clients as modules — skipping notectl's per-feature crate split since the entities are relationally coupled, not independent plugins) plus the binary package (main.rs: serve + local CLI; a second src/bin/ target: thin remote-CLI client), mirroring notectl's main+remote binary split.

Operation trait gains one new method: `fn surfaces(&self) -> Surfaces` (which of CLI/HTTP/MCP an operation is exposed on; defaults to all three). One operation registry now drives all three transports: CLI subcommand registration, HTTP route registration, and a hand-written `list_tools`/`call_tool` on the MCP ServerHandler that loops the registry directly, bypassing rmcp's `#[tool]` macro/ToolRouter (whose `ToolBase` trait requires compile-time-associated-function types incompatible with a runtime `Vec<Arc<dyn Operation>>`). `execute_json`'s `Result<Value, ErrorData>` converts almost directly into `CallToolResult`. This closes, by construction, the silent CLI/HTTP-vs-MCP drift that notectl actually has today (3 outline operations missing from MCP with nothing catching it).

MCP-only widget-toggle tools become ordinary Operations with `surfaces() = MCP only`, dispatched through the same generic mechanism as everything else. The MCP-only weekly-summary Resource is different in kind (no CLI/HTTP shape, not a Tool) and stays outside the Operation trait — hand-written list_resources/read_resource glue on ServerHandler (no rmcp macro or notectl precedent either way) — but its data-fetching logic lives in a capability-layer function, consistent with where every other piece of domain logic lives.

Local-CLI-direct-DB is not a runtime decision in the local binary at all: it always executes Operations in-process against the local DB, first-class and top-level alongside `serve` (matching notectl). Remote access is exclusively the separate thin remote-CLI binary over HTTP — the local binary structurally never talks remote. Given TASK-1.5's clean-close/checkpoint invariant, local-CLI adds a runtime lock probe (on the same POSIX advisory lock turso already takes) before opening the DB directly, failing fast if the server appears to hold it rather than risking silent WAL corruption.
<!-- SECTION:FINAL_SUMMARY:END -->

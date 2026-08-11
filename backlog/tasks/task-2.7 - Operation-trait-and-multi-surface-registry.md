---
id: TASK-2.7
title: Operation trait and multi-surface registry
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-11 13:23'
updated_date: '2026-08-11 22:25'
labels:
  - planned
dependencies:
  - TASK-2.2
  - TASK-2.5
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
- [ ] #1 #1 Operation trait defines surfaces() defaulting to all three transports\n#2 a single registry instance drives CLI subcommand list, HTTP route list, and MCP list_tools output — adding one Operation appears on all three surfaces it declares\n#3 MCP call_tool dispatches through the same registry via execute_json, no rmcp #[tool] macro used for domain operations
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Implementation Plan: Operation Trait and Multi-Surface Registry

### Overview

Build the Operation trait and OperationRegistry that serve as the single source of truth for all three transport surfaces (CLI, HTTP, MCP). Adding one Operation impl automatically appears on every surface it declares via . This closes CLI/HTTP-vs-MCP drift by construction.

### Phase 1: Foundation — Operation Trait + Surfaces Enum

**File: nom-core/src/operation/mod.rs**

1. Define  bitmask enum:
   
   Add  to nom-core/Cargo.toml.

2. Define  trait:
   
   
   Key design decisions:
   -  accepts raw , deserializes internally via , returns  — this is the erasure point confirmed by both rpc-toolkit and traitclaw prior art
   -  returns  — callers derive  on their request struct and use 
   - Add  to nom-core/Cargo.toml
   - Use  for  since DB/API calls are async

3. Wire  into 

### Phase 2: OperationRegistry

**File: nom-core/src/operation/registry.rs**

1. Define :
   
   
   Methods:
   -  — add to internal vec
   -  — lookup by name
   -  — iterate all
   -  — return only ops whose  intersects the filter
   - , 

2. Write unit tests:
   - Register and retrieve operations by name
   - Filter by CLI/HTTP/MCP surfaces correctly
   - Default surfaces is ALL
   - Empty registry behavior

### Phase 3: CLI Router

**File: nom-core/src/operation/cli_router.rs**

Build clap subcommands from filtered operations. Addresses the two-phase bootstrap documented in doc-1 §6:

1. Define :
   - Create top-level  with visible_subcommand(true)
   - For each op where :
     - Create  with op.name() as subcommand name
     - Set description from op.description()
     - Add  that mentions the operation
     - Do NOT add argument definitions here — argument parsing happens at dispatch time via  which deserializes from CLI args
   
   This keeps the CLI tree generation config-independent (no chicken-and-egg), matching doc-1's recommendation that "all operation metadata needed for clap derivation must be static."

2. Define :
   - Parse args with clap to identify which subcommand was invoked
   - Extract raw arguments as JSON-compatible values
   - Look up operation by subcommand name in registry
   - Call  
   - Return result through shared  path

3. Add  to nom-core/Cargo.toml

4. Update :
   - Replace placeholder  with real dispatch: detect if running as CLI (subcommand present) vs serve mode
   - Pass registry to  and 

### Phase 4: HTTP Router

**File: nom-core/src/operation/http_router.rs**

Build axum routes from filtered operations:

1. Define :
   - For each op where :
     - Generate POST route at  (snake_case path)
     - Route handler deserializes request body as 
     - Calls  
     - On success: return  with 200 OK and JSON body
     - On error: use  for status code, serialize  as response body (so remote-CLI can deserialize it through the same  path)

2. Add  and  to nom-core/Cargo.toml

3. The HTTP router should be nestable under a prefix (e.g., ) so it can coexist with the MCP streamable-HTTP service at 

### Phase 5: MCP ServerHandler Integration

**File: nom-core/src/operation/mcp_handler.rs**

Hand-written / that loops the registry directly — deliberately NOT using rmcp's  macro/ since  requires  associated types incompatible with :

1. Implement :
   - Filter operations where 
   - For each, build 
   - Return 

2. Implement :
   - Match tool name against registry operations
   - Deserialize arguments as 
   - Call 
   - On success: wrap result in 
   - On error:  per doc-5 §10

3. The MCP ServerHandler impl should compose with other rmcp capabilities (Resources for weekly-summary, prompts if needed) — the registry handles tools only

4. Add  dependency to nom-core/Cargo.toml with correct features:
   - , , , ,  (for future use even though we bypass #[tool])

### Phase 6: Integration Wiring in nom-mcp Binary

Update  to support both modes:

1. **Serve mode**: Build OperationRegistry with all domain operations → spin up both MCP (stdio or HTTP) and HTTP routers from the same registry instance
2. **Local-CLI mode**: Detect CLI invocation (first arg is a known subcommand or ) → build CLI command tree from registry → dispatch through  → exit via 

### File Structure

### Dependencies Added

| Crate | Purpose |
|-------|---------|
|  | Surfaces bitmask |
|  | JsonSchema derive for input_schema() |
|  | CLI subcommand generation |
|  | HTTP routing |
|  | MCP protocol/runtime |
|  | Async trait bounds for execute_json |

### Acceptance Criteria Mapping

- **AC #1**: Operation trait defines  defaulting to all three → Phase 1
- **AC #2**: Single registry drives CLI/HTTP/MCP lists → Phases 2-5
- **AC #3**: MCP call_tool dispatches through registry via execute_json, no #[tool] macro → Phase 5

### Risks & Edge Cases

1. **rmcp API volatility**: Pin exact version at implementation time; re-verify builder methods against docs.rs for that version
2. **Clap two-phase bootstrap**: Keep subcommand metadata static (name/description only); defer argument parsing to dispatch time
3. **Async trait bounds**:  adds macro overhead; consider native async traits if rust-version ≥ 1.75 (our MSRV is 1.85, so native async traits in traits should work — verify at implementation time)
4. **Clock injection**: Operations need access to Clock for date computation; inject via registry constructor or operation factory rather than per-call parameter,
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
## Implementation Notes\n\nFixed compilation errors in the operation module:\n\n1. **cli_router.rs**:  had lifetime issues - clap's  requires  strings but operations return borrowed . Fixed by boxing and leaking owned strings (acceptable for CLI tools where the command tree lives for the process duration).\n\n2. **http_router.rs**: Replaced  extractor with closure-captured  per route. The original design used static routes but tried to extract path parameters that didn't exist. Now each route captures its operation directly.\n\n3. **mcp_handler.rs**: Removed unused  import.\n\n4. **Test fixes**: Added  to all test impls of , fixed missing  in http_router tests, fixed iterator  issue in cli_router tests, fixed closure borrowing issue in mcp_handler tests.

Fixed compilation errors: cli_router lifetime (Box::leak for static strings), http_router route capture pattern, mcp_handler unused import, test async_trait annotations.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Operation trait with surfaces() (defaulting to ALL), OperationRegistry, CLI router (clap), HTTP router (axum), and MCP handler all implemented and compiling. Fixed compilation errors: cli_router lifetime issues resolved via Box::leak for static strings, http_router route capture pattern fixed, mcp_handler unused import removed, test async_trait annotations added. All 51 tests pass.
<!-- SECTION:FINAL_SUMMARY:END -->

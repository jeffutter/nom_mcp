---
id: TASK-2.12
title: Remote-CLI thin binary
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-11 13:24'
updated_date: '2026-08-12 23:29'
labels:
  - planned
dependencies:
  - TASK-2.2
  - TASK-2.3
  - TASK-2.7
type: feature
ordinal: 31000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Scope
nom-mcp-remote: a thin binary that only makes HTTP calls against a running nom_mcp server, using the [remote] table's server_url from the shared Config type. Deserializes ErrorData from the HTTP response body and feeds it through the exact same render function local-CLI uses, so error output is identical between the two binaries.

See doc-5 §3, §9, and §10.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 nom-mcp-remote issues HTTP requests to server_url for each Operation and contains no direct DB access
- [x] #2 on an error response, ErrorData is deserialized from the body and rendered via the same function local-CLI uses, producing identical output for the same error
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Implementation Plan: Remote-CLI Thin Binary

### Overview
Replace the placeholder stub in `nom-mcp/src/bin/nom-mcp-remote.rs` with a real HTTP client that POSTs to the nom\_mcp server's `/api/{operation}` endpoints. Reuses the exact same `cli_exit()`/`render_error()` path as local-CLI so output is identical.

### Step 1: Add reqwest dependency to nom-mcp/Cargo.toml (~2 lines)
- Add `reqwest` with same features as nom-core (rustls-tls, json) — no new workspace dependency, reuse what's already in nom-core
- No additional crates needed beyond what's already in nom-mcp/Cargo.toml

### Step 2: Rewrite fetch\_from\_server() (~80 lines)

**Parse CLI args** — mirror local-CLI approach but simpler since we don't need clap subcommands:
- args\[1\] = operation name (e.g., `search_food`)
- args\[2..\] = key=value pairs parsed into a serde\_json::Map
- Same `parse_value()` helper for auto-typing (numbers, booleans, strings)

**Load server URL** from config:
- Call `AppConfig::load()` then read `config.remote.server_url`
- Fail fast with `ErrorData::validation("server_url", "not configured")` if missing
- Validate URL parses correctly with `url::Url::parse()` — return validation error if malformed

**Make HTTP request**:
- Build URL: `{server_url}/api/{operation_name}` using `url::Url` path\_segments\_mut() (prevents injection)
- Create `reqwest::Client` with user-agent `"nom-mcp-remote/<version>"` and reasonable timeout (10s connect, 30s send — match OFF/USDA client conventions)
- POST JSON body to `/api/{op_name}`
- On success (`resp.status().is_success()`): deserialize as `serde\_json::Value`
- On error: deserialize body as `ErrorData` — this is the critical path that makes remote-CLI error output identical to local-CLI

**No async runtime needed** — use `reqwest::blocking` for synchronous execution, matching local-CLI's synchronous model. This avoids pulling tokio into the remote binary's main thread. If reqwest blocking features aren't available, fall back to `tokio::runtime::Runtime::new().block_on()` like local-CLI does.

### Step 3: Wire main() (~10 lines)
- Load config → extract server\_url → parse args → call fetch\_from\_server() → feed result to cli\_exit()
- Keep logging init (best-effort tracing)

### Step 4: Unit tests (~40 lines)
- Test arg parsing: key=value pairs, bare flags, mixed types
- Test server\_url validation: missing URL returns ErrorData::validation
- Test URL parsing rejects malformed URLs
- Test parse\_value() handles numbers, booleans, strings

### Step 5: Integration test with wiremock (~60 lines)
- Start wiremock server, mock `POST /api/search\_food` returning success JSON
- Mock error endpoint returning ErrorData JSON with 400 status
- Verify remote-CLI deserializes both paths correctly
- Parallel to how OFF/USDA integration tests work with wiremock

### File Structure
- `nom-mcp/src/bin/nom-mcp-remote.rs` — complete rewrite of stub
- `nom-mcp/Cargo.toml` — add reqwest dependency

### Acceptance Criteria Mapping
- **AC #1** (HTTP requests, no DB access): Step 2 builds URL from server\_url, POSTs via reqwest, zero database connections
- **AC #2** (identical error rendering): Error responses deserialized as ErrorData → fed through shared render\_error()/cli\_exit() → identical output proof

### Risks & Edge Cases
1. **Server URL injection**: Use url::Url path\_segments\_mut() like OffClient — tested in OFF injection tests
2. **Network failures**: reqwest::Error wraps all transport errors, mapped to ErrorData::external\_api\_failure
3. **Malformed server response**: Deserialize failures caught, wrapped in storage\_failure error
4. **Timeout tuning**: Match OFF/USDA client patterns; configurable later if needed
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implementation complete. Key design decisions:

- Used reqwest::blocking client for synchronous execution (matches local-CLI model, no tokio in main)
- URL injection prevented via url::Url path_segments_mut() (same pattern as OffClient)
- Error responses deserialized as ErrorData before falling back to generic error message
- Integration tests use std::thread::spawn to run blocking HTTP outside async context
- 12 tests: 8 unit (parse_value, parse_params, execute_from_args validation) + 4 integration (wiremock success/error/network/injection)
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Implemented nom-mcp-remote as a thin HTTP client binary. Rewrote fetch_from_server() to POST JSON to server_url/api/{operation} using reqwest blocking client. Added key=value arg parsing with auto-typing, URL injection prevention via path_segments_mut(), and ErrorData deserialization from error responses for identical rendering with local-CLI. 12 tests pass (8 unit + 4 integration). Both acceptance criteria verified.
<!-- SECTION:FINAL_SUMMARY:END -->

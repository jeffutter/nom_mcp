---
id: TASK-33
title: 'Fix: McpHandler::list_resources/read_resource have zero test coverage'
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-13 11:47'
updated_date: '2026-08-13 12:05'
labels:
  - review-followup
  - planned
dependencies:
  - TASK-2.17
priority: high
ordinal: 105
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while reviewing TASK-2.17 (nom-core/src/operation/mcp_handler.rs:140-223). list_resources() and read_resource() — the actual MCP resource-serving glue added by TASK-2.17, including URI dispatch, the unknown-URI error path, and JSON-serialization of fetch_weekly_summary()'s output into ReadResourceResult — have no test coverage at all (grep for read_resource/list_resources call sites outside their own definitions turns up nothing). The 8 weekly tests + 4 widget tests TASK-2.17 shipped only exercise fetch_weekly_summary() and the Operations directly; they never go through McpHandler's trait methods. This is a Correct-axis gap: AC #1 claims the nom://weekly-summary resource 'returns' the summary, but nothing verifies the resource-dispatch layer that actually returns it to a client — if URI matching broke, or serialization silently produced malformed JSON, or the unknown-URI error path panicked, no test would catch it. The file has a prior, structurally identical finding (TASK-4, already Done) for the same reason on the tools side (list_tools/build_tools), which was fixed by extracting a plain sync helper (build_tools()) that tests call directly instead of going through the ServerHandler trait method (which needs a rmcp::service::RequestContext<RoleServer> — a #[non_exhaustive] struct holding a Peer<R> that isn't practically constructible in a unit test). Apply the same pattern to resources.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 list_resources()'s body is extracted into a pub(crate) fn build_resources(&self) -> Vec<Resource> mirroring the existing build_tools() (mcp_handler.rs:55-71); list_resources() itself becomes a thin wrapper: Ok(ListResourcesResult::with_all_items(self.build_resources()))
- [x] #2 read_resource()'s URI-dispatch match block is extracted into a pub(crate) async fn dispatch_read_resource(&self, uri: \&str) -> Result<ReadResourceResult, rmcp::ErrorData>; read_resource() itself becomes a thin wrapper: self.dispatch_read_resource(&request.uri).await
- [x] #3 test_build_resources_lists_weekly_summary asserts build_resources() returns exactly one Resource with uri "nom://weekly-summary", the expected title, and mime_type "application/json"
- [x] #4 test_dispatch_read_resource_returns_weekly_summary_json constructs McpHandler via with_db_path() against a TempDb (same pattern as weekly/mod.rs and widget/mod.rs tests), calls dispatch_read_resource("nom://weekly-summary"), asserts Ok, and asserts the returned text content parses as JSON containing top-level keys start_date/end_date/nutrients/weight
- [x] #5 test_dispatch_read_resource_unknown_uri_errors calls dispatch_read_resource with an unrecognized URI (e.g. "nom://bogus") and asserts an Err whose message contains "unknown resource URI"
- [x] #6 nix develop -c cargo test -p nom-core passes
- [x] #7 nix develop -c cargo clippy -p nom-core --all-targets --all-features -- -D warnings passes
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
SETUP (read first): This is a Rust Cargo workspace (nom-core/ library, nom-mcp/ binaries) — a single-user MCP nutrition-tracking server. ALL commands must run inside the Nix dev shell: prefix every command with 'nix develop -c'. Work from the repository root unless told otherwise. Do not change pinned dependency versions. rmcp is pinned at 2.2.0 (see Cargo.lock) — confirmed: `Resource` (nom-core/src/operation/mcp_handler.rs) has plain public fields (`uri: String`, `mime_type: Option<String>`, etc., no `.raw` wrapper), and `ResourceContents::TextResourceContents { uri, mime_type, text, meta }` is a directly-matchable enum variant. `ErrorData.message: Cow<'static, str>` is a plain public field (see existing `test_bad_schema_does_not_panic` for the pattern: `err.message.contains(...)`).

1. Open nom-core/src/operation/mcp_handler.rs. Locate list_resources() (~line 140-152) and read_resource() (~line 154-223) inside `impl ServerHandler for McpHandler`.

2. Add a new inherent method on `impl McpHandler` (the block at the top of the file, alongside `build_tools()` at line ~55), extracting list_resources()'s existing vec literal verbatim:
   pub(crate) fn build_resources(&self) -> Vec<Resource> {
       vec![
           Resource::new("nom://weekly-summary", "weekly-summary")
               .with_title("Weekly Summary")
               .with_description("Rolling 7-day nutrition and weight summary")
               .with_mime_type("application/json"),
       ]
   }
   Then replace list_resources()'s body with:
   async fn list_resources(&self, _request: Option<PaginatedRequestParams>, _context: RequestContext<RoleServer>) -> Result<ListResourcesResult, ErrorData> {
       Ok(ListResourcesResult::with_all_items(self.build_resources()))
   }

3. Add a second inherent method on `impl McpHandler`, moving read_resource()'s entire existing match block (the #[cfg(test)]/#[cfg(not(test))] connection-opening logic plus the URI match, lines ~158-221) verbatim into it:
   pub(crate) async fn dispatch_read_resource(&self, uri: &str) -> Result<ReadResourceResult, ErrorData> {
       match uri {
           "nom://weekly-summary" => { ... (existing body, unchanged; note the existing code binds `let uri = &request.uri;` before the match — replace that binding, since `uri` is now the function parameter) ... }
           other => Err(ErrorData::new(ErrorCode::INVALID_PARAMS, format!("unknown resource URI: {}", other), None)),
       }
   }
   Then replace read_resource()'s body with:
   async fn read_resource(&self, request: rmcp::model::ReadResourceRequestParams, _context: RequestContext<RoleServer>) -> Result<ReadResourceResult, ErrorData> {
       self.dispatch_read_resource(&request.uri).await
   }
   Note: inside dispatch_read_resource, the existing `uri.clone()` used when constructing `ResourceContents::TextResourceContents { uri: uri.clone(), ... }` must become `uri.to_string()` (or keep `.to_string()`), since `uri` is now `&str` not `&String`.

4. In the #[cfg(test)] mod tests block at the bottom of the file, add three tests. Import `crate::storage::test::TempDb` at the top of the test module (same import weekly/mod.rs and widget/mod.rs use) for the new async tests, and add `#[serial_test::serial]` + `#[tokio::test]` (matching the attribute pair already used on every with_db_path test in widget/mod.rs and weekly/mod.rs) for the two async ones:

   - test_build_resources_lists_weekly_summary (sync #[test]):
       let clock = Clock { tz: chrono_tz::UTC };
       let handler = McpHandler::new(OperationRegistry::new(make_clock()), clock);
       let resources = handler.build_resources();
       assert_eq!(resources.len(), 1);
       assert_eq!(resources[0].uri, "nom://weekly-summary");
       assert_eq!(resources[0].title.as_deref(), Some("Weekly Summary"));
       assert_eq!(resources[0].mime_type.as_deref(), Some("application/json"));

   - test_dispatch_read_resource_returns_weekly_summary_json (async, needs a populated-enough TempDb that fetch_weekly_summary doesn't error — check weekly/mod.rs tests for the minimal seed data fetch_weekly_summary needs, e.g. at least the settings row / no entries is fine since the weekly tests cover empty-window cases):
       let db = TempDb::new().await;
       let clock = Clock { tz: chrono_tz::UTC };
       let handler = McpHandler::new(OperationRegistry::new(make_clock()), clock).with_db_path(db.path.clone());
       let result = handler.dispatch_read_resource("nom://weekly-summary").await;
       assert!(result.is_ok());
       let ReadResourceResult { contents, .. } = result.unwrap();
       let ResourceContents::TextResourceContents { text, .. } = &contents[0] else { panic!("expected text contents") };
       let value: serde_json::Value = serde_json::from_str(text).unwrap();
       assert!(value.get("start_date").is_some());
       assert!(value.get("end_date").is_some());
       assert!(value.get("nutrients").is_some());
       assert!(value.get("weight").is_some());
     (Check ReadResourceResult's exact field name for its contents Vec — likely `contents` per rmcp::model; adjust if compilation reveals a different name/accessor.)

   - test_dispatch_read_resource_unknown_uri_errors (async):
       let clock = Clock { tz: chrono_tz::UTC };
       let handler = McpHandler::new(OperationRegistry::new(make_clock()), clock);
       let result = handler.dispatch_read_resource("nom://bogus").await;
       assert!(result.is_err());
       assert!(result.unwrap_err().message.contains("unknown resource URI"));

5. Run: nix develop -c cargo test -p nom-core (all pass, including the 3 new tests). Run: nix develop -c cargo clippy -p nom-core --all-targets --all-features -- -D warnings (clean). Run: nix develop -c cargo fmt --all --check (clean; run `cargo fmt --all` first if it fails).
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Extracted list_resources()/read_resource() bodies into pub(crate) build_resources() and dispatch_read_resource(&self, uri: &str), mirroring the existing build_tools() pattern from TASK-4. Trait methods are now thin wrappers. Added 3 tests: test_build_resources_lists_weekly_summary (sync), test_dispatch_read_resource_returns_weekly_summary_json (async, TempDb-backed, asserts JSON keys start_date/end_date/nutrients/weight), test_dispatch_read_resource_unknown_uri_errors (async). cargo test -p nom-core, cargo clippy -p nom-core --all-targets --all-features -- -D warnings, and cargo fmt --all --check all pass.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Extracted build_resources() and dispatch_read_resource() as testable inherent methods on McpHandler, mirroring build_tools(); added unit test coverage for resource listing, successful weekly-summary dispatch, and the unknown-URI error path.
<!-- SECTION:FINAL_SUMMARY:END -->

---
id: TASK-33
title: 'Fix: McpHandler::list_resources/read_resource have zero test coverage'
status: To Do
assignee: []
created_date: '2026-08-13 11:47'
labels:
  - review-followup
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
- [ ] #1 list_resources()'s body is extracted into a pub(crate) fn build_resources(&self) -> Vec<Resource> mirroring the existing build_tools() (mcp_handler.rs:55-71); list_resources() itself becomes a thin wrapper: Ok(ListResourcesResult::with_all_items(self.build_resources()))
- [ ] #2 read_resource()'s URI-dispatch match block is extracted into a pub(crate) async fn dispatch_read_resource(&self, uri: \&str) -> Result<ReadResourceResult, rmcp::ErrorData>; read_resource() itself becomes a thin wrapper: self.dispatch_read_resource(&request.uri).await
- [ ] #3 test_build_resources_lists_weekly_summary asserts build_resources() returns exactly one Resource with uri "nom://weekly-summary", the expected title, and mime_type "application/json"
- [ ] #4 test_dispatch_read_resource_returns_weekly_summary_json constructs McpHandler via with_db_path() against a TempDb (same pattern as weekly/mod.rs and widget/mod.rs tests), calls dispatch_read_resource("nom://weekly-summary"), asserts Ok, and asserts the returned text content parses as JSON containing top-level keys start_date/end_date/nutrients/weight
- [ ] #5 test_dispatch_read_resource_unknown_uri_errors calls dispatch_read_resource with an unrecognized URI (e.g. "nom://bogus") and asserts an Err whose message contains "unknown resource URI"
- [ ] #6 nix develop -c cargo test -p nom-core passes
- [ ] #7 nix develop -c cargo clippy -p nom-core --all-targets --all-features -- -D warnings passes
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
SETUP (read first): This is a Rust Cargo workspace (nom-core/ library, nom-mcp/ binaries) — a single-user MCP nutrition-tracking server. ALL commands must run inside the Nix dev shell: prefix every command with 'nix develop -c'. Work from the repository root unless told otherwise. Do not change pinned dependency versions.

1. Open nom-core/src/operation/mcp_handler.rs. Locate list_resources() (~line 140-152) and read_resource() (~line 154-223).

2. Add a new method on impl McpHandler (the inherent impl block at the top of the file, alongside build_tools(), NOT inside the ServerHandler trait impl):
   pub(crate) fn build_resources(&self) -> Vec<Resource> {
       vec![
           Resource::new("nom://weekly-summary", "weekly-summary")
               .with_title("Weekly Summary")
               .with_description("Rolling 7-day nutrition and weight summary")
               .with_mime_type("application/json"),
       ]
   }
   Then replace list_resources()'s body with: Ok(ListResourcesResult::with_all_items(self.build_resources()))

3. Add a second inherent method, moving read_resource()'s entire existing match block (the #[cfg(test)]/#[cfg(not(test))] connection-opening logic plus the URI match) verbatim into it:
   pub(crate) async fn dispatch_read_resource(&self, uri: &str) -> Result<ReadResourceResult, ErrorData> {
       match uri {
           ... (existing match arms, unchanged) ...
       }
   }
   Then replace read_resource()'s body with: self.dispatch_read_resource(&request.uri).await

4. In the #[cfg(test)] mod tests block at the bottom of the file, add:
   - test_build_resources_lists_weekly_summary: call handler.build_resources(), assert len() == 1, assert resources[0].uri == "nom://weekly-summary" and resources[0].mime_type == Some("application/json".to_string()) (check exact field names/types on rmcp::model::Resource — adjust assertions to match, e.g. .raw.uri if Resource wraps a raw struct).
   - test_dispatch_read_resource_returns_weekly_summary_json: use crate::storage::test::TempDb (same import weekly/mod.rs and widget/mod.rs use) to create a temp DB, build an McpHandler via McpHandler::new(registry, clock).with_db_path(db.path.clone()), call handler.dispatch_read_resource("nom://weekly-summary").await, assert it's Ok, extract the text from the ReadResourceResult's ResourceContents::TextResourceContents variant, and assert serde_json::from_str::<serde_json::Value>(&text) succeeds and the resulting object has start_date/end_date/nutrients/weight keys.
   - test_dispatch_read_resource_unknown_uri_errors: same handler setup, call dispatch_read_resource("nom://bogus").await, assert it's Err, and assert the error's message/to_string() contains "unknown resource URI".

5. Run: nix develop -c cargo test -p nom-core (all pass, including the 3 new tests). Run: nix develop -c cargo clippy -p nom-core --all-targets --all-features -- -D warnings (clean). Run: nix develop -c cargo fmt --all --check (clean).
<!-- SECTION:PLAN:END -->

---
id: TASK-4
title: >-
  Fix: mcp_handler empty-registry test doesn't call list_tools() or assert
  anything
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-11 23:15'
updated_date: '2026-08-11 23:54'
labels:
  - review-followup
dependencies:
  - TASK-2.7
priority: high
type: bug
ordinal: 110
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while reviewing TASK-2.7 (nom-core/src/operation/mcp_handler.rs, test_empty_registry_list_tools in the #[cfg(test)] mod). The test constructs an McpHandler over an empty OperationRegistry and then does 'let_ = handler;' — it never calls list_tools() and asserts nothing, so it exercises no behavior and would pass even if list_tools() panicked or returned garbage on an empty registry. This is a Correct-axis gap: the empty-registry edge case named by the test's own title is untested.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 test_empty_registry_list_tools (or its replacement) actually invokes McpHandler::list_tools() against an empty registry and asserts the result is Ok with an empty tool list
- [ ] #2 nix develop -c cargo test -p nom-core passes
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
SETUP (read first): This is a Rust+WebAssembly core (crates/gql-core) with a TypeScript/React web app (web/). ALL commands must run inside the Nix dev shell: either run 'direnv allow' once, or prefix every command with 'nix develop -c'. Work from the repository root unless told otherwise. Do not change pinned dependency versions.

Note: this repo's actual layout is nom-core/ (library) and nom-mcp/ (binaries), not crates/gql-core / web/ — use nom-core/ for all paths below.

1. Open nom-core/src/operation/mcp_handler.rs and locate test_empty_registry_list_tools in the #[cfg(test)] mod tests block. It currently reads:
   #[test]
   fn test_empty_registry_list_tools() {
       let handler = McpHandler::new(OperationRegistry::new());
       // The handler should work even with an empty registry
       let_ = handler;
   }

2. Rewrite it as a #[tokio::test] (list_tools is async) that actually calls list_tools() and asserts on the result. You will need a RequestContext<RoleServer> to call list_tools() — check how nom-core's own rmcp dependency version constructs one for tests (search the rmcp crate docs/source under the pinned version in Cargo.lock, or check if rmcp exposes a test-friendly constructor); if none is available, cast a narrower net and instead unit-test the registry-iteration logic that list_tools() delegates to (e.g. by extracting the tool-building loop into a small helper function that doesn't need a RequestContext, and testing that helper directly against an empty registry). Prefer testing the real public method if it is reasonably constructible; fall back to the helper-extraction approach only if the RequestContext cannot be constructed in a unit test.

3. Assert the empty-registry case returns an empty (not error, not panic) tool list.

4. Run: nix develop -c cargo test -p nom-core. Must pass before closing this ticket.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Extracted tool-building logic into McpHandler::build_tools() helper (pub(crate)) so list_tools() delegates to it and the empty-registry test can actually invoke and assert on the result without needing RequestContext<RoleServer>. Rewrote test_empty_registry_list_tools to call build_tools() and assert tools.is_empty(). All 70 tests pass.

Fixup applied post-review (commit a2f3669, fixup! cf541c8): build_tools() had a broken indentation (rustfmt violation) and test_list_tools_omits_bad_schema_but_keeps_good_ops duplicated the filter_map logic build_tools() now encapsulates instead of calling it. Reformatted and rewired the test to call handler.build_tools() directly.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Fixed no-op empty-registry test by extracting build_tools() helper from list_tools(), enabling unit test of the tool-building logic without constructing RequestContext<RoleServer>.
<!-- SECTION:FINAL_SUMMARY:END -->

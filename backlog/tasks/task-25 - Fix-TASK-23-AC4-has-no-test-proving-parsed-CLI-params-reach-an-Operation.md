---
id: TASK-25
title: 'Fix: TASK-23 AC#4 has no test proving parsed CLI params reach an Operation'
status: To Do
assignee: []
created_date: '2026-08-13 01:09'
labels:
  - review-followup
dependencies:
  - TASK-23
priority: high
ordinal: 145
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while reviewing TASK-23 (nom-mcp/src/main.rs execute_from_args(), nom-core/src/cli.rs). TASK-23's own AC#4 required 'A new integration test proves local-CLI now passes parsed params through to an Operation (e.g. invoking search_food with query=almonds resolves a non-empty query, not an empty object)' and was checked off ([x]), but the two tests actually added (nom-core/src/cli.rs: test_parse_params_produces_query_for_search_food, test_parse_params_empty_vs_with_args) only call parse_params() in isolation — neither one calls SearchFood::execute_json, an Operation, or nom-mcp/src/main.rs::execute_from_args at all. nom-mcp/src/main.rs has zero #[cfg(test)] mod tests (confirmed via 'cargo test --workspace' output: 'Running unittests src/main.rs ... running 0 tests'). Correctness-axis gap: the exact regression this ticket existed to fix (main.rs silently discarding args via a hardcoded json!({}) stub) has no test tying the parser's output to an actual Operation invocation, so a future refactor could silently reintroduce it and cargo test --workspace would stay green.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A new test proves parsed CLI params (e.g. query=almonds) actually change an Operation's behavior/output compared to no params, not just parse_params()'s own return value
- [ ] #2 The test either lives in a #[cfg(test)] mod tests block added to nom-mcp/src/main.rs and calls execute_from_args(...), OR lives in nom-core/src/food/mod.rs's existing test module and feeds nom_core::cli::parse_params(...)'s output directly into SearchFood::execute_json (using the existing TempDb/.with_db_path()/make_off_client/make_fdc_client test seams already used by test_search_food_free_text_custom_only at ~line 949) — either way it must exercise an Operation's execute_json, not parse_params alone
- [ ] #3 nix develop -c cargo test --workspace passes
- [ ] #4 nix develop -c cargo clippy --workspace --all-targets is clean
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
SETUP (read first): This is a Rust+WebAssembly core (crates/gql-core) with a TypeScript/React web app (web/). Wait -- this repo's actual crate layout is nom-core/ and nom-mcp/ (not crates/gql-core -- ignore that path in the preamble; everything else in the preamble still applies). ALL commands must run inside the Nix dev shell: either run 'direnv allow' once, or prefix every command with 'nix develop -c'. Work from the repository root unless told otherwise. Do not change pinned dependency versions.

1. Open nom-core/src/food/mod.rs and read SearchFood (struct at ~line 347, Operation impl at ~line 372) and its SearchFoodRequest (~line 342, note 'query: String' is a REQUIRED field with no default -- deserializing {} against it fails with a validation error, while {"query": "..."} succeeds; this is your natural signal that params flowed through, distinct from calling with {}).
2. Read the existing test test_search_food_free_text_custom_only (nom-core/src/food/mod.rs ~line 949) as your template: it builds a TempDb, seeds a custom food row directly via SQL, wires make_off_client/make_fdc_client against a wiremock MockServer, and constructs SearchFood::new(off, Some(fdc)).with_db_path(db.path.clone()).
3. Add a new test in the same mod tests block, e.g. test_search_food_via_parsed_cli_params_differs_from_empty. In it: build a SearchFood op the same way as step 2 (seed one custom food, e.g. 'Almond Butter'), then call op.execute_json(Arc::new(nom_core::cli::parse_params(&["query=almond".to_string()]).unwrap())) and assert it returns the seeded food (non-empty array matching the seeded row). Then call op.execute_json(Arc::new(nom_core::cli::parse_params(&[]).unwrap())) (i.e. params = {}) and assert it returns Err (SearchFoodRequest deserialization fails because 'query' is missing) -- this proves the parsed key=value args are the thing actually changing the Operation's behavior, not just parse_params's own return value.
4. Add 'use nom_core::cli::parse_params;' (or fully-qualify as in step 3) to the test module's imports if not already present.
5. Run: nix develop -c cargo test -p nom-core -- food::tests::test_search_food_via_parsed_cli_params_differs_from_empty --nocapture and confirm it passes, then the full suite.
6. Run: nix develop -c cargo test --workspace -- confirm all tests pass (only the pre-existing separately-tracked failures, if any remain, should differ from a clean run -- as of this review the full suite is green with zero failures, so expect zero failures here too).
7. Run: nix develop -c cargo clippy --workspace --all-targets -- confirm clean.
8. Run: nix develop -c cargo fmt --all -- --check -- confirm clean.
<!-- SECTION:PLAN:END -->

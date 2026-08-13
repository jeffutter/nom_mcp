---
id: TASK-25
title: 'Fix: TASK-23 AC#4 has no test proving parsed CLI params reach an Operation'
status: Done
assignee:
  - '@ralph'
created_date: '2026-08-13 01:09'
updated_date: '2026-08-13 04:13'
labels:
  - review-followup
  - planned
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
- [x] #1 A new test proves parsed CLI params (e.g. query=almonds) actually change an Operation's behavior/output compared to no params, not just parse_params()'s own return value
- [x] #2 The test either lives in a #[cfg(test)] mod tests block added to nom-mcp/src/main.rs and calls execute_from_args(...), OR lives in nom-core/src/food/mod.rs's existing test module and feeds nom_core::cli::parse_params(...)'s output directly into SearchFood::execute_json (using the existing TempDb/.with_db_path()/make_off_client/make_fdc_client test seams already used by test_search_food_free_text_custom_only at ~line 949) — either way it must exercise an Operation's execute_json, not parse_params alone
- [x] #3 nix develop -c cargo test --workspace passes
- [x] #4 nix develop -c cargo clippy --workspace --all-targets is clean
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Single new integration test in nom-core/src/food/mod.rs tests module that feeds parse_params() output directly into SearchFood::execute_json().

## Steps

1. **Read context**: Open nom-core/src/food/mod.rs ~line 692 (mod tests block), confirm existing imports (`use crate::storage::test::TempDb;`, wiremock matchers). Note that `SearchFoodRequest` (~line 342) has `query: String` with NO default — deserializing `{}` against it produces `Err(validation("query", ...))`.

2. **Add the test** — `test_search_food_via_parsed_cli_params_proves_params_flow_to_operation`:
   - Build `SearchFood` op the same way as `test_search_food_free_text_custom_only`: `TempDb`, seed one custom food ("Almond Butter"), mock USDA returning empty, wire `make_off_client`/`make_fdc_client`, construct op with `.with_db_path(db.path.clone())`.
   - **Part A — with params**: Call `op.execute_json(Arc::new(crate::cli::parse_params(&["query=almond".into()]).unwrap()))`. Assert result is non-empty array containing the seeded food (`arr[0]["name"] == "Almond Butter"`).
   - **Part B — without params**: Call `op.execute_json(Arc::new(crate::cli::parse_params(&[]).unwrap()))`. Assert it returns `Err` because `SearchFoodRequest` deserialization fails on missing `query` field. This proves the parsed key=value args are what actually drives Operation behavior, not just `parse_params()` return value.

3. **Import**: Add `use crate::cli::parse_params;` to the test module imports (or fully qualify inline — `crate::cli::parse_params(...)` works too since `super::*` is already imported).

4. **Run targeted test**: `nix develop -c cargo test -p nom-core -- food::tests::test_search_food_via_parsed_cli_params_proves_params_flow_to_operation --nocapture` — confirm Part A succeeds with seeded food, Part B fails with validation error.

5. **Run full suite**: `nix develop -c cargo test --workspace` — all green (zero failures expected per current baseline).

6. **Lint check**: `nix develop -c cargo clippy --all-targets --all-features --workspace -- -D warnings` — clean.

7. **Format check**: `nix develop -c cargo fmt --all -- --check` — clean.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Test added in nom-core/src/food/mod.rs: test_search_food_via_parsed_cli_params_proves_params_flow_to_operation. Two-part test: Part A uses parse_params(['query=almond']) -> execute_json() -> finds seeded 'Almond Butter' custom food. Part B uses parse_params([]) -> execute_json() -> validation error on missing 'query'. All 166 workspace tests pass, clippy clean, format clean.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added integration test in nom-core/src/food/mod.rs that feeds parse_params() output directly into SearchFood::execute_json(). Part A proves parsed CLI params drive operation behavior (finds seeded food), Part B proves empty params produce validation error. This closes the correctness gap from TASK-23 where parse_params was tested in isolation but no test tied it to an Operation invocation.
<!-- SECTION:FINAL_SUMMARY:END -->

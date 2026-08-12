---
id: TASK-17
title: >-
  Fix: remove unreachable!() panic in SearchMeals's hand-rolled parameter-count
  dispatch
status: To Do
assignee: []
created_date: '2026-08-12 20:22'
labels:
  - review-followup
dependencies:
  - TASK-2.14
priority: high
ordinal: 190
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while reviewing TASK-2.14 (nom-core/src/meal/mod.rs:1336, inside SearchMeals's match params.len() { 1 => ..., 2 => ..., 3 => ..., _ => unreachable!() } dispatch spanning roughly lines 1260-1337). This works around dynamic SQL parameter binding by hand-writing three nearly-identical match arms (one per possible bound-parameter count from the optional date_range fields) and falling back to unreachable!() for any other count. Currently unreachable given the two optional date-range fields, but it is a live panic macro in non-test code -- the project's convention (see CLAUDE.md 'errors as values, no panics across the boundary') forbids unwrap/expect/panic outside tests, and this is exactly the kind of latent landmine that convention exists to prevent: a future added optional search filter that changes the possible param counts would make this reachable and crash the MCP server process instead of returning an error. It is also copy-pasted boilerplate (three near-identical query-and-collect blocks) rather than a single dynamic-params bind call.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The three near-identical match arms (params.len() == 1, 2, 3) in SearchMeals::execute_json are replaced with a single code path that binds a dynamic-length parameter list to the query, without any unreachable!()/panic!()/unwrap()/expect() in the non-test path
- [ ] #2 If the turso crate's query API requires a fixed-arity tuple for binding (check its docs/existing usage in this file), the replacement uses whatever dynamic-binding mechanism it provides (e.g. binding a Vec of turso::Value, or building the SQL string with the exact right number of ? placeholders and binding a slice) instead of a match-per-arity; if no such mechanism exists, the fallback arm returns Err(ErrorData::storage_failure(...)) instead of unreachable!(), and a comment explains why the match is exhaustive-in-practice
- [ ] #3 Existing search_meals tests (test_search_meals_matches_food_names, test_search_meals_no_results, and any date_range-filtered search test) pass unchanged
- [ ] #4 nix develop -c cargo test -p nom-core passes
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
SETUP (read first): This is a Rust+WebAssembly core (nom-core, nom-mcp) with SQLite storage via the turso crate. ALL commands must run inside the Nix dev shell: either run 'direnv allow' once, or prefix every command with 'nix develop -c'. Work from the repository root unless told otherwise. Do not change pinned dependency versions.

1. Open nom-core/src/meal/mod.rs and read SearchMeals::execute_json in full (search for 'impl Operation for SearchMeals', around line 1217 through the unreachable!() at line 1336 and a bit past it) to understand exactly what SQL and params vector are built before the match.
2. Check how the turso crate's Statement::query is invoked elsewhere in this file and in nom-core/src/food/mod.rs for a call site that binds a variable number of parameters (grep for '.query(' across both files) -- turso's turso crate may support binding a Vec<turso::Value> or similar dynamic parameter list; confirm by checking the turso crate's version pinned in nom-core/Cargo.toml and, if available, its docs via context7 or docs.rs for the exact API (e.g. does .query() accept impl IntoParams, and is there a Vec<Value>/params_from_iter equivalent?).
3. If a dynamic-arity bind exists: replace the params.len() match entirely with one query call using that dynamic form, building the params Vec<turso::Value> alongside the SQL string's ? placeholders (which should already be constructed dynamically earlier in this function based on which of query/date_range.start/date_range.end are present -- read that SQL-building code above the match to confirm).
4. If no dynamic-arity bind exists in this turso version: keep the three arms but replace '_ => unreachable!()' with '_ => return Err(ErrorData::storage_failure(format!("unexpected search_meals parameter count: {}", params.len())))' and add a one-line comment above the match explaining the arity is currently bounded by the two optional date_range fields plus the required query, so a future filter addition must extend this match rather than silently panicking.
5. Run: nix develop -c cargo test -p nom-core -- meal::tests::test_search_meals --nocapture and confirm all search_meals tests pass, then run the full suite: nix develop -c cargo test -p nom-core, and nix develop -c cargo clippy -p nom-core --all-targets
<!-- SECTION:PLAN:END -->
